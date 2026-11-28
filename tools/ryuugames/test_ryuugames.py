"""Pruebas sin red: DB sintética, respuestas JSON de ejemplo y un opener falso.

Ejecutar:  py -3.14 -m unittest discover -s tools/ryuugames -v
"""
import hashlib
import io
import json
import sqlite3
import tempfile
import threading
import unittest
import urllib.error
import urllib.parse
from pathlib import Path

import ryuugames as rg
from PIL import Image

ESQUEMA = """
CREATE TABLE juegos (id INTEGER PRIMARY KEY AUTOINCREMENT, titulo TEXT NOT NULL, descripcion TEXT DEFAULT '',
  estado TEXT DEFAULT 'Sin empezar', idioma TEXT DEFAULT 'Español', imagen BLOB, ruta TEXT DEFAULT '',
  codigo TEXT DEFAULT '', pagina TEXT DEFAULT '', tipo TEXT DEFAULT 'Otro', puntuacion REAL DEFAULT 0);
CREATE TABLE tags (id INTEGER PRIMARY KEY AUTOINCREMENT, nombre TEXT NOT NULL COLLATE NOCASE UNIQUE);
CREATE TABLE juego_tags (juego_id INTEGER NOT NULL, tag_id INTEGER NOT NULL, PRIMARY KEY (juego_id, tag_id));
"""
# id, titulo, descripcion, codigo, imagen
FILAS = [
    (1, "RY 1001", None, "RY-RJ01000001", None),        # verificada, título provisional
    (2, "Mi título bueno", "  \t\n", "RY-RA25000002", None),  # verificada, título significativo
    (3, "RY 1003", "", "RY-RJ01000003", None),          # no encontrado
    (4, "RY 1004", "", "RY-RJ01000004", None),          # dudosa (solo en contenido)
    (5, "RY 1005", "", "RY-12345678", None),            # excluido
    (6, "RY 1006", "", "RY-ABC123456", None),           # excluido
    (7, "RY 1007", "", "", None),                       # inválido
    (8, "Con descripción", "ya tiene", "RY-RJ01000008", None),  # no candidato
    (9, "RY 1009", "", "rj01000009", b"IMAGEN-EXISTENTE"),  # sin RY-, con imagen
]


def png(color=(200, 30, 30)):
    buf = io.BytesIO()
    Image.new("RGB", (640, 480), color).save(buf, "PNG")
    return buf.getvalue()


def post(id_, slug, titulo, descripcion, img=None, codigo_contenido=""):
    contenido = (f'<p>Original Title : algo {codigo_contenido}</p>'
                 f'<h4 class="ryuuline">DESCRIPTION</h4>'
                 f'<p>{descripcion}</p><p><img src="https://img.example/shot.jpg"></p>'
                 f'<h4 class="ryuuline">LINK DOWNLOAD ( 1 MB )</h4><script>var x=1;</script>')
    p = {"id": id_, "link": f"https://www.ryuugames.com/{slug}/", "slug": slug,
         "title": {"rendered": titulo}, "content": {"rendered": contenido}, "featured_media": 9}
    if img:
        p["_embedded"] = {"wp:featuredmedia": [{"source_url": img}]}
    return p


RESPUESTAS = {
    "RJ01000001": [post(1, "juego-uno-rj01000001", "[ENG] Juego Uno (V1.2) (V26.08.15) (RJ01000001)",
                        "Primera &#8216;historia&#8217;.<br />Segunda línea.", img="https://img.example/uno.jpg")],
    "RA25000002": [post(2, "eng-dos-ra25000002", "[ENG] Juego Dos Uncensored (072Project) (RA25000002)",
                        "Texto dos.", img="https://img.example/dos.png")],
    "RJ01000003": [],
    "RJ01000004": [post(4, "otra-cosa-rj09999999", "Otra cosa (RJ09999999)", "x", codigo_contenido="RJ01000004")],
    "rj01000009": [post(9, "nueve-rj01000009", "Nueve (RJ01000009)", "Desc nueve.", img="https://img.example/nueve.jpg")],
}


class Opener:
    """Sustituye a urllib.request.urlopen y registra cada URL pedida."""

    def __init__(self, respuestas=RESPUESTAS, imagenes=None, errores=None):
        self.respuestas, self.urls = respuestas, []
        self.imagenes = imagenes if imagenes is not None else {
            "https://img.example/uno.jpg": png(), "https://img.example/dos.png": png((0, 0, 255)),
            "https://img.example/nueve.jpg": png()}
        self.errores = errores or {}

    def __call__(self, req, timeout=None):
        url = req.full_url
        self.urls.append(url)
        e = self.errores.get(url)
        if isinstance(e, list):  # errores de una sola vez, luego responde normal
            e = e.pop(0) if e else None
        if e:
            raise e
        if url in self.imagenes:
            return io.BytesIO(self.imagenes[url])
        q = urllib.parse.parse_qs(urllib.parse.urlparse(url).query)
        return io.BytesIO(json.dumps(self.respuestas.get(q["search"][0], [])).encode())


def cliente(opener, **kw):
    return rg.Cliente(intervalo=0, abrir=opener, **kw)


class Base(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.db = Path(self.tmp.name) / "prueba.db"
        con = sqlite3.connect(self.db)
        con.executescript(ESQUEMA)
        con.executemany("INSERT INTO juegos (id, titulo, descripcion, codigo, imagen, estado, idioma, ruta, pagina, tipo, puntuacion)"
                        " VALUES (?,?,?,?,?,'Jugando','Inglés','C:/j','ryuugames','RPG',4.5)", FILAS)
        con.execute("INSERT INTO tags (nombre) VALUES ('t')")
        con.execute("INSERT INTO juego_tags VALUES (1, 1)")
        con.commit()
        con.close()

    def tearDown(self):
        self.tmp.cleanup()

    def filas(self):
        con = sqlite3.connect(self.db)
        try:
            return {r[0]: r for r in con.execute("SELECT * FROM juegos ORDER BY id")}
        finally:
            con.close()

    def sha(self):
        return hashlib.sha256(self.db.read_bytes()).hexdigest()


class Codigos(unittest.TestCase):
    def test_prefijo_y_regla_r(self):
        casos = {"RY-RJ01234567": ("RJ01234567", "elegible"), "RY-R12345678": ("R12345678", "elegible"),
                 "RY-12345678": ("12345678", "excluido"), "RY-ABC123456": ("ABC123456", "excluido"),
                 "RJ01234567": ("RJ01234567", "elegible"), "ry-rj1704529": ("rj1704529", "elegible"),
                 "RY-RA25078923": ("RA25078923", "elegible")}
        for cod, esperado in casos.items():
            self.assertEqual(rg.clasificar_codigo(cod), esperado, cod)

    def test_invalidos(self):
        for cod in (None, "", "   ", "RY-", "RY- ", "RJ 123", "RJ/1", "RY-RJ1;DROP"):
            self.assertEqual(rg.clasificar_codigo(cod)[1], "invalido", cod)

    def test_titulo_provisional(self):
        self.assertTrue(rg.es_titulo_provisional("RY 42996", "RY-D", "D"))
        self.assertTrue(rg.es_titulo_provisional("", "RY-RJ1", "RJ1"))
        self.assertTrue(rg.es_titulo_provisional("RJ01", "RY-RJ01", "RJ01"))
        self.assertFalse(rg.es_titulo_provisional("Tropical Junction", "RY-RJ1", "RJ1"))


class Extraccion(unittest.TestCase):
    def test_limpiar_titulo(self):
        self.assertEqual(rg.limpiar_titulo("[ENG] Tropical Junction (V2.2) (V26.08.15) (RJ01389169)"), "Tropical Junction")
        self.assertEqual(rg.limpiar_titulo("[ENG] Elena&#8217;s Lewd Service Uncensored (072Project) (RA25101529)"),
                         "Elena’s Lewd Service Uncensored (072Project)")
        self.assertEqual(rg.limpiar_titulo("JD to Goblin Yashiki (RJ395613)"), "JD to Goblin Yashiki")
        self.assertEqual(rg.limpiar_titulo("Juego (Ver1.02b) (RJ0123, RJ0456)"), "Juego")
        self.assertEqual(rg.limpiar_titulo("[ENG] (V1.0)"), "")  # solo etiquetas: no se propone título

    def test_descripcion(self):
        h = RESPUESTAS["RJ01000001"][0]["content"]["rendered"]
        self.assertEqual(rg.extraer_descripcion(h), "Primera ‘historia’. Segunda línea.")
        self.assertEqual(rg.extraer_descripcion("<p>sin bloque</p>"), "")
        anidada = '<h4>DESCRIPTION</h4><div><div class="vndetails"><p>Uno.</p><p>Dos.</p></div></div><h4>X</h4>'
        self.assertEqual(rg.extraer_descripcion(anidada), "Uno. Dos.")
        img_inicial = '<h4>DESCRIPTION</h4><p><img src="https://x/a.jpg"></p><p>Uno.</p><p><img src="https://x/b.jpg"></p><p>pie</p>'
        self.assertEqual(rg.extraer_descripcion(img_inicial), "Uno.")
        span = '<h4>DESCRIPTION</h4><p>Uno.</p><p><span style="x">LINK DOWNLOAD ( 397 MB )</span></p>'
        self.assertEqual(rg.extraer_descripcion(span), "Uno.")

    def test_limite_400(self):
        largo = "<h4>DESCRIPTION</h4><p>" + "palabra " * 100 + "</p>"
        d = rg.extraer_descripcion(largo)
        self.assertLessEqual(len(d), 400)
        self.assertTrue(d.endswith("palabra"))
        self.assertEqual(rg.recortar("a" * 450), "a" * 400)

    def test_imagen(self):
        self.assertEqual(rg.url_imagen(RESPUESTAS["RJ01000001"][0]), "https://img.example/uno.jpg")
        sin_embed = {"content": {"rendered": '<img src="https://x/no.jpg"><img class="alignnone wp-image-5" src="https://x/si.jpg">'}}
        self.assertEqual(rg.url_imagen(sin_embed), "https://x/si.jpg")
        self.assertEqual(rg.url_imagen({"content": {"rendered": '<img src="https://x/no.jpg">'}}), "")
        self.assertEqual(rg.url_imagen({"content": {"rendered": "<img class='wp-image-5' src='https://x/s.jpg'>"}}), "https://x/s.jpg")
        self.assertEqual(rg.url_imagen({"_embedded": {"wp:featuredmedia": [{"source_url": "http://x/inseguro.jpg"}]}}), "")

    def test_validacion(self):
        self.assertEqual(rg.validar("RJ1", [])[0], "no_encontrado")
        self.assertEqual(rg.validar("RJ01000001", RESPUESTAS["RJ01000001"])[0], "verificada")
        self.assertEqual(rg.validar("RJ01000004", RESPUESTAS["RJ01000004"])[0], "dudosa")
        # prefijo de otro código no cuenta: RJ0100000 ≠ RJ01000001
        self.assertEqual(rg.validar("RJ0100000", RESPUESTAS["RJ01000001"])[0], "no_encontrado")
        dos = RESPUESTAS["RJ01000001"] * 2
        self.assertEqual(rg.validar("RJ01000001", dos)[0], "dudosa")
        jp = post(7, "juego-uno-jp-rj01000001", "Juego Uno JP (RJ01000001)", "x")
        v, p, motivo = rg.validar("RJ01000001", [jp] + RESPUESTAS["RJ01000001"])
        self.assertEqual((v, p["id"]), ("verificada", 1))
        self.assertIn("[ENG]", motivo)
        # código como parte de una palabra: «PR» no coincide con «prepare»
        brat = [post(8, "you-little-brat-prepare-yourself-rj01", "You little brat, prepare yourself! (RJ01)", "x")]
        self.assertEqual(rg.validar("PR", brat)[0], "no_encontrado")
        self.assertEqual(rg.validar("RJ0", [post(9, "a-rj0b", "A (RJ0B)", "x")])[0], "no_encontrado")
        # código sin dígitos nunca se verifica
        self.assertEqual(rg.validar("CSAR", [post(10, "csar", "CSAR", "x")])[0], "dudosa")
        self.assertEqual(rg.validar("R1", [post(11, "mecha-r1-ver2", "Mecha R1 Ver2", "x")])[0], "dudosa")

    def test_webp(self):
        w = rg.a_webp(png())
        self.assertEqual(w[:4], b"RIFF")
        self.assertLessEqual(max(Image.open(io.BytesIO(w)).size), 300)
        with self.assertRaises(Exception):
            rg.a_webp(b"<html>no es imagen</html>")


class Cliente_(unittest.TestCase):
    def test_reintentos_limitados(self):
        url = "https://h/x"
        err = urllib.error.HTTPError(url, 500, "x", {}, None)
        op = Opener(errores={url: err})
        with self.assertRaises(rg.ErrorHttp):
            cliente(op, reintentos=2).get(url)
        self.assertEqual(len(op.urls), 3)

    def test_respuesta_cortada_se_reintenta(self):
        import http.client
        url = "https://h/x"
        op = Opener(imagenes={url: b"ok"}, errores={url: [http.client.IncompleteRead(b"")]})
        self.assertEqual(cliente(op).get(url), b"ok")

    def test_429_detiene(self):
        url = "https://h/x"
        op = Opener(errores={url: urllib.error.HTTPError(url, 429, "x", {}, None)})
        with self.assertRaises(rg.Bloqueado):
            cliente(op).get(url)
        self.assertEqual(len(op.urls), 1)

    def test_desafio_cloudflare(self):
        url = "https://h/x"
        op = Opener(imagenes={url: b"<!DOCTYPE html><html><title>Just a moment...</title>"})
        with self.assertRaises(rg.Bloqueado):
            cliente(op).get(url)

    def test_conexion_interrumpida_se_recupera(self):
        url = "https://h/x"
        op = Opener(imagenes={url: b"ok"}, errores={url: [urllib.error.URLError("reset")]})
        self.assertEqual(cliente(op).get(url), b"ok")

    def test_cancelacion(self):
        ev = threading.Event()
        ev.set()
        with self.assertRaises(rg.Cancelado):
            rg.Cliente(intervalo=5, cancel=ev, abrir=Opener()).get("https://h/x")


class Flujo(Base):
    def test_candidatos(self):
        total, regs = rg.leer_candidatos(self.db)
        self.assertEqual(total, 9)
        por = {r["id"]: r["clase"] for r in regs}
        self.assertEqual(por, {1: "elegible", 2: "elegible", 3: "elegible", 4: "elegible", 5: "excluido",
                               6: "excluido", 7: "invalido", 9: "elegible"})

    def test_dry_run_no_modifica_y_excluidos_sin_http(self):
        antes = self.sha()
        op = Opener()
        sesion, parada = rg.buscar(self.db, cliente(op))
        self.assertIsNone(parada)
        self.assertEqual(self.sha(), antes)
        buscados = [urllib.parse.parse_qs(urllib.parse.urlparse(u).query)["search"][0] for u in op.urls if "wp-json" in u]
        self.assertEqual(sorted(buscados), sorted(["RJ01000001", "RA25000002", "RJ01000003", "RJ01000004", "rj01000009"]))
        self.assertNotIn("https://img.example/nueve.jpg", op.urls)  # ya tiene imagen: no se descarga
        self.assertEqual(sesion["1"]["validacion"], "verificada")
        self.assertEqual(sesion["1"]["titulo_encontrado"], "Juego Uno")
        self.assertTrue(Path(sesion["1"]["imagen_archivo"]).exists())
        self.assertEqual(sesion["3"]["validacion"], "no_encontrado")
        self.assertEqual(sesion["4"]["validacion"], "dudosa")
        self.assertFalse(rg.seleccionable(sesion["3"]) or rg.seleccionable(sesion["4"]))

    def test_reanudacion_y_cache(self):
        rg.buscar(self.db, cliente(Opener()))
        op = Opener()
        rg.buscar(self.db, cliente(op))
        self.assertEqual(op.urls, [])  # todo resuelto: ninguna solicitud
        op = Opener()
        rg.buscar(self.db, cliente(op), reintentar=True)
        self.assertEqual(len(op.urls), 2)  # solo no encontrado + dudosa

    def test_error_http_se_reintenta_en_la_siguiente_ejecucion(self):
        url = rg.url_busqueda("RJ01000003")
        op = Opener(errores={url: urllib.error.HTTPError(url, 404, "x", {}, None)})
        sesion, _ = rg.buscar(self.db, cliente(op))
        self.assertEqual(sesion["3"]["estado_busqueda"], "error_http")
        op = Opener()
        rg.buscar(self.db, cliente(op))
        self.assertEqual(len(op.urls), 1)

    def test_bloqueo_detiene_sin_perder_progreso(self):
        url = rg.url_busqueda("RA25000002")
        op = Opener(errores={url: urllib.error.HTTPError(url, 403, "x", {}, None)})
        sesion, parada = rg.buscar(self.db, cliente(op))
        self.assertIn("bloqueó", parada)
        self.assertIn("1", rg.cargar_sesion(self.db))
        self.assertNotIn("2", sesion)

    def test_aplicar(self):
        antes = self.filas()
        sesion, _ = rg.buscar(self.db, cliente(Opener()))
        res = rg.aplicar(self.db, sesion, [1, 2, 3, 4, 9])
        self.assertEqual(sorted(res["actualizados"]), [1, 2, 9])
        self.assertTrue(Path(res["backup"]).exists())
        bk = sqlite3.connect(res["backup"])
        self.assertEqual(bk.execute("SELECT descripcion FROM juegos WHERE id=1").fetchone()[0], None)
        bk.close()
        despues = self.filas()
        # 1: título provisional reemplazado, descripción e imagen nuevas
        self.assertEqual(despues[1][1], "Juego Uno")
        self.assertEqual(despues[1][2], "Primera ‘historia’. Segunda línea.")
        self.assertEqual(despues[1][5][:4], b"RIFF")
        # 2: título significativo conservado
        self.assertEqual(despues[2][1], "Mi título bueno")
        self.assertEqual(despues[2][2], "Texto dos.")
        # 9: imagen existente intacta
        self.assertEqual(despues[9][5], b"IMAGEN-EXISTENTE")
        # no encontrados, dudosos, excluidos y con descripción: idénticos
        for i in (3, 4, 5, 6, 7, 8):
            self.assertEqual(despues[i], antes[i], i)
        # otros campos intactos
        for i in (1, 2, 9):
            self.assertEqual(despues[i][3:5] + despues[i][6:], antes[i][3:5] + antes[i][6:], i)
        con = sqlite3.connect(self.db)
        self.assertEqual(con.execute("SELECT * FROM juego_tags").fetchall(), [(1, 1)])
        con.close()
        self.assertTrue(rg.cargar_sesion(self.db)["1"]["aplicado"])

    def test_reemplazar_titulo_autorizado(self):
        sesion, _ = rg.buscar(self.db, cliente(Opener()))
        rg.aplicar(self.db, sesion, [2], reemplazar_titulo=[2])
        self.assertEqual(self.filas()[2][1], "Juego Dos Uncensored (072Project)")

    def test_titulo_autorizado_pero_editado_entretanto_se_conserva(self):
        sesion, _ = rg.buscar(self.db, cliente(Opener()))
        con = sqlite3.connect(self.db)
        con.execute("UPDATE juegos SET titulo='Editado a mano' WHERE id=2")
        con.commit()
        con.close()
        rg.aplicar(self.db, sesion, [2], reemplazar_titulo=[2])
        self.assertEqual(self.filas()[2][1:3], ("Editado a mano", "Texto dos."))

    def test_db_bloqueada_no_cambia_nada_y_muestra_el_error_real(self):
        sesion, _ = rg.buscar(self.db, cliente(Opener()))
        antes = self.filas()
        otro = sqlite3.connect(self.db, isolation_level=None)
        otro.execute("BEGIN EXCLUSIVE")
        try:
            with self.assertRaisesRegex(sqlite3.OperationalError, "locked"):
                rg.aplicar(self.db, sesion, [1])
        finally:
            otro.execute("ROLLBACK")
            otro.close()
        self.assertEqual(self.filas(), antes)

    def test_no_sobrescribe_descripcion_escrita_entretanto(self):
        sesion, _ = rg.buscar(self.db, cliente(Opener()))
        con = sqlite3.connect(self.db)
        con.execute("UPDATE juegos SET descripcion='manual' WHERE id=1")
        con.commit()
        con.close()
        res = rg.aplicar(self.db, sesion, [1])
        self.assertEqual(res["actualizados"], [])
        self.assertIn(1, res["omitidos"])
        self.assertEqual(self.filas()[1][1:3], ("RY 1001", "manual"))

    def test_imagen_invalida_no_cambia_imagen(self):
        op = Opener(imagenes={"https://img.example/uno.jpg": b"basura", "https://img.example/dos.png": png()})
        sesion, _ = rg.buscar(self.db, cliente(op))
        self.assertIn("imagen inválida", sesion["1"]["extraccion"])
        self.assertTrue(rg.seleccionable(sesion["1"]))  # la descripción sí es aplicable
        rg.aplicar(self.db, sesion, [1])
        self.assertIsNone(self.filas()[1][5])

    def test_informes(self):
        total, regs = rg.leer_candidatos(self.db)
        sesion, _ = rg.buscar(self.db, cliente(Opener()))
        csv_path, txt_path = rg.escribir_informes(self.db, total, regs, sesion)
        lineas = csv_path.read_text(encoding="utf-8-sig").splitlines()
        self.assertEqual(len(lineas), 1 + len(regs))
        self.assertTrue(lineas[0].startswith("Código original,Código utilizado"))
        r = rg.resumen(total, regs, sesion)
        self.assertEqual((r["Registros elegibles"], r["Excluidos por no contener R"], r["Código inválido"]), (5, 2, 1))
        self.assertEqual((r["Coincidencias verificadas"], r["Coincidencias dudosas"], r["No encontrados"]), (3, 1, 1))
        self.assertIn("Total de registros: 9", txt_path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
