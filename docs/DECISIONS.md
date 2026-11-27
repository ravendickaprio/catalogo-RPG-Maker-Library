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
**Revisión (Fase 3).** Se mantiene el CSS propio con variables y 4 temas (Sistema, Claro, Oscuro y Noche). No se adopta Tailwind/shadcn: la UI cubre las necesidades actuales sin dependencias extra.

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

## ADR-012 — Escritura sobre la DB real (Fase 4); sustituye las guardas del ADR-003
**Contexto.** El 2026-11-27 el usuario autorizó trabajar directamente sobre `juegos.db`. Mantiene varios backups fuera del repositorio.
**Decisión.**
- Se elimina el rechazo a `juegos.db`. La ruta sale de `RPGLIB_DB_PATH`, luego de los ajustes de la app (`settings.json` en el directorio de configuración de Tauri) y, en último caso, de la copia de desarrollo.
- La app abre en lectura y escritura salvo que se active el modo solo lectura en los ajustes.
- **Salvaguardas que se mantienen:**
  - backup automático antes de la primera escritura de cada sesión, con rotación de 20 en `<dbDir>/backups/`;
  - control de concurrencia optimista mediante `revision`;
  - transacciones `BEGIN IMMEDIATE`;
  - validación de esquema al abrir, sin crear ni alterar nada;
  - no forzar valores desconocidos de estado, idioma o tipo;
  - normalización NFC.
- Los tests y benchmarks se siguen ejecutando sobre copias o DB sintéticas.
**Consecuencias.** La app Python sigue siendo compatible: solo se escriben las columnas existentes y los triggers mantienen la búsqueda. `backups/` se añade a `.gitignore`.

## ADR-013 — Cierre de la Fase 4: migración v1 y ajuste del orden por código
- **v1 implementada:** `ix_juegos_titulo_nocase` + `ANALYZE`. Se registra en `app_metadata('desktop_schema_version')` y solo se aplica cuando el usuario lo pide desde Ajustes, con backup previo (ADR-009).
- **Orden por código:** la consulta de página usa `FROM juegos j NOT INDEXED LEFT JOIN juegos_busqueda bq …`, para que el planificador no recorra el índice de título.
- **Medición** en una copia sintética de 50k registros (p50, sin índice → con índice):
  - orden por código: 73,5 → 49,3 ms (antes del ajuste empeoraba: 72,6 → 207,6 ms);
  - orden por título: 60,8 → 2,9 ms;
  - página profunda: 79,8 → 3,1 ms.
- Ningún escenario empeora más allá del ruido. Con esto se cierra la decisión diferida en el ADR-011.

## ADR-014 — Portadas (Fase 5)
**Hechos.** El legacy (`comprimir_imagen`, v04 L318-336) reduce a un máximo de 300×300, aplana el alfa sobre blanco y guarda WebP calidad 65, con JPEG como alternativa. Las 249 portadas actuales son WebP de 3–28 KB.
**Decisión.**
- **Mismo formato que el legacy:** WebP con pérdida, q = 65, máximo 300×300 conservando la proporción y alfa aplanado sobre blanco. Crates: `image` (decodificación de JPEG, PNG, WebP, GIF y BMP en Rust puro, con límites de dimensiones y memoria) y `webp` (bindings de libwebp para codificar con pérdida). Así ambas apps producen portadas equivalentes y la DB no crece.
- **Entrega al frontend:** protocolo propio `cover` (`http://cover.localhost/<id>?r=<revision>` en Windows y `cover://localhost/<id>?r=…` en Linux) con `Cache-Control: max-age=31536000, immutable`. La revisión invalida la caché. `get_cover` en base64 se mantiene por compatibilidad. La CSP añade ese origen a `img-src`.
- **Entrada:** bytes en bruto por IPC (`InvokeBody::Raw`, sin base64), con un límite de 20 MB y validación por magic bytes. Se aceptan desde selector de archivo, arrastrar y soltar (`dragDropEnabled: false` en la ventana para recibir el evento HTML5) o pegar desde el portapapeles. Sin plugins.
- El procesamiento se hace en `spawn_blocking`. La escritura reutiliza revisión, backup de sesión y `BEGIN IMMEDIATE` (ADR-012).

## ADR-015 — Importador integrado (Fase 6)
**Contexto.** El importador Python (`Importar_Juegos_RPG_Maker_v2.py`) tiene bugs verificados en la auditoría:
- desfase de offsets al quitar corchetes antes del código, que produjo los 906 títulos `RY …`;
- `F`/`P` sueltas en cualquier posición se toman como estado;
- " en " de un título en español se toma como inglés;
- un `_` corta el código (`RY-d_642996` → `RY-D`);
- la deduplicación no normaliza el código.

**Decisión.**
- **Parser nuevo en Rust (`importer::parse_name`), con estas reglas:**
  - El código se elimina del texto **antes** de quitar los corchetes; los offsets se calculan siempre sobre la cadena vigente.
  - Estado: `[F]`/`[P]` entre corchetes en cualquier posición, o `F`/`P` solo si son el **primer** token.
  - Idioma: `EN`/`ES`/`ESP`/`JP`/`JPN` como token delimitado por `_ - . [ ] ( )` o por el final del nombre (sin distinguir mayúsculas), o separado por espacios solo si va **en mayúsculas**. Por defecto, Japonés (como el legacy).
  - Código: `RY[- _]+([A-Z0-9][A-Z0-9_-]*)`, en mayúsculas y sin `-` ni `_` finales.
  - Título vacío → el código, y si tampoco hay código, el nombre original (como el legacy).
- **Validación con datos reales:** sobre la copia, el parser nuevo se aplica al nombre de archivo de cada `ruta` y se compara con el estado y el idioma guardados. Debe igualar o superar al parser legacy (1158/1159 en estado). Los casos de la auditoría se convierten en golden tests, y las diferencias intencionales se documentan.
- **Detección de candidatos:** archivos `.zip`, `.rar` y `.7z` y carpetas cuyo nombre contenga un código. Se omiten `.git`, `__pycache__`, `.venv` y `node_modules`, y no se siguen enlaces simbólicos. El recorrido se puede cancelar y emite progreso por `tauri::ipc::Channel`.
- **Duplicados:** se detectan por código normalizado (la misma expresión que `codigo_norm`) y por ruta (`casefold` y separadores unificados), tanto contra la DB como dentro del propio lote.
- **Vista previa editable:** se puede marcar o desmarcar cada elemento y editar su título, estado e idioma. `pagina` y `tipo` por defecto son configurables; se proponen los valores más frecuentes en la DB.
- **Importación:** un único `BEGIN IMMEDIATE` con backup de sesión previo. Devuelve el resumen y los ids insertados.
- **Historial:** se añade al final de `app_data_dir/import-history.jsonl` (fecha, raíz, conteos e ids). "Deshacer última importación" borra esos ids solo si su revisión no cambió desde que se importaron; los demás se omiten y se informa. No requiere cambios de esquema.
- **Selector de carpeta:** se adopta `tauri-plugin-dialog` con **solo** el permiso `dialog:allow-open` (auditado y mínimo). Es la única forma razonable de obtener una ruta de carpeta en un WebView. También se acepta escribir la ruta como texto.
- El importador **nunca** mueve, renombra, extrae ni modifica archivos.

## ADR-016 — Centro de mantenimiento (Fase 7)
**Decisión.**
- **Detectores (solo lectura, paginados, en el backend):**
  - sin descripción;
  - sin portada;
  - sin tags;
  - códigos duplicados por `codigo_norm` (p. ej. ids 124/1016);
  - código vacío;
  - títulos con el patrón del bug del importador (`^RY\b`);
  - rutas inexistentes (comprobación de existencia en lotes y cancelable; en Linux, una ruta `X:\` se informa como "ruta de Windows");
  - estado inconsistente con la marca `[F]`/`[P]` del nombre de archivo;
  - tags huérfanas;
  - `juegos_busqueda` o `juegos_fts` desincronizados.
- **Acciones por lote** con vista previa:
  - fijar estado, idioma, tipo o página;
  - añadir o quitar una tag;
  - para títulos `RY …`, proponer el título que da el parser nuevo a partir del nombre de archivo de la ruta, solo si mejora (no vacío y sin `RY`);
  - eliminar tags huérfanas;
  - reconstruir FTS (`INSERT INTO juegos_fts(juegos_fts) VALUES('rebuild')` tras resincronizar `juegos_busqueda` desde `juegos`).
- **Seguridad de los lotes:** backup de sesión y `BEGIN IMMEDIATE`. Cada lote guarda un registro de deshacer (valores anteriores e ids, con la revisión posterior) en `app_data_dir/undo/<timestamp>.json`. "Deshacer último lote" restaura solo los registros cuya revisión no cambió desde entonces.
- **Cola de descripciones pendientes:** equivale a `catalogo_rpgmaker-descripciones.py`. Edita uno tras otro con "Guardar y siguiente", usando el `update_game` normal.
