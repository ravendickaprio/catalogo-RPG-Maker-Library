#!/usr/bin/env python
"""Prueba de coexistencia Python <-> app Rust sobre una COPIA temporal (ADR-018).

Uso (solo biblioteca estándar):
    python -I tools/interop_test.py [--db dev-data/juegos.dev.db] [--no-build] [--keep]

Qué hace (nunca toca la DB real: rechaza `juegos.db` y trabaja sobre una copia en un
directorio temporal):

  A. Python escribe con las sentencias EXACTAS de los programas legacy y la app Rust lo lee
       - `save()` de catalogo_rpgmaker-editor-completo.py (UPDATE + DELETE/INSERT de tags)
       - `crear` / `guardar_tags` / `eliminar` de catalogo_rpgmaker-v04.py (clase BaseDatos)
  B. La app Rust escribe (`write::create_game/update_game/set_favorite/delete_game`) y Python
       lee con las consultas legacy (`listar`, `obtener`, `tags_juego` de v04).
  C. Concurrencia: Python retiene BEGIN IMMEDIATE / BEGIN EXCLUSIVE y la app intenta escribir
       (debe devolver Busy en ~5 s, sin corrupción); y al revés: la sonda Rust retiene una
       transacción de escritura y Python intenta escribir (debe fallar con "database is locked").
  D. `PRAGMA journal_mode` sigue en `delete` y no aparecen archivos -wal/-shm.
  E. La DB de origen queda intacta (SHA-256) y una apertura de solo lectura no modifica la copia.

La parte Rust es el ejemplo `desktop/src-tauri/examples/interop_probe.rs` (se compila con
`cargo build --release --example interop_probe`). Importar con el importador de la app no se
cubre aquí: el importador inserta con las mismas sentencias que `write::create_game`.
Código de salida: 0 si todo PASS, 1 si algo FALLA.
"""
import argparse
import hashlib
import json
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TAURI = ROOT / "desktop" / "src-tauri"
EXE = ".exe" if sys.platform.startswith("win") else ""
PROBE = TAURI / "target" / "release" / "examples" / f"interop_probe{EXE}"

RESULTS = []  # (grupo, nombre, ok, detalle)


def check(group, name, ok, detail=""):
    RESULTS.append((group, name, bool(ok), str(detail)))
    return bool(ok)


def sha256(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def probe(*args, timeout=60):
    r = subprocess.run([str(PROBE), *map(str, args)], capture_output=True, timeout=timeout)
    out = r.stdout.decode("utf-8", "replace").strip()
    try:
        return json.loads(out.splitlines()[-1])
    except Exception:
        return {"ok": False, "error": {"kind": "ProbeCrash",
                                       "message": (out + r.stderr.decode("utf-8", "replace"))[:400]}}


# ---------------------------------------------------------------------------
# Réplica fiel de las sentencias legacy
# ---------------------------------------------------------------------------
class Legacy:
    """Sentencias copiadas de catalogo_rpgmaker-v04.py (clase BaseDatos, L186-L306)."""

    def __init__(self, path):
        self.con = sqlite3.connect(path)          # v04 L192 (timeout por defecto = 5 s)
        self.con.execute("PRAGMA foreign_keys = ON")  # v04 L233

    # v04 L236-240
    def listar(self):
        return self.con.execute(
            "SELECT id, titulo, descripcion, estado, idioma, ruta, codigo FROM juegos").fetchall()

    # v04 L242-247
    def obtener(self, jid):
        return self.con.execute(
            "SELECT id, titulo, descripcion, estado, idioma, ruta, codigo, imagen, "
            "pagina, tipo, puntuacion FROM juegos WHERE id = ?", (jid,)).fetchone()

    # v04 L249-252
    def tags_juego(self, jid):
        return [r[0] for r in self.con.execute(
            "SELECT t.nombre FROM tags t JOIN juego_tags jt ON jt.tag_id=t.id "
            "WHERE jt.juego_id=? ORDER BY t.nombre COLLATE NOCASE", (jid,))]

    # v04 L258-269
    def guardar_tags(self, jid, nombres):
        self.con.execute("DELETE FROM juego_tags WHERE juego_id=?", (jid,))
        for nombre in nombres:
            nombre = nombre.strip()
            if not nombre:
                continue
            self.con.execute("INSERT OR IGNORE INTO tags(nombre) VALUES (?)", (nombre,))
            tag_id = self.con.execute(
                "SELECT id FROM tags WHERE nombre=? COLLATE NOCASE", (nombre,)).fetchone()[0]
            self.con.execute("INSERT OR IGNORE INTO juego_tags(juego_id,tag_id) VALUES (?,?)",
                             (jid, tag_id))
        self.con.commit()

    # v04 L283-292
    def crear(self, titulo, descripcion, estado, idioma, ruta, codigo, imagen,
              pagina, tipo, puntuacion):
        cur = self.con.execute(
            "INSERT INTO juegos (titulo, descripcion, estado, idioma, ruta, codigo, imagen, "
            "pagina, tipo, puntuacion) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (titulo, descripcion, estado, idioma, ruta, codigo, imagen, pagina, tipo, puntuacion))
        self.con.commit()
        return cur.lastrowid

    # v04 L303-306
    def eliminar(self, jid):
        self.con.execute("DELETE FROM juego_tags WHERE juego_id = ?", (jid,))
        self.con.execute("DELETE FROM juegos WHERE id = ?", (jid,))
        self.con.commit()

    # editor-completo L65-71 (`save`), sin Tk: mismas sentencias y orden
    def editor_save(self, jid, title, desc, estado, idioma, ruta, codigo, imgbytes,
                    pagina, tipo, puntos, tags_text):
        try:
            vals = (title, desc, estado, idioma, ruta, codigo, imgbytes, pagina, tipo, float(puntos))
            self.con.execute(
                "UPDATE juegos SET titulo=?,descripcion=?,estado=?,idioma=?,ruta=?,codigo=?,"
                "imagen=?,pagina=?,tipo=?,puntuacion=? WHERE id=?", (*vals, jid))
            self.con.execute("DELETE FROM juego_tags WHERE juego_id=?", (jid,))
            for tag in dict.fromkeys(x.strip() for x in tags_text.split(",") if x.strip()):
                self.con.execute("INSERT OR IGNORE INTO tags(nombre) VALUES(?)", (tag,))
                tid = self.con.execute(
                    "SELECT id FROM tags WHERE nombre=? COLLATE NOCASE", (tag,)).fetchone()[0]
                self.con.execute("INSERT OR IGNORE INTO juego_tags(juego_id,tag_id) VALUES(?,?)",
                                 (jid, tid))
            self.con.commit()
        except Exception:
            self.con.rollback()
            raise


def journal_mode(path):
    c = sqlite3.connect(path)
    try:
        return c.execute("PRAGMA journal_mode").fetchone()[0]
    finally:
        c.close()


def integrity(path):
    c = sqlite3.connect(path)
    try:
        return c.execute("PRAGMA integrity_check").fetchone()[0]
    finally:
        c.close()


def sidecars(db):
    return [p.name for p in db.parent.iterdir() if p.name.startswith(db.name + "-")]


GAME = lambda **kw: {  # GameInput (camelCase) para la sonda
    "titulo": "x", "descripcion": "", "estado": "Sin empezar", "idioma": "Español",
    "ruta": "", "codigo": "", "pagina": "", "tipo": "Otro", "puntuacion": 0.0, "tags": [], **kw}


# ---------------------------------------------------------------------------
def part_a(db):
    g = "A"
    leg = Legacy(db)
    # --- A1: editor-completo save() sobre un juego existente (conserva y reemplaza imagen)
    row = leg.con.execute(
        "SELECT id FROM juegos WHERE imagen IS NOT NULL ORDER BY id LIMIT 1 OFFSET 7").fetchone()
    jid = row[0]
    blob = leg.obtener(jid)[7]
    titulo = "Interop Ñandú Edición ZXQ1"
    desc = "Descripción editada desde Python — línea 1\nlínea 2 ¡ü!"
    leg.editor_save(jid, titulo, desc, "Empezado", "Inglés", r"C:\Juegos\Interop ZXQ1",
                    "RY-ZXQ9001", blob, "ryuugames", "RPG", "3.5",
                    "Interop, Favorito , interop, ZxqTag")
    r = probe("read", db, jid, "ZXQ1")
    gm = r.get("game") or {}
    check(g, "A1 editor save(): campos leídos por Rust", r.get("ok") and
          (gm.get("titulo"), gm.get("descripcion"), gm.get("estado"), gm.get("idioma"),
           gm.get("ruta"), gm.get("codigo"), gm.get("pagina"), gm.get("tipo"), gm.get("puntuacion"))
          == (titulo, desc, "Empezado", "Inglés", r"C:\Juegos\Interop ZXQ1", "RY-ZXQ9001",
              "ryuugames", "RPG", 3.5), json.dumps(gm, ensure_ascii=False)[:200])
    check(g, "A1 tags (dedupe NOCASE, orden NOCASE) y portada intacta",
          gm.get("tags") == ["Favorito", "Interop", "ZxqTag"] and gm.get("hasCover") is True,
          gm.get("tags"))
    check(g, "A1 búsqueda en la app encuentra el id editado",
          jid in (r.get("search") or {}).get("ids", []), r.get("search"))
    ix = r.get("indexStatus", {})
    check(g, "A1 índice de búsqueda sincronizado (triggers)", ix.get("mismatched") == 0 and ix.get("ftsOk") is True, ix)
    r2 = probe("read", db, jid, "ZxqTag")
    check(g, "A1 búsqueda por tag nuevo", jid in (r2.get("search") or {}).get("ids", []), r2.get("search"))

    # --- A2: v04 crear() + guardar_tags() con portada
    cover = b"RIFF\x24\x00\x00\x00WEBPVP8 " + bytes(range(40))
    nid = leg.crear("Juego Nuevo Python QWK2", "desc python", "Sin empezar", "Español",
                    "", "RY-QWK2002", cover, "Elonichan", "Visual Novel", 2.0)
    leg.guardar_tags(nid, ["QwkTag", "Favorito"])
    r = probe("read", db, nid, "QWK2")
    gm = r.get("game") or {}
    check(g, "A2 v04 crear(): leído por Rust",
          r.get("ok") and gm.get("titulo") == "Juego Nuevo Python QWK2"
          and gm.get("codigo") == "RY-QWK2002" and gm.get("tipo") == "Visual Novel"
          and gm.get("hasCover") is True and gm.get("tags") == ["Favorito", "QwkTag"],
          json.dumps(gm, ensure_ascii=False)[:200])
    check(g, "A2 búsqueda encuentra el juego creado", nid in (r.get("search") or {}).get("ids", []), r.get("search"))
    check(g, "A2 índice sincronizado", r["indexStatus"]["mismatched"] == 0 and r["indexStatus"]["ftsOk"], r["indexStatus"])

    # --- A3: v04 eliminar()
    leg.eliminar(nid)
    r = probe("read", db, nid, "QWK2")
    check(g, "A3 v04 eliminar(): Rust no encuentra el juego ni en búsqueda",
          r.get("game") is None and nid not in (r.get("search") or {}).get("ids", [])
          and r["indexStatus"]["mismatched"] == 0, r.get("search"))
    orphans = leg.con.execute(
        "SELECT count(*) FROM juego_tags WHERE juego_id NOT IN (SELECT id FROM juegos)").fetchone()[0]
    check(g, "A3 sin juego_tags huérfanos", orphans == 0, orphans)
    leg.con.close()
    return jid, blob


def part_b(db, a_jid, a_blob):
    g = "B"
    leg = Legacy(db)
    n0 = len(leg.listar())
    # --- B1: Rust create_game -> Python listar/obtener/tags_juego
    inp = GAME(titulo="Creado por Rust ÁÉÍ Q7", descripcion="desde la app", estado="Empezado",
               idioma="Japonés", ruta="/home/x/q7", codigo="RY-RST7007", pagina="Estahi0",
               tipo="RPG", puntuacion=4.5, tags=["RustTag", "Favorito"])
    r = probe("create", db, json.dumps(inp, ensure_ascii=False))
    nid = (r.get("game") or {}).get("id")
    check(g, "B1 Rust create_game ok", r.get("ok") and nid, r.get("error"))
    if not nid:
        return
    rows = {x[0]: x for x in leg.listar()}
    check(g, "B1 Python listar(): +1 fila y datos", len(rows) == n0 + 1 and rows.get(nid)
          and rows[nid][1:7] == ("Creado por Rust ÁÉÍ Q7", "desde la app", "Empezado", "Japonés", "/home/x/q7", "RY-RST7007"),
          rows.get(nid))
    o = leg.obtener(nid)
    check(g, "B1 Python obtener(): pagina/tipo/puntuacion, imagen NULL",
          o and o[7] is None and o[8:] == ("Estahi0", "RPG", 4.5), o and (o[7], o[8:]))
    check(g, "B1 Python tags_juego()", leg.tags_juego(nid) == ["Favorito", "RustTag"], leg.tags_juego(nid))

    # --- B2: Rust update_game conserva la portada de un juego con imagen
    cur = leg.obtener(a_jid)
    upd = GAME(titulo=cur[1] + " (Rust)", descripcion=cur[2] + " +rust", estado="Terminado",
               idioma=cur[4], ruta=cur[5] or "", codigo=cur[6] or "", pagina=cur[8] or "",
               tipo=cur[9] or "Otro", puntuacion=5.0, tags=["Interop", "OtroTag"])
    r = probe("update", db, a_jid, json.dumps(upd, ensure_ascii=False))
    check(g, "B2 Rust update_game ok", r.get("ok"), r.get("error"))
    o = leg.obtener(a_jid)
    check(g, "B2 Python obtener(): campos nuevos y portada INTACTA (mismos bytes)",
          o[1] == upd["titulo"] and o[3] == "Terminado" and o[10] == 5.0 and o[7] == a_blob,
          (o[1], o[3], o[10], o[7] == a_blob))
    check(g, "B2 Python tags_juego() tras update", leg.tags_juego(a_jid) == ["Interop", "OtroTag"], leg.tags_juego(a_jid))

    # --- B3: Rust set_favorite true/false
    r = probe("favorite", db, nid, "false")
    check(g, "B3 set_favorite(false): Python ya no ve 'Favorito'",
          r.get("ok") and "Favorito" not in leg.tags_juego(nid) and "RustTag" in leg.tags_juego(nid),
          leg.tags_juego(nid))
    r = probe("favorite", db, nid, "true")
    check(g, "B3 set_favorite(true): Python ve 'Favorito'", r.get("ok") and "Favorito" in leg.tags_juego(nid), leg.tags_juego(nid))

    # --- B4: Python edita y luego Rust (revisión nueva) edita sin conflicto; Python vuelve a leer
    leg.editor_save(nid, "Creado por Rust ÁÉÍ Q7", "editado en Python", "Terminado", "Japonés",
                    "/home/x/q7", "RY-RST7007", None, "Estahi0", "RPG", "1.0", "Favorito")
    r = probe("favorite", db, nid, "false")
    check(g, "B4 Rust escribe tras edición en Python (sin Conflict)", r.get("ok"), r.get("error"))
    check(g, "B4 Python ve ambos cambios", leg.obtener(nid)[2] == "editado en Python" and leg.tags_juego(nid) == [],
          (leg.obtener(nid)[2], leg.tags_juego(nid)))

    # --- B5: Rust delete_game -> Python
    r = probe("delete", db, nid)
    check(g, "B5 Rust delete_game: Python no encuentra la fila ni tags huérfanos",
          r.get("ok") and leg.obtener(nid) is None and
          leg.con.execute("SELECT count(*) FROM juego_tags WHERE juego_id=?", (nid,)).fetchone()[0] == 0,
          r.get("error"))
    leg.con.close()
    check(g, "B integrity_check tras escrituras cruzadas", integrity(db) == "ok", integrity(db))


def part_c(db):
    g = "C"
    jid = sqlite3.connect(db).execute("SELECT min(id) FROM juegos").fetchone()[0]
    for mode in ("IMMEDIATE", "EXCLUSIVE"):
        holder = sqlite3.connect(db, isolation_level=None)
        holder.execute(f"BEGIN {mode}")
        holder.execute("UPDATE juegos SET descripcion = descripcion WHERE id = ?", (jid,))
        t = time.time()
        r = probe("busytry", db, jid, timeout=60)
        el = time.time() - t
        holder.execute("ROLLBACK")
        holder.close()
        err = r.get("error") or {}
        if mode == "IMMEDIATE":
            check(g, "C1 Python BEGIN IMMEDIATE: Rust devuelve Busy en ~5 s",
                  r.get("ok") is False and err.get("kind") == "Busy" and 4.0 <= el <= 9.0,
                  f"kind={err.get('kind')} t={el:.1f}s")
        else:
            # Con EXCLUSIVE (modo delete) incluso las lecturas esperan: la apertura ya falla.
            check(g, "C2 Python BEGIN EXCLUSIVE: la app falla (Busy/lock) sin escribir ni colgarse",
                  r.get("ok") is False and el <= 15,
                  f"kind={err.get('kind')} t={el:.1f}s msg={err.get('message', '')[:90]}")
        check(g, f"C {mode}: integrity_check ok y journal_mode=delete tras el bloqueo",
              integrity(db) == "ok" and journal_mode(db) == "delete", "")
    # tras liberar, la app escribe con normalidad
    r = probe("favorite", db, jid, "true")
    check(g, "C3 tras liberar el bloqueo la app escribe normalmente", r.get("ok"), r.get("error"))
    probe("favorite", db, jid, "false")

    # --- C4: al revés. La sonda retiene BEGIN IMMEDIATE y Python intenta escribir.
    p = subprocess.Popen([str(PROBE), "hold", str(db), "8"], stdout=subprocess.PIPE)
    first = p.stdout.readline().decode().strip()
    ok_locked = False
    msg = ""
    if '"held":true' in first.replace(" ", ""):
        c = sqlite3.connect(db, timeout=1)
        t = time.time()
        try:
            c.execute("UPDATE juegos SET descripcion = descripcion WHERE id = ?", (jid,))
            c.commit()
        except sqlite3.OperationalError as e:
            ok_locked, msg = "locked" in str(e), str(e)
        c.close()
        el = time.time() - t
    else:
        el = 0
        msg = f"la sonda no retuvo el bloqueo: {first!r}"
    p.wait(timeout=30)
    check(g, "C4 Rust retiene transacción de escritura: Python falla con 'database is locked'",
          ok_locked and el < 4, f"{msg} t={el:.1f}s")
    c = sqlite3.connect(db, timeout=1)
    c.execute("UPDATE juegos SET descripcion = descripcion WHERE id = ?", (jid,))
    c.commit()
    c.close()
    check(g, "C4 liberado el bloqueo, Python escribe; integrity_check ok", integrity(db) == "ok", "")


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser()
    ap.add_argument("--db", default=str(ROOT / "dev-data" / "juegos.dev.db"))
    ap.add_argument("--no-build", action="store_true")
    ap.add_argument("--keep", action="store_true", help="no borrar el directorio temporal")
    a = ap.parse_args()
    src = Path(a.db).resolve()
    if src.name.lower() == "juegos.db":
        sys.exit("Se rechaza juegos.db (base real). Use una copia, p. ej. dev-data/juegos.dev.db.")
    if not src.is_file():
        sys.exit(f"No existe {src}")
    if not a.no_build or not PROBE.exists():
        print("Compilando interop_probe (cargo build --release --example interop_probe)...", flush=True)
        r = subprocess.run(["cargo", "build", "--release", "--example", "interop_probe"], cwd=TAURI)
        if r.returncode != 0:
            sys.exit("Falló la compilación de la sonda.")
    h_src = sha256(src)
    tmp = Path(tempfile.mkdtemp(prefix="rpglib_interop_"))
    db = tmp / "interop_copy.db"
    shutil.copy2(src, db)
    try:
        check("D", "D0 journal_mode inicial = delete", journal_mode(db) == "delete", journal_mode(db))
        # E1: apertura de solo lectura no modifica la copia
        h0 = sha256(db)
        probe("read", db, 1, "a")
        check("E", "E1 lectura de la app no modifica la DB (SHA-256)", sha256(db) == h0, "")
        jid, blob = part_a(db)
        part_b(db, jid, blob)
        part_c(db)
        check("D", "D1 journal_mode final = delete", journal_mode(db) == "delete", journal_mode(db))
        check("D", "D2 sin archivos -wal/-shm/-journal residuales", not sidecars(db), sidecars(db))
        final = probe("read", db, 1)
        check("D", "D3 integrity_check + índice FTS final", integrity(db) == "ok"
              and final["indexStatus"]["mismatched"] == 0 and final["indexStatus"]["ftsOk"], final.get("indexStatus"))
        check("E", "E2 la DB de origen quedó intacta (SHA-256)", sha256(src) == h_src, "")
    finally:
        if a.keep:
            print(f"Directorio temporal conservado: {tmp}")
        else:
            shutil.rmtree(tmp, ignore_errors=True)

    w = max(len(n) for _, n, _, _ in RESULTS)
    print(f"\n{'Prueba'.ljust(w)}  Resultado  Detalle")
    print("-" * (w + 40))
    for gname, n, ok, d in RESULTS:
        print(f"{n.ljust(w)}  {'PASS' if ok else 'FAIL'}       {d[:150] if (not ok or gname == 'C') else ''}")
    bad = [r for r in RESULTS if not r[2]]
    print(f"\n{len(RESULTS) - len(bad)}/{len(RESULTS)} PASS")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
