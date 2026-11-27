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
