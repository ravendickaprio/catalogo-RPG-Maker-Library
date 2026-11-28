"""Recuperación de títulos, descripciones y portadas desde Ryuugames (ADR-021).

Núcleo sin interfaz: filtro de códigos, búsqueda por la API REST pública de
WordPress, validación conservadora, extracción, sesión reanudable, backup y
aplicación transaccional. La GUI está en recuperar_ryuugames.py.
"""
import csv
import html
import http.client
import io
import json
import logging
import os
import re
import sqlite3
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime
from html.parser import HTMLParser
from pathlib import Path

BASE = "https://www.ryuugames.com"
USER_AGENT = "RPGMakerLibrary-metadata/1.0"
DESC_MAX = 400
IMG_MAX = (300, 300)          # igual que catalogo_rpgmaker-v04.py
IMG_CALIDAD = 65
IMG_MAX_BYTES = 15 * 1024 * 1024
WS = "char(32,9,10,11,12,13,160)"  # espacios que cuentan como «vacío» en SQL
VACIA = f"trim(coalesce(descripcion,''), {WS}) = ''"

log = logging.getLogger("ryuugames")


# ---------------------------------------------------------------- códigos
def clasificar_codigo(codigo):
    """Devuelve (código de búsqueda | None, 'elegible' | 'excluido' | 'invalido')."""
    if codigo is None or not str(codigo).strip():
        return None, "invalido"
    c = str(codigo).strip()
    if c[:3].upper() == "RY-":
        c = c[3:].strip()
    if not c or not re.fullmatch(r"[A-Za-z0-9_-]+", c):
        return None, "invalido"
    return (c, "elegible") if "R" in c.upper() else (c, "excluido")


def es_titulo_provisional(titulo, codigo, codigo_busqueda):
    t = (titulo or "").strip()
    if not t or re.fullmatch(r"RY[\s_-]+\S+", t, re.I):
        return True
    return t.upper() in {str(codigo or "").strip().upper(), (codigo_busqueda or "").upper()}


# ---------------------------------------------------------------- DB (lectura)
def conectar_ro(db_path):
    return sqlite3.connect(Path(db_path).resolve().as_uri() + "?mode=ro", uri=True)


def leer_candidatos(db_path):
    """Registros sin descripción, clasificados. No lee BLOBs."""
    con = conectar_ro(db_path)
    try:
        total = con.execute("SELECT count(*) FROM juegos").fetchone()[0]
        filas = con.execute(
            "SELECT id, codigo, titulo, (imagen IS NULL OR length(imagen) = 0) "
            f"FROM juegos WHERE {VACIA} ORDER BY id").fetchall()
    finally:
        con.close()
    regs = []
    for id_, codigo, titulo, sin_img in filas:
        busq, clase = clasificar_codigo(codigo)
        regs.append({"id": id_, "codigo": codigo, "codigo_busqueda": busq, "clase": clase,
                     "titulo": titulo or "", "sin_imagen": bool(sin_img),
                     "titulo_provisional": es_titulo_provisional(titulo, codigo, busq)})
    return total, regs


# ---------------------------------------------------------------- HTTP
class Bloqueado(Exception):
    """El sitio limita o bloquea: se detiene todo el proceso."""


class ErrorHttp(Exception):
    pass


class Cancelado(Exception):
    pass


class Cliente:
    """Solicitudes secuenciales con intervalo, timeout y reintentos limitados."""

    def __init__(self, intervalo=3.0, timeout=20, reintentos=2, cancel=None, abrir=None):
        self.intervalo, self.timeout, self.reintentos = intervalo, timeout, reintentos
        self.cancel = cancel or threading.Event()
        self.abrir = abrir or urllib.request.urlopen
        self._ultimo = 0.0
        self.solicitudes = 0

    def _esperar(self, segundos):
        if self.cancel.wait(max(0.0, segundos)):
            raise Cancelado()

    def get(self, url, max_bytes=IMG_MAX_BYTES):
        for intento in range(self.reintentos + 1):
            self._esperar(self._ultimo + self.intervalo - time.monotonic())
            self._ultimo = time.monotonic()
            self.solicitudes += 1
            try:
                req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
                with self.abrir(req, timeout=self.timeout) as r:
                    datos = r.read(max_bytes + 1)
                if len(datos) > max_bytes:
                    raise ErrorHttp(f"respuesta mayor de {max_bytes} bytes")
                if b"Just a moment" in datos[:4096] and b"<html" in datos[:4096].lower():
                    raise Bloqueado("desafío anti-bot (Cloudflare)")
                return datos
            except urllib.error.HTTPError as e:
                if e.code in (403, 429, 503):
                    raise Bloqueado(f"HTTP {e.code}") from e
                if e.code < 500 or intento == self.reintentos:
                    raise ErrorHttp(f"HTTP {e.code}") from e
            except (urllib.error.URLError, http.client.HTTPException, OSError) as e:
                if intento == self.reintentos:
                    raise ErrorHttp(f"conexión: {getattr(e, 'reason', e)}") from e
            log.warning("reintento %d: %s", intento + 1, url)
            self._esperar(self.intervalo * 2 ** intento)
        raise AssertionError("inalcanzable")


def url_busqueda(codigo):
    q = urllib.parse.urlencode({
        "search": codigo, "per_page": 10, "_embed": "wp:featuredmedia",
        "_fields": "id,link,slug,title,content,featured_media,_links,_embedded"})
    return f"{BASE}/wp-json/wp/v2/posts?{q}"


# ---------------------------------------------------------------- validación y extracción
def _contiene_codigo(texto, codigo):
    return re.search(rf"(?<![a-z0-9]){re.escape(codigo.lower())}(?![a-z0-9])", (texto or "").lower()) is not None


def validar(codigo, posts):
    """Devuelve (validación, post | None, motivo)."""
    if not posts:
        return "no_encontrado", None, "la búsqueda no devolvió resultados"
    titulo = lambda p: html.unescape((p.get("title") or {}).get("rendered", ""))
    buenos = [p for p in posts if _contiene_codigo(p.get("slug"), codigo) or _contiene_codigo(titulo(p), codigo)]
    if buenos and not re.search(r"\d{5,}", codigo):
        return "dudosa", None, "código demasiado corto (menos de 5 dígitos): no identifica un juego de forma fiable"
    if len(buenos) == 1:
        return "verificada", buenos[0], ""
    if len(buenos) > 1:
        # mismo juego publicado en japonés y en inglés: se prefiere la única entrada [ENG]
        eng = [p for p in buenos if re.match(r"\s*\[ENG\]", titulo(p), re.I)]
        if len(eng) == 1:
            return "verificada", eng[0], f"{len(buenos)} entradas con el código; se eligió la versión [ENG]"
        return "dudosa", None, f"{len(buenos)} entradas con el código en título/URL"
    if any(_contiene_codigo((p.get("content") or {}).get("rendered"), codigo) for p in posts):
        return "dudosa", None, "el código solo aparece en el contenido, no en título ni URL"
    return "no_encontrado", None, f"{len(posts)} resultado(s) sin el código"


def limpiar_titulo(raw):
    """Quita [ENG] y similares, versiones «(V1.2)» y códigos «(RJ0123, RA45)»."""
    t = html.unescape(raw or "")
    t = re.sub(r"\[[^\]]*\]", " ", t)
    t = re.sub(r"\(\s*v(?:er(?:sion)?)?\.?\s*\d[^)]*\)", " ", t, flags=re.I)
    t = re.sub(r"\(\s*R[A-Z]?\d{3,}(?:\s*[,/&;-]\s*R[A-Z]?\d{3,})*\s*\)", " ", t, flags=re.I)
    t = re.sub(r"\s+", " ", t).strip(" -–|")
    return t if re.search(r"\w", t) else ""  # solo etiquetas: no se propone título


class _Descripcion(HTMLParser):
    FIN = {"h1", "h2", "h3", "h4", "h5", "h6", "script", "iframe"}
    SALTO = {"p", "br", "div", "li"}

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.en_titulo = 0
        self.estado = "antes"   # antes → dentro → fin
        self.partes = []

    def handle_starttag(self, tag, attrs):
        if self.estado == "antes" and tag in {"h1", "h2", "h3", "h4", "h5", "h6", "strong", "b"}:
            self.en_titulo += 1
        elif self.estado == "dentro":
            # las capturas siguen a la descripción: una imagen la cierra si ya hay texto
            if (tag in self.FIN or "ryuu-sl" in (dict(attrs).get("class") or "")
                    or (tag == "img" and "".join(self.partes).strip())):
                self.estado = "fin"
            elif tag in self.SALTO:
                self.partes.append(" ")

    def handle_endtag(self, tag):
        if self.estado == "antes" and self.en_titulo and tag in {"h1", "h2", "h3", "h4", "h5", "h6", "strong", "b"}:
            self.en_titulo -= 1

    def handle_data(self, data):
        if self.estado == "antes" and self.en_titulo and re.fullmatch(r"DESCRIPTION\s*:?", data.strip(), re.I):
            self.estado = "dentro"
        elif self.estado == "dentro":
            if re.match(r"\s*LINK DOWNLOAD", data):  # a veces es un <span>, no un encabezado
                self.estado = "fin"
            else:
                self.partes.append(data)


def recortar(texto, n=DESC_MAX):
    if len(texto) <= n:
        return texto
    corte = texto[:n]
    if texto[n] != " " and " " in corte:
        corte = corte.rsplit(" ", 1)[0]
    return corte.rstrip(" ,;:-")


def extraer_descripcion(contenido_html):
    p = _Descripcion()
    p.feed(contenido_html or "")
    texto = re.sub(r"\s+", " ", "".join(p.partes)).strip()
    return recortar(texto)


def url_imagen(post):
    """Portada destacada; si no hay, la primera imagen wp-image del contenido."""
    for m in (post.get("_embedded") or {}).get("wp:featuredmedia") or []:
        if isinstance(m, dict) and str(m.get("source_url", "")).startswith("https://"):
            return m["source_url"]
    h = (post.get("content") or {}).get("rendered") or ""
    for tag in re.findall(r"<img\b[^>]*>", h):
        if re.search(r"\bwp-image-\d+", tag):
            m = re.search(r"""\bsrc=["'](https://[^"']+)["']""", tag)
            if m:
                return html.unescape(m.group(1))
    return ""


# ---------------------------------------------------------------- imágenes
def a_webp(datos):
    """Valida la imagen y la convierte como el legacy: 300×300, fondo blanco, WebP q65."""
    from PIL import Image
    Image.open(io.BytesIO(datos)).verify()
    img = Image.open(io.BytesIO(datos))
    if img.mode in ("RGBA", "LA") or (img.mode == "P" and "transparency" in img.info):
        img = img.convert("RGBA")
        fondo = Image.new("RGB", img.size, (255, 255, 255))
        fondo.paste(img, mask=img.split()[3])
        img = fondo
    else:
        img = img.convert("RGB")
    img.thumbnail(IMG_MAX, Image.LANCZOS)
    buf = io.BytesIO()
    img.save(buf, "WEBP", quality=IMG_CALIDAD, method=6)
    return buf.getvalue()


def ruta_cache(carpeta, codigo, url):
    ext = Path(urllib.parse.urlparse(url).path).suffix.lower()
    ext = ext if ext in {".jpg", ".jpeg", ".png", ".webp", ".gif"} else ".img"
    seguro = re.sub(r"[^A-Za-z0-9_-]", "_", codigo)
    return Path(carpeta) / "cache" / f"{seguro}{ext}"


def descargar_imagen(cliente, url, destino):
    """Descarga a la caché si no hay ya una copia válida. Devuelve la ruta."""
    destino = Path(destino)
    if destino.exists():
        try:
            a_webp(destino.read_bytes())
            return destino
        except Exception:
            log.warning("caché inválida, se descarga de nuevo: %s", destino)
    datos = cliente.get(url)
    a_webp(datos)  # lanza si no es una imagen válida; no se guarda nada
    destino.parent.mkdir(parents=True, exist_ok=True)
    tmp = destino.with_suffix(destino.suffix + ".tmp")
    tmp.write_bytes(datos)
    os.replace(tmp, destino)
    return destino


# ---------------------------------------------------------------- sesión
def carpeta_trabajo(db_path):
    return Path(db_path).resolve().parent / "recuperacion_ryuugames"


def ruta_sesion(db_path):
    return carpeta_trabajo(db_path) / f"{Path(db_path).stem}_sesion.json"


def cargar_sesion(db_path):
    p = ruta_sesion(db_path)
    return json.loads(p.read_text(encoding="utf-8")) if p.exists() else {}


def guardar_sesion(db_path, sesion):
    p = ruta_sesion(db_path)
    p.parent.mkdir(parents=True, exist_ok=True)
    tmp = p.with_suffix(".tmp")
    tmp.write_text(json.dumps(sesion, ensure_ascii=False, indent=1), encoding="utf-8")
    os.replace(tmp, p)


def seleccionable(r):
    return r.get("validacion") == "verificada" and bool(r.get("descripcion")) and not r.get("aplicado")


# ---------------------------------------------------------------- búsqueda
def procesar_registro(cliente, reg, carpeta):
    """Busca un registro elegible. Nunca toca la DB."""
    r = {"id": reg["id"], "codigo": reg["codigo"], "codigo_busqueda": reg["codigo_busqueda"],
         "titulo_actual": reg["titulo"], "titulo_provisional": reg["titulo_provisional"],
         "sin_imagen": reg["sin_imagen"], "titulo_encontrado": "", "url": "", "descripcion": "",
         "imagen_url": "", "imagen_archivo": "", "estado_busqueda": "", "validacion": "",
         "extraccion": "", "motivo": "", "aplicado": False}
    try:
        posts = json.loads(cliente.get(url_busqueda(reg["codigo_busqueda"]), max_bytes=8 * 1024 * 1024))
        if not isinstance(posts, list):
            raise ValueError("la API no devolvió una lista")
    except ErrorHttp as e:
        r.update(estado_busqueda="error_http", motivo=str(e))
        return r
    except ValueError as e:
        r.update(estado_busqueda="error_parsing", motivo=f"JSON inválido: {e}")
        return r
    r["estado_busqueda"] = "con_resultados" if posts else "sin_resultados"
    r["validacion"], post, r["motivo"] = validar(reg["codigo_busqueda"], posts)
    if not post:
        return r
    try:
        r["url"] = post.get("link", "")
        r["titulo_encontrado"] = limpiar_titulo((post.get("title") or {}).get("rendered", ""))
        r["descripcion"] = extraer_descripcion((post.get("content") or {}).get("rendered", ""))
        r["imagen_url"] = url_imagen(post)
    except Exception as e:  # HTML inesperado: el registro queda intacto
        r.update(estado_busqueda="error_parsing", validacion="", motivo=f"parsing: {e}")
        return r
    faltas = []
    if not r["descripcion"]:
        faltas.append("sin bloque DESCRIPTION")
    if reg["sin_imagen"]:
        if not r["imagen_url"]:
            faltas.append("sin imagen")
        else:
            try:
                destino = ruta_cache(carpeta, reg["codigo_busqueda"], r["imagen_url"])
                r["imagen_archivo"] = str(descargar_imagen(cliente, r["imagen_url"], destino))
            except ErrorHttp as e:
                faltas.append(f"imagen: {e}")
            except (Bloqueado, Cancelado):
                raise
            except Exception as e:
                faltas.append(f"imagen inválida: {e}")
    r["extraccion"] = "ok" if not faltas else "; ".join(faltas)
    if not r["descripcion"]:
        r["motivo"] = "coincidencia verificada pero sin descripción extraíble"
    return r


FINALES = {"verificada", "dudosa", "no_encontrado"}


def pendientes(regs, sesion, reintentar=False):
    """Elegibles que hay que (re)buscar.

    Se reintentan siempre los errores HTTP/parsing y las verificadas con extracción
    incompleta; las dudosas y no encontradas solo con `reintentar`.
    """
    out = []
    for reg in regs:
        if reg["clase"] != "elegible":
            continue
        prev = sesion.get(str(reg["id"]))
        if prev and prev.get("codigo") == reg["codigo"] and prev.get("validacion") in FINALES:
            if prev["validacion"] == "verificada":
                if prev.get("extraccion") == "ok":
                    continue
            elif not reintentar:
                continue
        out.append(reg)
    return out


def buscar(db_path, cliente, reintentar=False, progreso=None):
    """Dry run: busca los pendientes y guarda la sesión tras cada registro.

    Devuelve (sesion, motivo_parada | None). No modifica la DB.
    """
    carpeta = carpeta_trabajo(db_path)
    _, regs = leer_candidatos(db_path)
    sesion = cargar_sesion(db_path)
    cola = pendientes(regs, sesion, reintentar)
    for i, reg in enumerate(cola, 1):
        if progreso:
            progreso(i - 1, len(cola), reg["codigo_busqueda"])
        try:
            r = procesar_registro(cliente, reg, carpeta)
        except Cancelado:
            return sesion, "cancelado por el usuario"
        except Bloqueado as e:
            log.error("detenido por el sitio: %s", e)
            return sesion, f"el sitio bloqueó o limitó las solicitudes ({e}); proceso detenido"
        log.info("%s → %s %s %s", reg["codigo_busqueda"], r["estado_busqueda"], r["validacion"], r["motivo"])
        sesion[str(reg["id"])] = r
        guardar_sesion(db_path, sesion)
    if progreso:
        progreso(len(cola), len(cola), "")
    return sesion, None


# ---------------------------------------------------------------- backup y aplicación
def crear_backup(db_path):
    """Copia con la API de backup de SQLite + integrity_check.

    Se llama con el bloqueo RESERVED ya tomado (BEGIN IMMEDIATE en otra conexión): nadie
    más puede escribir, así que la copia es exactamente el estado al que se aplicarán los
    cambios, y `backup()` (que reintenta sin límite ante BUSY) no puede quedarse esperando.
    """
    db_path = Path(db_path).resolve()
    carpeta = db_path.parent / "backups"
    carpeta.mkdir(exist_ok=True)
    base = f"{db_path.name}.bak-{datetime.now():%Y%m%d-%H%M%S}-ryuugames"
    destino, n = carpeta / base, 1
    while destino.exists():
        destino, n = carpeta / f"{base}-{n}", n + 1
    src, dst = conectar_ro(db_path), sqlite3.connect(destino)
    try:
        src.backup(dst)
        ok = dst.execute("PRAGMA integrity_check").fetchone()[0]
    finally:
        dst.close()
        src.close()
    if ok != "ok":
        raise RuntimeError(f"el backup no pasó integrity_check: {ok}")
    return destino


def aplicar(db_path, sesion, ids, reemplazar_titulo=()):
    """Aplica los registros `ids` (solo los seleccionables) en una transacción.

    Devuelve dict con backup, actualizados, omitidos (id → motivo), imagenes, imagenes_fallidas.
    """
    elegidos = [sesion[str(i)] for i in ids if str(i) in sesion and seleccionable(sesion[str(i)])]
    res = {"backup": None, "actualizados": [], "omitidos": {}, "imagenes": 0, "imagenes_fallidas": 0}
    if not elegidos:
        return res
    webps = {}
    for r in elegidos:  # conversión fuera de la transacción
        if r.get("imagen_archivo"):
            try:
                webps[r["id"]] = a_webp(Path(r["imagen_archivo"]).read_bytes())
            except Exception as e:
                log.warning("imagen %s no aplicable: %s", r["codigo_busqueda"], e)
                res["imagenes_fallidas"] += 1
    reemplazar = {int(i) for i in reemplazar_titulo}
    con = sqlite3.connect(db_path, isolation_level=None, timeout=5)
    try:
        con.execute("BEGIN IMMEDIATE")  # si la DB está bloqueada, el error real llega intacto
    except BaseException:
        con.close()
        raise
    try:
        res["backup"] = str(crear_backup(db_path))
        for r in elegidos:
            fila = con.execute(f"SELECT titulo, {VACIA} FROM juegos WHERE id=? AND codigo IS ?",
                               (r["id"], r["codigo"])).fetchone()
            if not fila:
                res["omitidos"][r["id"]] = "el registro ya no existe o cambió su código"
                continue
            if not fila[1]:
                res["omitidos"][r["id"]] = "la descripción ya no está vacía"
                continue
            con.execute(f"UPDATE juegos SET descripcion=? WHERE id=? AND {VACIA}", (r["descripcion"], r["id"]))
            autorizado = r["id"] in reemplazar and fila[0] == r["titulo_actual"]  # no si se editó entretanto
            if r["titulo_encontrado"] and (autorizado or
                                           es_titulo_provisional(fila[0], r["codigo"], r["codigo_busqueda"])):
                con.execute("UPDATE juegos SET titulo=? WHERE id=?", (r["titulo_encontrado"], r["id"]))
            if r["id"] in webps:
                cur = con.execute("UPDATE juegos SET imagen=? WHERE id=? AND (imagen IS NULL OR length(imagen)=0)",
                                  (webps[r["id"]], r["id"]))
                res["imagenes"] += cur.rowcount
            res["actualizados"].append(r["id"])
        con.execute("COMMIT")
    except BaseException:
        con.execute("ROLLBACK")
        raise
    finally:
        con.close()
    for i in res["actualizados"]:
        sesion[str(i)]["aplicado"] = True
    guardar_sesion(db_path, sesion)
    return res


# ---------------------------------------------------------------- informes
CSV_CAMPOS = [("codigo", "Código original"), ("codigo_busqueda", "Código utilizado"),
              ("titulo_actual", "Título original"), ("titulo_encontrado", "Título recuperado"),
              ("url", "URL"), ("estado_busqueda", "Estado de búsqueda"), ("validacion", "Estado de validación"),
              ("extraccion", "Resultado de la extracción"), ("motivo", "Motivo de omisión o error"),
              ("aplicado", "Aplicado")]


def filas_informe(regs, sesion):
    """Una fila por registro sin descripción: excluidos/inválidos + resultados de sesión."""
    filas = []
    for reg in regs:
        if reg["clase"] != "elegible":
            filas.append({"codigo": reg["codigo"], "codigo_busqueda": reg["codigo_busqueda"] or "",
                          "titulo_actual": reg["titulo"], "estado_busqueda": "no_buscado",
                          "validacion": reg["clase"],
                          "motivo": "código inválido" if reg["clase"] == "invalido" else "sin «R» tras quitar RY-"})
        else:
            filas.append(sesion.get(str(reg["id"]), {"codigo": reg["codigo"], "codigo_busqueda": reg["codigo_busqueda"],
                                                     "titulo_actual": reg["titulo"], "estado_busqueda": "pendiente"}))
    return filas


def resumen(total, regs, sesion, aplicacion=None):
    rs = [sesion[str(r["id"])] for r in regs if str(r["id"]) in sesion]
    cuenta = lambda f: sum(1 for r in rs if f(r))
    ap = aplicacion or {}
    return {
        "Total de registros": total,
        "Registros sin descripción": len(regs),
        "Registros elegibles": sum(r["clase"] == "elegible" for r in regs),
        "Excluidos por no contener R": sum(r["clase"] == "excluido" for r in regs),
        "Código inválido": sum(r["clase"] == "invalido" for r in regs),
        "Procesados": len(rs),
        "Coincidencias verificadas": cuenta(lambda r: r.get("validacion") == "verificada"),
        "Coincidencias dudosas": cuenta(lambda r: r.get("validacion") == "dudosa"),
        "No encontrados": cuenta(lambda r: r.get("validacion") == "no_encontrado"),
        "Errores HTTP": cuenta(lambda r: r.get("estado_busqueda") == "error_http"),
        "Errores de parsing": cuenta(lambda r: r.get("estado_busqueda") == "error_parsing"),
        "Imágenes descargadas": cuenta(lambda r: bool(r.get("imagen_archivo"))),
        "Imágenes fallidas": cuenta(lambda r: "imagen" in (r.get("extraccion") or "")),
        "Registros actualizados (total en sesión)": cuenta(lambda r: r.get("aplicado")),
        "Actualizados en esta aplicación": len(ap.get("actualizados", [])),
        "Omitidos en esta aplicación": len(ap.get("omitidos", {})),
    }


def escribir_informes(db_path, total, regs, sesion, aplicacion=None):
    carpeta = carpeta_trabajo(db_path)
    carpeta.mkdir(parents=True, exist_ok=True)
    sello = f"{datetime.now():%Y%m%d-%H%M%S}"
    csv_path, txt_path = carpeta / f"informe_{sello}.csv", carpeta / f"informe_{sello}.txt"
    with open(csv_path, "w", newline="", encoding="utf-8-sig") as f:
        w = csv.writer(f)
        w.writerow([t for _, t in CSV_CAMPOS])
        for fila in filas_informe(regs, sesion):
            w.writerow([fila.get(k, "") for k, _ in CSV_CAMPOS])
    lineas = [f"{k}: {v}" for k, v in resumen(total, regs, sesion, aplicacion).items()]
    if aplicacion and aplicacion.get("backup"):
        lineas.append(f"Backup: {aplicacion['backup']}")
    txt_path.write_text("\n".join(lineas) + "\n", encoding="utf-8")
    return csv_path, txt_path


def configurar_log(db_path):
    carpeta = carpeta_trabajo(db_path)
    carpeta.mkdir(parents=True, exist_ok=True)
    for h in list(log.handlers):
        log.removeHandler(h)
        h.close()
    h = logging.FileHandler(carpeta / f"ryuugames_{datetime.now():%Y%m%d}.log", encoding="utf-8")
    h.setFormatter(logging.Formatter("%(asctime)s %(levelname)s %(message)s"))
    log.addHandler(h)
    log.setLevel(logging.INFO)
    return h.baseFilename
