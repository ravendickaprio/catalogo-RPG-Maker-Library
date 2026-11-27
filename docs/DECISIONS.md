# Registro de decisiones (ADR)

Formato: contexto → decisión → consecuencias. Fecha de referencia: 2026-11-26.

## ADR-001 — Base de la rama `feature/desktop-modernization-tauri`
**Contexto.** `main` está por detrás de `feature/description-cleanup-phase1`: los 8 commits más recientes (herramienta de descripciones, editor completo, `.gitignore` de DB) solo existen en esa rama.
**Decisión.** La rama de modernización se crea desde el HEAD de `feature/description-cleanup-phase1` (`0d82aed`), para tomar como referencia el legacy más reciente. `main` no se toca.
**Consecuencias.** Al integrar en el futuro, primero habrá que llevar `feature/description-cleanup-phase1` a `main` (decisión del usuario).

## ADR-002 — Acceso a SQLite: `rusqlite` (bundled) en vez de SQLx
**Contexto.** La app es local, monousuario y con una sola DB de ~5 MB (1160 registros). La DB ya usa FTS5 con tokenizer `trigram` y triggers.
**Decisión.** Usar `rusqlite` con la feature `bundled`, que compila SQLite reciente con FTS5 y trigram. Las consultas se ejecutan fuera del hilo de UI con `tauri::async_runtime::spawn_blocking`.
**Por qué no SQLx.** Sus macros en compilación requieren `DATABASE_URL` o modo offline. Su backend SQLite usa igualmente un hilo worker, así que no aporta paralelismo real, y añade más dependencias. rusqlite da control directo de flags de apertura, `query_only`, FTS5 y funciones.
**Consecuencias.** Para las migraciones de la Fase 2+ se evaluará `rusqlite_migration` o SQL versionado propio.

## ADR-003 — Prototipo en modo solo lectura
**Decisión.** En la Fase 1, la DB se abre con `SQLITE_OPEN_READ_ONLY` + `PRAGMA query_only = ON`. No se crean tablas, índices, triggers ni migraciones.
**Guardas.** La ruta se toma de `RPGLIB_DB_PATH`. Si no está definida, se usa `dev-data/juegos.dev.db` (copia ignorada por Git). Se rechaza abrir un archivo llamado `juegos.db` (la DB principal) salvo `RPGLIB_ALLOW_PRIMARY_DB=1`.
**Consecuencias.** No hay riesgo de escritura sobre la biblioteca. Coexistencia segura con la app Python. El CRUD llega en la Fase 4.

## ADR-004 — Búsqueda inicial
**Contexto.** La DB contiene `juegos_busqueda` (tabla espejo mantenida por triggers) y `juegos_fts` (FTS5 `trigram`, content table). Ningún script del repositorio crea estas estructuras (verificado con `git log -S`): proceden de una herramienta externa. La app legacy v04 no las usa y filtra en Python.
**Decisión.**
- Consulta con ≥ 3 caracteres (tras trim) y si `juegos_fts` existe: `juegos_fts MATCH ?` con la frase entrecomillada y escapada, que hace coincidencia por subcadena sin distinguir mayúsculas. Si la consulta normalizada como código (sin espacios, `-`, `_`, NBSP, en minúsculas) tiene ≥ 3 caracteres, se añade `OR codigo_norm : "<norm>"`.
- Consulta con < 3 caracteres, o si no hay FTS: `LIKE '%q%' ESCAPE '\'` sobre `juegos_busqueda` (o sobre `juegos` si esta tampoco existe).
- Los campos buscados son los mismos que en el legacy: título, código, código normalizado, descripción, página, tipo y tags.
**Diferencias conocidas.** El legacy usa `casefold()` de Python. `LIKE` solo pliega mayúsculas en ASCII y trigram pliega Unicode. Ninguno de los dos elimina acentos (igual que el legacy). Hay que validarlo con casos en la Fase 2.
**Riesgo.** Si `juegos_fts` se desincroniza, los resultados divergen. Se verifica `count(juegos) == count(juegos_busqueda)` en `get_library_info` y se informa en la UI.

## ADR-005 — Semántica de "Juego sugerido del día"
**Hechos (DB copia, 2026-11-26).** `estado` ∈ {`Sin empezar` 961, `Terminado` 153, `Empezado` 46}, sin variantes de mayúsculas ni espacios. El legacy define `ESTADOS` y el importador asigna `[F]`→Terminado, `[P]`→Empezado y el resto →Sin empezar.
**Decisión (MVP).**
- Elegible: `lower(trim(coalesce(estado,''))) <> 'terminado'`. Un estado nuevo o vacío cuenta como "no terminado" y se incluye.
- Ámbito: toda la biblioteca, ignorando búsqueda y filtros activos (es más predecible; respetar filtros queda como opción en la Fase 3).
- Se recalcula en cada pulsación. El parámetro `excludeId` evita repetir la sugerencia inmediatamente anterior si hay más de un candidato.
- No se persiste nada: ni fecha, ni historial, ni cambios de estado. La conexión es de solo lectura, así que una escritura accidental fallaría.
- Sin candidatos: la respuesta trae `game = null` con el conteo y la UI muestra un mensaje.

## ADR-006 — Frontend sin framework CSS en la Fase 1
**Decisión.** Vite + React + TypeScript con CSS propio y variables (tema claro/oscuro según `prefers-color-scheme`). Listas con TanStack Virtual. La elección de Tailwind/shadcn se decide en la Fase 3.
**Motivo.** Mantener el prototipo pequeño y auditable.

## ADR-007 — Portadas en la Fase 1
**Hechos.** 249/1160 registros tienen `imagen`, todos WebP (magic `RIFF`), con un máximo de 27,8 KB.
**Decisión.** Comando `get_cover(id)` → `{ mime, dataBase64 }`, cargado solo en el detalle o la sugerencia. Los listados nunca leen el BLOB: solo `hasCover`, vía `length(imagen) > 0`. El protocolo personalizado y la caché de miniaturas quedan para la Fase 5.

## ADR-008 — Superficie Tauri mínima
**Decisión.** Sin plugins (ni opener, ni fs, ni shell, ni dialog). Capability `core:default` únicamente. CSP restrictiva (`default-src 'self'; img-src 'self' data:`). Comandos expuestos: `get_library_info`, `search_games`, `get_game`, `get_cover`, `suggest_game`.

## ADR-009 — Estrategia de migraciones (Fase 2: decisión; implementación con el primer cambio de esquema)
**Contexto.** El legacy crea y altera el esquema de forma perezosa (`CREATE IF NOT EXISTS` + `ALTER`) y no usa `user_version` (= 0). Una herramienta externa añadió `app_metadata(busqueda_schema=1)`, `juegos_busqueda`, `juegos_fts` y 9 triggers. Tanto la app Python como los editores escriben sobre la misma DB.
**Decisión.**
- **Versionado propio en `app_metadata`**, clave `desktop_schema_version`. No se usa `user_version`, para no chocar con herramientas que pudieran usarlo, y no se modifica `busqueda_schema`.
- **Solo migraciones aditivas y compatibles con el legacy:** índices (`CREATE INDEX IF NOT EXISTS ix_…`) y tablas nuevas con prefijo `desktop_`. Nunca se renombran, eliminan ni cambian columnas o triggers existentes. Toda migración se prueba antes sobre DB sintéticas y sobre la copia.
- **Proceso:** se ejecuta por acción explícita del usuario, nunca automáticamente al abrir. Pasos:
  1. Backup con la API `backup` de SQLite en `<db>.backup-<AAAAMMDD-HHMMSS>-pre-v<N>`.
  2. Migración dentro de `BEGIN IMMEDIATE … COMMIT`.
  3. Comprobación de `PRAGMA integrity_check`, `foreign_key_check` y `INSERT INTO juegos_fts(juegos_fts) VALUES('integrity-check')`.
  4. Si algo falla, rollback e informe; el backup se conserva.
- **Reparaciones de datos** (p. ej. los 906 títulos `RY …`): son "migraciones de datos" con vista previa, backup y un registro de los cambios para poder revertirlos. Requieren aprobación del usuario caso por caso.
**Consecuencias.** La app Python sigue funcionando sin cambios. No se implementa código de migración hasta que haga falta el primer cambio (índices, si el ADR-011 los justifica, o el CRUD de la Fase 4).

## ADR-010 — Concurrencia con la app Python
**Hechos.** `journal_mode = delete` (rollback journal). Python `sqlite3` usa `timeout = 5 s` por defecto. Los editores hacen commit tras cada guardado.
**Decisión.**
- **Mantener el rollback journal (no WAL)** mientras coexistan las dos apps. WAL es persistente, cambia el formato en disco (`-wal`, `-shm`) y afecta a todas las herramientas. Se reevaluará cuando la app Python se retire.
- Lecturas cortas (consultas paginadas, sin transacciones largas abiertas) y `busy_timeout`: 2 s para lectura y 5 s para escritura cuando exista el CRUD.
- Las escrituras de la Fase 4 usarán transacciones `BEGIN IMMEDIATE`, breves y sin interacción de UI dentro de ellas. Si devuelven `SQLITE_BUSY`, se muestra un error recuperable ("la base está ocupada por otra aplicación").
- **Detección de cambios externos:** al recuperar el foco de la ventana (Fase 3) se consulta `PRAGMA data_version`. Si cambió, se refrescan la lista y el detalle sin perder la selección.
- Recomendación de uso: no editar el mismo juego en ambas apps a la vez. La última escritura gana, y la Fase 4 lo detectará comparando el registro antes de guardar.

## ADR-011 — Rendimiento de consultas sin cambiar el esquema; índice de título diferido (Fase 2)
**Hechos medidos** (sobre la DB sintética de 50k registros; detalle en `BENCHMARKS.md`):
- En `juegos`, la columna `imagen` (BLOB) precede a `ruta`, `codigo`, `pagina`, `tipo` y `puntuacion`. Leer esas columnas recorre las páginas de overflow del BLOB: `count(*) WHERE codigo<>'x'` tarda 119 ms, frente a 29 ms con `estado`.
- `juegos_busqueda` (sin BLOB) es barata: 19 ms.
**Decisión.**
- La búsqueda se hace en dos fases. Primero se obtienen los ids ordenados usando solo columnas anteriores al BLOB o `juegos_busqueda`. Después se leen como mucho 200 filas de la página.
- La sugerencia elige el id en Rust (splitmix64) en lugar de `ORDER BY random()`.
- `library_info` se resuelve con un único `GROUP BY estado, idioma` y se cachea durante la sesión.
- Paridad con el legacy: los empates de `title_desc` se ordenan por `id ASC`. Un código con normalización de 1–2 caracteres usa LIKE sobre `codigo_norm`. Las consultas no ASCII en la ruta LIKE se pliegan con `rpg_fold()`, que aplica el `to_lowercase` de Rust. **No** se replica la coincidencia entre campos concatenados del legacy.
- **Índice `titulo COLLATE NOCASE`:** lleva el orden por título y la paginación profunda de ~60 ms a ~3 ms con 50k registros, pero empeora el orden por código (74 → 189 ms) por el orden de join que elige el planificador. Se **difiere** a la primera migración (Fase 4, ADR-009), junto con un ajuste del join del orden por código y una nueva medición. Los índices de `codigo` e `idioma` se descartan; el de `estado` es opcional.
**Consecuencias.** Con 50k registros, todas las rutas medidas quedan en ≤ ~100 ms sin tocar la DB. Con la biblioteca real (1160), todo está por debajo de 2,1 ms.
