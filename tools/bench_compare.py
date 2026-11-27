#!/usr/bin/env python
"""Comparativa reproducible de la estrategia de búsqueda legacy frente a la de la app (ADR-018).

Uso (solo biblioteca estándar):
    python -I tools/bench_compare.py [--reps 10] [--dbs dev-data/synth/synth_5000.db dev-data/synth/synth_50000.db]
                                     [--no-gen] [--md salida.md]

* App: ejecuta `cargo run --release --bin bench <dbs...>` (desktop/src-tauri; 5 calentamientos
  + 50 medidas por escenario; SQL del backend, 100 filas por página + recuento total).
* Legacy (réplica FIEL, N+1): cada repetición hace lo que hace la app Tk en `refrescar_lista()`
  de catalogo_rpgmaker-v04.py, sin dibujar widgets:
    - `_filtrar()` (L981-L1022): `db.listar()` (1 SELECT) y, POR CADA FILA, `db.tags_juego(jid)`
      (1 SELECT) y `SELECT pagina, tipo, puntuacion FROM juegos WHERE id=?` (1 SELECT); filtro y
      orden en Python (casefold). Es decir 1 + 2N sentencias.
    - `refrescar_lista()` (L1024-L1047), vista lista: `estadisticas()` (3 SELECT), `todos_tags()`
      (1 SELECT) y, POR CADA FILA del resultado, otro `SELECT pagina, tipo, puntuacion` (L1038).
  Excluye el coste de Tk (insertar filas en el Treeview), que es adicional.
* Legacy «precargado»: la misma lógica de filtro pero con 3 consultas masivas por repetición
  (equivale a `tools/legacy_parity.py`, sin N+1). Sirve para separar el coste del patrón N+1
  del coste de filtrar en Python.
* Abre las DB en solo lectura (`mode=ro`); rechaza `juegos.db`.
* Mide en la misma máquina, mismas DB y mismas consultas. No incluye IPC ni render de ninguno.
"""
import argparse
import json
import platform
import re
import sqlite3
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TAURI = ROOT / "desktop" / "src-tauri"
ESTADOS = ["Sin empezar", "Empezado", "Terminado"]   # v04 L47
IDIOMAS = ["Español", "Inglés", "Japonés"]           # v04 L48

# (nombre del escenario en `bench`, texto, estado, idioma). Mismos filtros en ambos lados;
# el orden es «Nombre (A-Z)» en el legacy y title_asc en la app.
SCENARIOS = [
    ('search empty, sort title, limit 100', "", None, None),
    ('search "the" (fts)', "the", None, None),
    ('search "ry-1" (fts code)', "ry-1", None, None),
    ('search "ryu" (fts, 3 chars)', "ryu", None, None),
    ('search "the dark" (fts, long)', "the dark", None, None),
    ('search "a" (like, 1 char)', "a", None, None),
    ('search "ry" (like, 2 chars)', "ry", None, None),
    ('search "zzzzqqq" (no match)', "zzzzqqq", None, None),
    ("estado='Sin empezar' + idioma='Japonés'", "", "Sin empezar", "Japonés"),
    ('"the" + estado + idioma', "the", "Sin empezar", "Japonés"),
]


def indice(lista, valor):                            # v04 L50-51
    return lista.index(valor) if valor in lista else len(lista)


def normalizar_codigo(texto):                        # v04 L355-357
    return re.sub(r"[\s\-_]", "", (texto or "").casefold())


def pct(v, p):
    v = sorted(v)
    return v[min(len(v) - 1, max(0, int(-(-len(v) * p // 1)) - 1))]


def stats(ms):
    return statistics.median(ms), pct(ms, 0.95), max(ms)


# ------------------------------------------------------------------ legacy fiel (N+1)
class LegacyDB:
    """Sentencias de BaseDatos (v04 L236-L317), conexión de solo lectura."""

    def __init__(self, path):
        self.con = sqlite3.connect(Path(path).resolve().as_uri() + "?mode=ro", uri=True)

    def listar(self):                                  # L236-240
        return self.con.execute(
            "SELECT id, titulo, descripcion, estado, idioma, ruta, codigo FROM juegos").fetchall()

    def tags_juego(self, jid):                         # L249-252
        return [r[0] for r in self.con.execute(
            "SELECT t.nombre FROM tags t JOIN juego_tags jt ON jt.tag_id=t.id "
            "WHERE jt.juego_id=? ORDER BY t.nombre COLLATE NOCASE", (jid,))]

    def todos_tags(self):                              # L254-256
        return [r[0] for r in self.con.execute(
            "SELECT nombre FROM tags ORDER BY nombre COLLATE NOCASE")]

    def estadisticas(self):                            # L308-312
        total = self.con.execute("SELECT COUNT(*) FROM juegos").fetchone()[0]
        por_estado = dict(self.con.execute("SELECT estado, COUNT(*) FROM juegos GROUP BY estado"))
        por_idioma = dict(self.con.execute("SELECT idioma, COUNT(*) FROM juegos GROUP BY idioma"))
        return total, por_estado, por_idioma


def _pasa(texto, texto_norm, f_estado, f_idioma, fila, tags, pagina, tipo):
    jid, titulo, desc, estado, idioma, _, codigo = fila
    campos = " ".join([pagina or "", tipo or "", " ".join(tags)]).casefold()       # L996
    en_codigo = (texto in (codigo or "").casefold()
                 or (texto_norm and texto_norm in normalizar_codigo(codigo)))      # L997-998
    if texto and not (en_codigo or texto in titulo.casefold()
                      or texto in (desc or "").casefold() or texto in campos):     # L999-1001
        return False
    if f_estado != "Todos" and estado != f_estado:                                  # L1002
        return False
    if f_idioma != "Todos" and idioma != f_idioma:                                  # L1004
        return False
    return True


def legacy_refresh(db, texto, estado, idioma):
    """_filtrar() (L981-1022) + parte SQL de refrescar_lista() (L1024-1047), vista lista."""
    texto = texto.strip().casefold()
    texto_norm = normalizar_codigo(texto)
    f_estado, f_idioma = estado or "Todos", idioma or "Todos"
    juegos = []
    for fila in db.listar():                                                         # L989
        jid = fila[0]
        tags = db.tags_juego(jid)                                                    # L991
        pagina, tipo, _punt = db.con.execute(
            "SELECT pagina, tipo, puntuacion FROM juegos WHERE id=?", (jid,)).fetchone()  # L992-995
        if _pasa(texto, texto_norm, f_estado, f_idioma, fila, tags, pagina, tipo):
            juegos.append(fila)
    juegos.sort(key=lambda j: j[1].casefold())                                       # L1021 (Nombre A-Z)
    db.estadisticas()                                                                # L1026
    db.todos_tags()                                                                  # L1027
    for fila in juegos:                                                              # L1036-1040
        db.con.execute("SELECT pagina, tipo, puntuacion FROM juegos WHERE id=?", (fila[0],)).fetchone()
    return len(juegos)


# ------------------------------------------------------------------ legacy precargado
def legacy_preloaded(db, texto, estado, idioma):
    texto = texto.strip().casefold()
    texto_norm = normalizar_codigo(texto)
    f_estado, f_idioma = estado or "Todos", idioma or "Todos"
    filas = db.listar()
    extra = {r[0]: r[1:] for r in db.con.execute("SELECT id, pagina, tipo, puntuacion FROM juegos")}
    tags = {}
    for jid, nombre in db.con.execute(
            "SELECT jt.juego_id, t.nombre FROM juego_tags jt JOIN tags t ON t.id=jt.tag_id "
            "ORDER BY t.nombre COLLATE NOCASE"):
        tags.setdefault(jid, []).append(nombre)
    juegos = [f for f in filas
              if _pasa(texto, texto_norm, f_estado, f_idioma, f, tags.get(f[0], []),
                       extra[f[0]][0], extra[f[0]][1])]
    juegos.sort(key=lambda j: j[1].casefold())
    return len(juegos)


def measure(fn, reps, warmup=1):
    for _ in range(warmup):
        fn()
    out = []
    n = 0
    for _ in range(reps):
        t = time.perf_counter()
        n = fn()
        out.append((time.perf_counter() - t) * 1000)
    return stats(out), n


# ------------------------------------------------------------------ app (bench)
def run_app_bench(dbs):
    cmd = ["cargo", "run", "--release", "--bin", "bench", "--", *map(str, dbs)]
    print("$", " ".join(cmd), flush=True)
    r = subprocess.run(cmd, cwd=TAURI, capture_output=True)
    if r.returncode != 0:
        sys.exit("bench falló:\n" + r.stderr.decode("utf-8", "replace")[-2000:])
    text = r.stdout.decode("utf-8", "replace")
    res, cur = {}, None
    for line in text.splitlines():
        if line.startswith("### "):
            cur = line[4:].strip()
            res[cur] = {"_hdr": []}
        elif cur and line.startswith("- "):
            res[cur]["_hdr"].append(line[2:])
        elif cur and line.startswith("|") and not line.startswith("|---") and "p50" not in line:
            c = [x.strip() for x in line.strip("|").split("|")]
            res[cur][c[0]] = (float(c[1]), float(c[2]), float(c[3]))
    return res


def cpu_info():
    try:
        o = subprocess.run(["wmic", "cpu", "get", "Name,NumberOfCores,NumberOfLogicalProcessors", "/value"],
                           capture_output=True, timeout=15).stdout.decode(
            "oem" if sys.platform.startswith("win") else "utf-8", "replace")
        d = dict(l.strip().split("=", 1) for l in o.splitlines() if "=" in l)
        if d.get("Name"):
            return f"{d['Name'].strip()} ({d.get('NumberOfCores', '?')} núcleos / {d.get('NumberOfLogicalProcessors', '?')} hilos)"
    except Exception:
        pass
    try:
        o = subprocess.run(["powershell", "-NoProfile", "-Command",
                            "(Get-CimInstance Win32_Processor | Select-Object -First 1 | "
                            "ForEach-Object { $_.Name + ' (' + $_.NumberOfCores + ' núcleos / ' + "
                            "$_.NumberOfLogicalProcessors + ' hilos)' })"],
                           capture_output=True, text=True, timeout=30).stdout.strip()
        if o:
            return o
    except Exception:
        pass
    try:
        for l in open("/proc/cpuinfo"):
            if l.startswith("model name"):
                return l.split(":", 1)[1].strip()
    except Exception:
        pass
    return platform.processor() or platform.machine()


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser()
    ap.add_argument("--reps", type=int, default=10, help="repeticiones del legacy por escenario")
    ap.add_argument("--dbs", nargs="+", default=[str(ROOT / "dev-data/synth/synth_5000.db"),
                                                 str(ROOT / "dev-data/synth/synth_50000.db")])
    ap.add_argument("--no-gen", action="store_true")
    ap.add_argument("--md", help="escribe también el informe en este archivo")
    a = ap.parse_args()
    dbs = [Path(p).resolve() for p in a.dbs]
    for p in dbs:
        if p.name.lower() == "juegos.db":
            sys.exit("Se rechaza juegos.db.")
        if not p.exists():
            if a.no_gen:
                sys.exit(f"Falta {p}")
            m = re.search(r"synth_(\d+)\.db$", p.name)
            if not m:
                sys.exit(f"Falta {p}")
            p.parent.mkdir(parents=True, exist_ok=True)
            subprocess.run(["cargo", "run", "--release", "--bin", "gen_synth", "--", str(p), m.group(1), "42"],
                           cwd=TAURI, check=True)

    app = run_app_bench(dbs)
    lines = []
    emit = lambda s="": (lines.append(s), print(s, flush=True))
    emit(f"Máquina: {cpu_info()}; {platform.platform()}; Python {platform.python_version()}, "
         f"SQLite (módulo sqlite3) {sqlite3.sqlite_version}.")
    emit(f"Legacy: {a.reps} repeticiones (+1 de calentamiento) por escenario; app: 50 (+5), según `bench`.\n")
    for p in dbs:
        label = p.name
        info = app.get(label, {})
        db = LegacyDB(p)
        n = db.con.execute("SELECT count(*) FROM juegos").fetchone()[0]
        emit(f"#### {label} ({n} registros)\n")
        emit("| escenario | resultados legacy | legacy N+1 p50 | legacy N+1 p95 | legacy precargado p50 | legacy precargado p95 | app p50 | app p95 | p50 legacy N+1 / app |")
        emit("|---|---|---|---|---|---|---|---|---|")
        for name, texto, est, idi in SCENARIOS:
            (p50, p95, _), cnt = measure(lambda: legacy_refresh(db, texto, est, idi), a.reps)
            (q50, q95, _), cnt2 = measure(lambda: legacy_preloaded(db, texto, est, idi), a.reps)
            ap50, ap95, _ = info.get(name, (float("nan"),) * 3)
            ratio = f"{p50 / ap50:,.0f}×" if ap50 == ap50 and ap50 > 0 else "n/d"
            emit(f"| {name} | {cnt} | {p50:,.1f} | {p95:,.1f} | {q50:,.1f} | {q95:,.1f} | {ap50:.2f} | {ap95:.2f} | {ratio} |")
        emit("")
    if a.md:
        Path(a.md).write_text("\n".join(lines), encoding="utf-8")


if __name__ == "__main__":
    main()
