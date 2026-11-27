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


## B-003 — Comparativa legacy vs app (Fase 9)
- **Comando exacto** (desde la raíz del repo; las DB sintéticas se generan con `gen_synth` si faltan):
  `python -I tools/bench_compare.py --reps 10` — ejecuta internamente `cd desktop/src-tauri && cargo run --release --bin bench -- dev-data/synth/synth_5000.db dev-data/synth/synth_50000.db`.
- **Máquina:** AMD Ryzen 7 5700X 8-Core Processor (8 núcleos / 16 hilos); Windows-11-10.0.26300-SP0; Python 3.14.2, SQLite (módulo sqlite3) 3.50.4.
- Legacy: 10 repeticiones (+1 de calentamiento) por escenario; app: 50 (+5), según `bench`.
- **Qué se mide.**
  - *App:* `repo::search` del backend Rust (SQL, FTS5/LIKE), una página de 100 filas más el recuento total, conexión de solo lectura. Sin IPC ni render.
  - *Legacy N+1 (réplica fiel):* lo que hace `refrescar_lista()` de `catalogo_rpgmaker-v04.py` (L981-L1047) en la vista lista, sin dibujar: `listar()` + por cada fila `tags_juego()` y `SELECT pagina,tipo,puntuacion` (1 + 2N sentencias), filtro y orden en Python con `casefold`, más `estadisticas()`, `todos_tags()` y otra consulta por cada fila resultante. Excluye el coste de Tk (insertar filas en el Treeview), que se suma al real.
  - *Legacy precargado:* el mismo filtro Python, pero con 3 consultas masivas por repetición (equivale a `tools/legacy_parity.py`). Aísla el coste del patrón N+1 del de filtrar en Python.
  - Mismas DB, mismas consultas y mismos filtros (orden «Nombre A-Z»/`title_asc`). Todo en solo lectura sobre las DB sintéticas (seed 42); no se usó la base real.
- **Resultados (ms; p50/p95 sobre 10 repeticiones el legacy y 50 la app):**

#### synth_5000.db (5000 registros)

| escenario | resultados legacy | legacy N+1 p50 | legacy N+1 p95 | legacy precargado p50 | legacy precargado p95 | app p50 | app p95 | p50 legacy N+1 / app |
|---|---|---|---|---|---|---|---|---|
| search empty, sort title, limit 100 | 5000 | 467.2 | 534.7 | 50.4 | 55.6 | 6.11 | 7.64 | 77× |
| search "the" (fts) | 851 | 371.4 | 431.6 | 63.0 | 68.3 | 4.05 | 4.30 | 92× |
| search "ry-1" (fts code) | 448 | 330.0 | 404.1 | 52.5 | 68.9 | 0.90 | 1.08 | 367× |
| search "ryu" (fts, 3 chars) | 5000 | 443.2 | 525.6 | 56.7 | 69.0 | 8.21 | 12.15 | 54× |
| search "the dark" (fts, long) | 6 | 340.5 | 380.5 | 55.7 | 70.4 | 0.49 | 0.83 | 693× |
| search "a" (like, 1 char) | 5000 | 469.8 | 527.3 | 55.9 | 63.2 | 11.23 | 14.90 | 42× |
| search "ry" (like, 2 chars) | 5000 | 420.4 | 447.7 | 49.0 | 58.6 | 10.13 | 11.58 | 41× |
| search "zzzzqqq" (no match) | 0 | 331.1 | 356.7 | 61.1 | 69.9 | 0.13 | 0.15 | 2,471× |
| estado='Sin empezar' + idioma='Japonés' | 3024 | 430.5 | 455.5 | 59.7 | 82.7 | 7.01 | 9.18 | 61× |
| "the" + estado + idioma | 532 | 378.2 | 426.2 | 66.7 | 75.5 | 4.18 | 4.89 | 90× |

#### synth_50000.db (50000 registros)

| escenario | resultados legacy | legacy N+1 p50 | legacy N+1 p95 | legacy precargado p50 | legacy precargado p95 | app p50 | app p95 | p50 legacy N+1 / app |
|---|---|---|---|---|---|---|---|---|
| search empty, sort title, limit 100 | 50000 | 4,644.2 | 5,040.9 | 477.8 | 520.5 | 74.26 | 90.97 | 63× |
| search "the" (fts) | 8345 | 3,197.8 | 3,484.0 | 572.9 | 640.3 | 52.70 | 64.34 | 61× |
| search "ry-1" (fts code) | 4428 | 2,987.9 | 3,650.7 | 592.9 | 604.2 | 37.10 | 48.10 | 81× |
| search "ryu" (fts, 3 chars) | 50000 | 4,521.2 | 5,124.3 | 459.9 | 493.0 | 87.76 | 127.73 | 52× |
| search "the dark" (fts, long) | 99 | 2,365.8 | 2,465.8 | 427.7 | 724.8 | 3.27 | 5.60 | 723× |
| search "a" (like, 1 char) | 50000 | 3,437.5 | 3,539.1 | 415.2 | 481.6 | 136.23 | 163.24 | 25× |
| search "ry" (like, 2 chars) | 50000 | 3,488.6 | 3,557.4 | 363.5 | 381.2 | 134.56 | 174.00 | 26× |
| search "zzzzqqq" (no match) | 0 | 2,287.3 | 2,383.0 | 420.7 | 425.6 | 0.17 | 0.21 | 13,298× |
| estado='Sin empezar' + idioma='Japonés' | 30053 | 2,797.8 | 2,818.9 | 346.4 | 349.7 | 84.89 | 103.24 | 33× |
| "the" + estado + idioma | 5080 | 2,391.8 | 2,414.5 | 421.4 | 426.0 | 56.42 | 68.64 | 42× |

- **Lectura de los resultados.**
  - El legacy fiel necesita ~0,3–0,5 s por refresco con 5 000 juegos y 2,3–4,6 s con 50 000, **con cualquier texto de búsqueda**, porque siempre recorre y consulta todas las filas. La app tarda 0,1–11 ms (5 000) y 0,2–136 ms (50 000).
  - Quitar solo el patrón N+1 (legacy precargado) ya baja el legacy a ~0,35–0,6 s con 50 000 juegos; el resto del salto viene de filtrar en SQL con índices y paginar.
  - Las búsquedas de 1–2 caracteres en la app (ruta LIKE) son las más lentas: 135–136 ms p50 con 50 000 registros, por encima del objetivo de 100 ms (en B-002 se midieron ~100 ms; ver «ruido» más abajo). Con 5 000 registros, ~10–11 ms.
- **Caveats.**
  - **p95 con 10 repeticiones** (legacy) es casi el máximo; úselo como orientación. La app usa 50 repeticiones.
  - **Ruido de la máquina:** no se controló la carga de la máquina (era un equipo de desarrollo en uso, con otras sesiones de trabajo posibles); B-002 ya advertía de variaciones de hasta ~40 %. Los números de la app en 50 000 filas salieron entre un 20 % y un 35 % por encima de B-002 (p. ej. búsqueda vacía por título 74,3 ms frente a 57,3 ms), lo que puede deberse a ese ruido o a cambios posteriores del backend (no se investigó). Repita en una máquina en reposo antes de citar cifras absolutas.
  - **No equivalente 1:1:** el legacy materializa y ordena **todos** los resultados (5 000 / 50 000 filas) y la app devuelve 100 (más el recuento). Es la diferencia real de diseño (paginación), pero no es la misma cantidad de trabajo de salida. El orden por título del legacy usa `casefold` Unicode y el de la app `NOCASE` ASCII.
  - **No incluye** el render (Tk Treeview en el legacy; React virtualizado en la app), la latencia por pulsación ni el IPC. El legacy real, con ~50 000 filas, añadiría además la inserción de miles de filas en el Treeview.
  - Los datos son sintéticos (distribución calcada del perfil real): las proporciones de coincidencias reales podrían variar. Con la biblioteca real (1 160 registros) las diferencias son menores en valor absoluto (el legacy ronda 60 ms; B-001).
  - Windows únicamente; Linux no medido.
