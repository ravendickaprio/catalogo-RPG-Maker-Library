# Benchmarks

Regla: solo se registran valores **medidos**. Los targets de `PROJECT_AUDIT.md` son metas, no resultados.

## B-001 — Backend Rust, Fase 1 (2026-11-26)
- **Comando:** `cd desktop/src-tauri && cargo run --release --bin bench` (por defecto usa `dev-data/juegos.dev.db`; acepta una ruta como argumento).
- **Entorno:** Windows 11 Pro x86_64, Rust 1.100.0 (MSVC), SQLite 3.50.2 (bundled por rusqlite), perfil release.
- **DB:** copia `juegos.dev.db`, 1160 registros, 5 017 600 bytes, FTS disponible y sincronizado. Conexión de solo lectura.
- **Metodología:** 5 iteraciones de calentamiento + 50 medidas por escenario. Mide solo la ruta SQL del backend: **no incluye IPC ni render**.

| Escenario | p50 ms | p95 ms | max ms |
|---|---|---|---|
| library_info | 0.577 | 0.597 | 0.626 |
| búsqueda vacía, orden por título, limit 100 | 2.181 | 2.262 | 2.554 |
| búsqueda "the" (fts) | 1.827 | 1.967 | 2.129 |
| búsqueda "ry-1" (fts, código) | 0.309 | 0.342 | 0.342 |
| búsqueda "a" (like, < 3 caracteres) | 3.129 | 3.979 | 4.095 |
| búsqueda "zzzzqqq" (sin coincidencias) | 0.199 | 0.239 | 0.351 |
| estado='Sin empezar' + idioma='Japonés' | 0.430 | 0.459 | 0.514 |
| get_game (ids aleatorios) | 0.056 | 0.083 | 0.093 |
| get_cover (ids con portada) | 0.033 | 0.049 | 0.057 |
| suggest | 0.748 | 0.788 | 0.799 |

Observado en la app real (Fase 1): la barra de estado mostró 2,8–3,1 ms de tiempo SQL en la carga inicial (búsqueda vacía, 100 filas).

## Referencia legacy (no comparable 1:1)
El patrón SQL de `_filtrar()` replicado en Python (no la app Tk completa), sobre la misma copia y en esta máquina, da **59,5 ms** de mediana con búsqueda vacía (3481 sentencias por refresco). Fuente: auditoría Haiku, `docs/PHASE0_AUDIT_VERIFIED.md`. No incluye el render de Tk, así que no es una comparación equivalente. Una comparación justa requiere medir también la latencia por pulsación en la UI (Fase 2/9).

## B-002 — Fase 2: escala sintética y optimización en dos fases (2026-11-27)
- **DB sintéticas:** `cargo run --release --bin gen_synth dev-data/synth/synth_N.db N 42` (seed 42, distribución calcada del perfil real, sin datos personales). Tamaños: 1k = 4,7 MiB · 10k = 45,6 MiB · 50k = 227 MiB. Todas pasan `integrity_check` y el `integrity-check` de FTS.
- **Bench:** `cargo run --release --bin bench dev-data/juegos.dev.db dev-data/synth/synth_50000.db …` (`--plans` para ver los planes de consulta, `--index-experiment` para probar índices sobre copias temporales).
- **Entorno:** el mismo que en B-001. La máquina tenía ruido de hasta un ~40 % entre ejecuciones, así que las diferencias pequeñas son solo orientativas.
- Salida completa: `docs/benchmarks-phase2-raw.txt`.

p50 en ms, antes y después de las optimizaciones del ADR-011 (sin índices nuevos):

| escenario | 50k antes | 50k después | 10k después | real 1160 después |
|---|---|---|---|---|
| library_info (sin caché) | 137,4 | 67,1 | 12,1 | 0,53 |
| vacía, orden título | 55,0 | 57,3 | 11,7 | 1,49 |
| vacía, orden código | 143,0 | 68,5 | 20,5 | 0,58 |
| página profunda | 107,5 | 70,8 | 28,0 | 0,82 |
| "the" (fts) | 37,5 | 37,1 | 12,3 | 0,62 |
| "ryu" (fts) | 76,8 | 77,3 | 16,3 | 2,06 |
| "ry" (like) | 98,2 | 100,7 | 19,9 | ~2,0 |
| estado + idioma | 90,7 | 73,4 | 12,7 | 0,46 |
| "the" + estado + idioma | 62,2 | 39,7 | 8,1 | — |
| filtro de tag | 0,33 | 0,27 | 0,17 | — |
| sugerencia | 162,6 | 66,5 | 13,1 | 0,43 |

**Contra los targets** (búsqueda simple ≤ 100 ms, combinada ≤ 150 ms, filtros ≤ 100 ms):
- Se cumplen con 50k registros en el SQL del backend. La única ruta en el límite es el LIKE de 1–2 caracteres (~100 ms).
- Con la biblioteca real, todo está por debajo de 2,1 ms.
- Aún no se ha medido la latencia de extremo a extremo (pulsación → render).

**Experimento de índice** (`titulo COLLATE NOCASE`, sobre copia temporal): con 50k registros, el orden por título baja de 60,2 a 2,9 ms y la página profunda de 82,0 a 3,0 ms. Pero el orden por código sube de 74,0 a 189,3 ms. Coste en disco: +2,5 MiB. Decisión diferida en ADR-011.

**Paridad con el legacy** (`python -I tools/legacy_parity.py <db>`, 38 casos):
- Coinciden 37/38, tanto en conjunto de resultados como en orden, en la copia real y en la sintética de 5k.
- La única diferencia es el caso 19, coincidencias entre campos concatenados, que se descartó a propósito.

## Pendiente
- Latencia de extremo a extremo (pulsación → render), memoria y scroll (Fase 3/5).
- Linux (Nobara/Fedora): no medido.
