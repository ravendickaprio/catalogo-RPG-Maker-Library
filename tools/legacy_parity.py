#!/usr/bin/env python
"""Paridad de búsqueda: Rust (`parity` bin) vs. `_filtrar()` del legacy v04.

Uso (stdlib only):  python -I tools/legacy_parity.py <db> [--cases tools/parity_cases.json]
                    [--rust-bin desktop/src-tauri/target/release/parity.exe]

* Abre la DB en SOLO LECTURA (`file:...?mode=ro`, uri=True). Rechaza `juegos.db`.
* Reimplementa `_filtrar()` de catalogo_rpgmaker-v04.py (L981-1022) sobre datos
  precargados (misma semántica, sin el patrón N+1).
* Ejecuta el binario Rust con los mismos casos y compara conjunto y orden.
"""
import argparse
import json
import re
import sqlite3
import subprocess
import sys
from pathlib import Path

# catalogo_rpgmaker-v04.py L47-48
ESTADOS = ["Sin empezar", "Empezado", "Terminado"]
IDIOMAS = ["Español", "Inglés", "Japonés"]

# Etiquetas de ORDENES (v04 L53) <- claves de SortKey del contrato
SORT_LABEL = {
    "title_asc": "Nombre (A-Z)",
    "title_desc": "Nombre (Z-A)",
    "code": "Código",
    "status": "Estado",
    "language": "Idioma",
    "recent": "Más recientes",
}


def indice(lista, valor):
    # v04 L50-51
    return lista.index(valor) if valor in lista else len(lista)


def normalizar_codigo(texto):
    """v04 L355-357: 'RY-4762560' y 'ry 4762560' y 'RY4762560' se comparan igual."""
    return re.sub(r"[\s\-_]", "", (texto or "").casefold())


class Legacy:
    def __init__(self, db_path):
        uri = Path(db_path).resolve().as_uri() + "?mode=ro"
        self.con = sqlite3.connect(uri, uri=True)
        # listar() v04 L236-240 (mismo orden de filas: sin ORDER BY => rowid)
        self.filas = self.con.execute(
            "SELECT id, titulo, descripcion, estado, idioma, ruta, codigo FROM juegos"
        ).fetchall()
        self.extra = {
            r[0]: r[1:]
            for r in self.con.execute("SELECT id, pagina, tipo, puntuacion FROM juegos")
        }
        self.tags = {}
        # tags_juego() v04 L249-252: ORDER BY nombre COLLATE NOCASE
        for jid, nombre in self.con.execute(
            "SELECT jt.juego_id, t.nombre FROM juego_tags jt JOIN tags t ON t.id=jt.tag_id "
            "ORDER BY t.nombre COLLATE NOCASE"
        ):
            self.tags.setdefault(jid, []).append(nombre)

    def filtrar(self, case):
        """Copia fiel de `_filtrar()` v04 L981-1022."""
        texto = (case["query"] or "").strip().casefold()          # L982
        texto_norm = normalizar_codigo(texto)                      # L983
        f_estado = case.get("estado") or "Todos"                   # L984
        f_idioma = case.get("idioma") or "Todos"                   # L985
        f_tag = case.get("tag") or "Todos"                         # L986

        juegos = []
        for fila in self.filas:                                    # L989
            jid, titulo, desc, estado, idioma, _, codigo = fila    # L990
            tags = self.tags.get(jid, [])                          # L991
            pagina, tipo, puntuacion = self.extra[jid]             # L992-995
            campos = " ".join([pagina or "", tipo or "", " ".join(tags)]).casefold()  # L996
            en_codigo = (texto in (codigo or "").casefold()
                         or (texto_norm and texto_norm in normalizar_codigo(codigo)))  # L997-998
            if texto and not (en_codigo or texto in titulo.casefold()
                              or texto in (desc or "").casefold() or texto in campos):  # L999-1001
                continue
            if f_estado != "Todos" and estado != f_estado:         # L1002
                continue
            if f_idioma != "Todos" and idioma != f_idioma:         # L1004
                continue
            if f_tag != "Todos" and f_tag.casefold() not in [t.casefold() for t in tags]:  # L1006
                continue
            juegos.append(fila)

        orden = SORT_LABEL[case["sort"]]                           # L1010
        if orden == "Código":
            juegos.sort(key=lambda j: (not j[6], (j[6] or "").casefold(), j[1].casefold()))
        elif orden == "Nombre (Z-A)":
            juegos.sort(key=lambda j: j[1].casefold(), reverse=True)
        elif orden == "Estado":
            juegos.sort(key=lambda j: (indice(ESTADOS, j[3]), j[1].casefold()))
        elif orden == "Idioma":
            juegos.sort(key=lambda j: (indice(IDIOMAS, j[4]), j[1].casefold()))
        elif orden == "Más recientes":
            juegos.sort(key=lambda j: j[0], reverse=True)
        else:
            juegos.sort(key=lambda j: j[1].casefold())
        return juegos

    def sort_key(self, case, jid):
        j = next(f for f in self.filas if f[0] == jid)
        o = SORT_LABEL[case["sort"]]
        if o == "Código":
            return (not j[6], (j[6] or "").casefold(), j[1].casefold())
        if o in ("Nombre (A-Z)", "Nombre (Z-A)"):
            return j[1].casefold()
        if o == "Estado":
            return (indice(ESTADOS, j[3]), j[1].casefold())
        if o == "Idioma":
            return (indice(IDIOMAS, j[4]), j[1].casefold())
        return j[0]

    # ---- clasificación heurística de diferencias de CONJUNTO
    def why_legacy_only(self, case, jid):
        f = next(f for f in self.filas if f[0] == jid)
        texto = case["query"].strip().casefold()
        norm = normalizar_codigo(texto)
        pagina, tipo, _ = self.extra[jid]
        tags = self.tags.get(jid, [])
        singles = [f[1], f[6] or "", f[2] or "", pagina or "", tipo or ""] + tags
        if any(texto in s.casefold() for s in singles):
            if any(ord(ch) > 127 for ch in texto) and not any(
                    case["query"].strip() in s for s in singles):
                return ("pliegue Unicode: casefold de Python casa mayúscula/minúscula no ASCII; "
                        "LIKE de SQLite solo pliega ASCII")
            return "subcadena en un campo tras casefold (causa no clasificada)"
        if norm and norm in normalizar_codigo(f[6]) and len(norm) < 3 <= len(texto):
            return "codigo_norm<3 chars con consulta cruda>=3 (Rust no añade la cláusula de código)"
        if texto in " ".join([pagina or "", tipo or "", " ".join(tags)]).casefold():
            return "coincidencia cruzando campos pagina/tipo/tags (legacy concatena)"
        return "desconocida"

    def why_rust_only(self, case, jid):
        f = next(f for f in self.filas if f[0] == jid)
        texto = case["query"].strip().casefold()
        if not texto:
            return "desconocida"
        if any(ord(ch) > 127 for ch in texto):
            return "consulta no ASCII (pliegue Unicode de trigram/LIKE vs casefold)"
        return "desconocida"


def run_rust(rust_bin, db, cases_path):
    out = subprocess.run([str(rust_bin), str(db), str(cases_path)],
                         capture_output=True, text=True, encoding="utf-8")
    if out.returncode != 0:
        sys.exit(f"parity (Rust) falló: {out.stderr.strip()}")
    return json.loads(out.stdout)


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser()
    ap.add_argument("db")
    ap.add_argument("--cases", default=str(Path(__file__).with_name("parity_cases.json")))
    ap.add_argument("--rust-bin", default=str(
        Path(__file__).resolve().parents[1] / "desktop/src-tauri/target/release/parity.exe"))
    a = ap.parse_args()
    if Path(a.db).name.lower() == "juegos.db":
        sys.exit("Se rechaza abrir juegos.db; use una copia.")
    cases = json.loads(Path(a.cases).read_text(encoding="utf-8"))
    legacy = Legacy(a.db)
    rust = run_rust(a.rust_bin, a.db, a.cases)
    assert len(rust) == len(cases)

    print(f"# Paridad Rust vs legacy _filtrar() — {Path(a.db).name} "
          f"({len(legacy.filas)} filas, {len(cases)} casos)\n")
    print("| # | caso | legacy | rust | strat | conjunto | orden |")
    print("|---|---|---|---|---|---|---|")
    diffs = []
    n_set = n_order = 0
    for i, (case, r) in enumerate(zip(cases, rust)):
        assert r["case"]["query"] == case["query"]
        lids = [f[0] for f in legacy.filtrar(case)]
        rids = r["ids"]
        same_set = set(lids) == set(rids)
        same_order = lids == rids
        n_set += same_set
        n_order += same_order
        desc = f'q={case["query"]!r} e={case.get("estado")} i={case.get("idioma")} t={case.get("tag")} s={case["sort"]}'
        print(f"| {i} | {desc} | {len(lids)} | {len(rids)} | {r['strategy']} | "
              f"{'OK' if same_set else 'DIFF'} | {'OK' if same_order else 'DIFF'} |")
        if not same_order:
            diffs.append((i, case, lids, rids, same_set))
    print(f"\nConjunto igual: {n_set}/{len(cases)}; orden idéntico: {n_order}/{len(cases)}\n")

    for i, case, lids, rids, same_set in diffs:
        print(f"## Caso {i}: {json.dumps(case, ensure_ascii=False)}")
        if not same_set:
            lo = sorted(set(lids) - set(rids))
            ro = sorted(set(rids) - set(lids))
            print(f"- solo legacy: {len(lo)} ids, muestra {lo[:8]}")
            for j in lo[:3]:
                print(f"    - id {j}: {legacy.why_legacy_only(case, j)}")
            print(f"- solo rust: {len(ro)} ids, muestra {ro[:8]}")
            for j in ro[:3]:
                print(f"    - id {j}: {legacy.why_rust_only(case, j)}")
        common = [x for x in lids if x in set(rids)]
        rcommon = [x for x in rids if x in set(lids)]
        if common != rcommon:
            k = next(n for n, (x, y) in enumerate(zip(common, rcommon)) if x != y)
            a_id, b_id = common[k], rcommon[k]
            ka, kb = legacy.sort_key(case, a_id), legacy.sort_key(case, b_id)
            if ka == kb:
                reason = "empate de clave en legacy (orden estable asc por id) vs desempate de Rust"
            else:
                texts = [f[1] + (f[6] or "") for f in legacy.filas if f[0] in (a_id, b_id)]
                reason = ("colación: casefold Unicode (Python) vs NOCASE solo ASCII (SQLite)"
                          if any(ord(ch) > 127 for t in texts for ch in t) else "desconocida")
            n_diff = sum(1 for x, y in zip(common, rcommon) if x != y)
            print(f"- orden (sobre ids comunes): primera diferencia en posición {k}: "
                  f"legacy id {a_id} vs rust id {b_id}; posiciones distintas: {n_diff}/{len(common)}")
            print(f"    - motivo: {reason}")
        print()


if __name__ == "__main__":
    main()
