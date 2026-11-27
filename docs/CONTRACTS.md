# Contratos frontend ↔ backend (Fase 1 + cambios Fases 2 y 3)

Comandos Tauri invocados con `invoke(nombre, args)`. Los DTO se serializan en **camelCase** (`#[serde(rename_all = "camelCase")]`). Los errores siguen siempre la forma `AppError { kind, message }`. Todos los comandos son `async` y ejecutan SQLite con `spawn_blocking`. La conexión es de solo lectura (ADR-003).

## Tipos

```ts
type SortKey = "title_asc" | "title_desc" | "code" | "status" | "language" | "recent";

interface SearchRequest {
  query: string;          // ≤ 200 caracteres tras trim; "" = sin texto
  estado?: string | null; // valor literal de juegos.estado; null = todos
  idioma?: string | null; // valor literal de juegos.idioma; null = todos
  tag?: string | null;    // [Fase 2] nombre de tag, comparación NOCASE; null = todos
  sort: SortKey;
  offset: number;         // ≥ 0
  limit: number;          // 1..=200
}

interface GameSummary {
  id: number; titulo: string; codigo: string; estado: string; idioma: string;
  tipo: string; pagina: string; puntuacion: number; hasCover: boolean;
}

interface SearchPage {
  items: GameSummary[];
  total: number;          // total de coincidencias (no de la página)
  offset: number; limit: number;
  elapsedMs: number;      // tiempo SQL medido en backend
  strategy: "fts" | "like" | "none";
}

interface GameDetail extends GameSummary {
  descripcion: string; ruta: string; tags: string[];
}

interface Cover { mime: string; dataBase64: string } // mime por magic bytes: image/webp|png|jpeg|gif

interface CountItem { value: string; count: number }

interface LibraryInfo {
  dbFileName: string;     // solo el nombre del archivo, no la ruta completa
  readOnly: boolean;      // siempre true en la Fase 1
  total: number;
  ftsAvailable: boolean;  // existen juegos_fts y juegos_busqueda
  ftsInSync: boolean;     // count(juegos) == count(juegos_busqueda)
  estados: CountItem[];   // orden por count desc
  idiomas: CountItem[];
  tags: CountItem[];      // [Fase 2] tags con nº de juegos, orden nombre NOCASE
  sqliteVersion: string;
}

interface SuggestRequest { excludeId?: number | null }
interface Suggestion {
  game: GameDetail | null;   // null = sin candidatos
  eligibleCount: number;     // juegos no terminados
  totalCount: number;
}

interface DataVersion { version: number; changed: boolean } // [Fase 3]

interface AppError { kind: "NotFound" | "InvalidInput" | "Database" | "Config"; message: string }
```

## Comandos

| Comando | Args | Retorno |
|---|---|---|
| `get_library_info` | — | `LibraryInfo` |
| `search_games` | `{ req: SearchRequest }` | `SearchPage` |
| `get_game` | `{ id: number }` | `GameDetail` (`NotFound` si no existe) |
| `get_cover` | `{ id: number }` | `Cover \| null` |
| `suggest_game` | `{ req: SuggestRequest }` | `Suggestion` |
| `check_data_version` | — | `DataVersion` [Fase 3] |

## Reglas de backend
- Nunca seleccionar `imagen` excepto en `get_cover`. `hasCover = coalesce(length(imagen),0) > 0`.
- Sin N+1: las tags del detalle se obtienen en una sola consulta. Los listados no incluyen tags.
- Orden: `title_asc` = `titulo COLLATE NOCASE, id`; `title_desc` = `titulo COLLATE NOCASE DESC, id ASC` (igual que el sort estable invertido del legacy, Fase 2); `code` = sin código al final, luego `codigo COLLATE NOCASE, titulo`; `status` = `CASE estado WHEN 'Sin empezar' THEN 0 WHEN 'Empezado' THEN 1 WHEN 'Terminado' THEN 2 ELSE 3 END, titulo`; `language` = `idioma COLLATE NOCASE, titulo`; `recent` = `id DESC`.
- Filtro tag [Fase 2]: `j.id IN (SELECT jt.juego_id FROM juego_tags jt JOIN tags t ON t.id = jt.tag_id WHERE t.nombre = ? COLLATE NOCASE)`.
- Validación: tag ≤ 100 caracteres; query ≤ 200 caracteres, limit 1..=200, offset ≥ 0, id > 0. Si falla → `InvalidInput`.
- Elegibilidad de la sugerencia: `lower(trim(coalesce(estado,''))) <> 'terminado'`. Con `excludeId` y más de un candidato, se excluye ese id.

## `check_data_version` [Fase 3, ADR-010]
Lee `PRAGMA data_version` en la conexión de la app (solo lectura, sin escribir) y lo compara con el último valor visto (guardado en `Ready`, inicializado al abrir la base). `changed = true` cuando otra conexión (p. ej. la app Python) hizo commit desde la última comprobación; en ese caso se invalida la caché de `LibraryInfo`, de modo que el siguiente `get_library_info` devuelve datos nuevos. Cada llamada actualiza el valor visto: dos llamadas seguidas sin escrituras externas devuelven `changed = false`. El frontend la invoca al recuperar el foco y cada 30 s mientras la ventana es visible.

## Reglas de frontend
- Debounce de 150 ms en la búsqueda. Cada petición lleva un número de secuencia y se descartan respuestas obsoletas.
- Lista virtualizada (TanStack Virtual) con paginación incremental de 100 en 100 al hacer scroll.
- La portada se pide solo al abrir el detalle o la sugerencia.
- La sugerencia se muestra en un diálogo accesible (`role="dialog"`, `aria-modal`, Esc para cerrar, foco inicial). Botones "Otra sugerencia" y "Ver en la lista". Nada se escribe.

---

## Fase 4 — CRUD, ajustes, backups y migraciones

### Modo de apertura y configuración
- **Ajustes** en `<app_config_dir>/settings.json`, donde `app_config_dir` es la ruta de Tauri (`%APPDATA%\local.rpgmakerlibrary.desktop` en Windows y `~/.config/local.rpgmakerlibrary.desktop` en Linux). Contenido: `{ "dbPath": string | null, "readOnly": boolean }`.
- **Orden de resolución de la ruta:** `RPGLIB_DB_PATH` → `settings.dbPath` → copia de desarrollo. Ya no se rechaza `juegos.db`, porque el usuario lo autorizó el 2026-11-27. El ADR-003 queda reemplazado por el ADR-012.
- **Modo:** lectura y escritura salvo `settings.readOnly = true` o `RPGLIB_READ_ONLY=1`.
- **Conexión de escritura:** `foreign_keys = ON` y `busy_timeout = 5000`. Las escrituras usan `BEGIN IMMEDIATE`.
- **Validación de esquema al abrir:** `juegos` debe tener las 11 columnas del legacy, más `tags` y `juego_tags`. Si falta algo → `Config`, con la lista de faltantes. La app no crea ni altera nada al abrir.

### Tipos nuevos
```ts
interface GameInput {
  titulo: string;        // trim, 1..=300
  descripcion: string;   // ≤ 20000
  estado: string;        // trim, 1..=50 (cualquier valor: no se fuerza a la lista)
  idioma: string;        // trim, 1..=50
  ruta: string;          // ≤ 1000, sin normalizar separadores
  codigo: string;        // trim, ≤ 100 ('' permitido)
  pagina: string;        // trim, ≤ 100
  tipo: string;          // trim, ≤ 50; '' → 'Otro' (como el legacy)
  puntuacion: number;    // 0..=5, múltiplo de 0.5
  tags: string[];        // trim, se descartan vacíos, únicos (NOCASE), cada uno ≤ 100, máx. 50
}
interface WriteOptions {
  allowDuplicateCode?: boolean; // por defecto false
  expectedRevision?: string | null; // obligatorio en update/delete
}
interface GameDetail { ...; revision: string } // nuevo campo: huella del registro (ver abajo)
interface Vocabulary { estados: string[]; idiomas: string[]; paginas: string[]; tipos: string[]; puntuaciones: number[] }
interface SessionInfo { dbFileName: string; dbDir: string; readOnly: boolean; lastBackup: string | null }
```
- **`revision`:** hash FNV-1a de 64 bits (en hex) de los valores actuales de titulo, descripcion, estado, idioma, ruta, codigo, pagina, tipo, puntuacion, `length(imagen)` y tags ordenados. Detecta si otra app modificó el registro.
- **NFC:** todo el texto se normaliza a NFC antes de escribir (crate `unicode-normalization`).
- **Vocabulario:** las listas del legacy (v04 L47-52) unidas a los valores distintos presentes en la DB, para no perder valores desconocidos.

### Comandos nuevos
| Comando | Args | Retorno | Notas |
|---|---|---|---|
| `get_vocabulary` | — | `Vocabulary` | |
| `get_session_info` | — | `SessionInfo` | |
| `create_game` | `{ input, opts }` | `GameDetail` | Código duplicado (por `codigo_norm`) sin `allowDuplicateCode` → error `DuplicateCode` |
| `update_game` | `{ id, input, opts }` | `GameDetail` | Si la revisión no coincide → `Conflict`. No toca `imagen` |
| `delete_game` | `{ id, opts }` | `null` | Borra `juego_tags` y luego `juegos`. Exige `expectedRevision` |
| `create_backup` | — | `{ path: string }` | Backup manual |
| `set_db_path` | `{ path }` | `LibraryInfo` | Valida, guarda en ajustes y reabre. Ruta absoluta a un archivo existente |
| `set_read_only` | `{ readOnly }` | `SessionInfo` | Reabre la conexión |

Nuevos `AppError.kind`: `"Conflict" | "DuplicateCode" | "ReadOnly" | "Busy"`. `DuplicateCode` incluye en el mensaje el título del juego que ya tiene ese código.

### Backups (ADR-012)
- Antes de la **primera escritura de cada sesión** se hace un backup automático con la API de backup: `<dbDir>/backups/<nombre>.bak-AAAAMMDD-HHMMSS`. Se conservan los 20 más recientes con ese prefijo; los más antiguos se eliminan solo dentro de `backups/`.
- Si el backup falla, la escritura se aborta con un error `Database` y no se escribe nada.

### Reglas transversales
- Tras cada escritura: invalidar la caché de `library_info` y actualizar el `data_version` conocido, para no disparar el aviso de "cambio externo" por cambios propios.
- Los triggers existentes mantienen `juegos_busqueda` y `juegos_fts`. En los tests, tras cada operación, `integrity-check` de FTS y `juegos_busqueda` deben quedar sincronizados.
- No se escribe `imagen` en esta fase (portadas: Fase 5).

### Migraciones (ADR-009 aplicado)
| Comando | Retorno |
|---|---|
| `get_migration_status` | `{ current: number, latest: number, pending: { version: number, description: string }[] }` |
| `run_migrations` | `{ applied: number[], backupPath: string }` (requiere modo escritura; solo por acción explícita del usuario) |

- **v1:** `CREATE INDEX IF NOT EXISTS ix_juegos_titulo_nocase ON juegos(titulo COLLATE NOCASE)` + `ANALYZE`. Antes hay que ajustar la consulta del orden por código para que no empeore con el índice (ADR-011) y volver a medir.
- Versión en `app_metadata('desktop_schema_version')`. Proceso: backup → `BEGIN IMMEDIATE` → DDL → `UPDATE`/`INSERT` de la versión → `COMMIT` → `integrity_check` y `integrity-check` de FTS. Si algo falla, rollback.

### Notas de implementación (Fase 4, verificadas)
- `descripcion` y `ruta` también se recortan (trim), igual que el `guardar()` del legacy.
- Las comprobaciones de integridad de la migración se ejecutan **antes** del `COMMIT`, así que un fallo hace rollback de todo.
- SQLite no incrementa `data_version` por los commits de la propia conexión, así que las escrituras propias nunca disparan el aviso de "cambio externo". La caché de `library_info` sí se invalida.
- `create_backup` funciona también en modo solo lectura y no sustituye al backup automático de la sesión.
- `opts` es opcional en create. En update y delete, `expectedRevision` es obligatorio (`InvalidInput` si falta).
- Las tags se deduplican sin distinguir mayúsculas en ASCII, con la misma regla que `NOCASE`. Se rechazan caracteres NUL.
- `LibraryInfo.readOnly` refleja el modo real.

## Fase 5 — Portadas (ADR-014)

### Comandos nuevos
| Comando | Entrada | Retorno | Notas |
|---|---|---|---|
| `set_cover` | **Cuerpo en bruto** (`InvokeBody::Raw`, bytes de la imagen) + cabeceras `x-game-id` y `x-expected-revision` | `GameDetail` | `invoke('set_cover', uint8array, { headers })`. Un JSON como cuerpo, una cabecera ausente o no válida, o una imagen no soportada dan `InvalidInput`; otra revisión → `Conflict`; solo lectura → `ReadOnly`; juego inexistente → `NotFound` |
| `remove_cover` | `{ id, opts: { expectedRevision } }` | `GameDetail` | Deja `imagen = NULL`, igual que el legacy para un juego sin portada |
| `get_cover` | `{ id }` | `Cover \| null` | Se mantiene por compatibilidad (base64). El frontend ya no lo usa |

- **Procesamiento** (`images::process_cover`, en `spawn_blocking` y fuera del bloqueo de la DB): máximo 20 MiB; el formato se detecta por magic bytes (JPEG, PNG, WebP, GIF, BMP; otro → `InvalidInput "Formato no soportado"`); límites de decodificación de 12000x12000 y 512 MiB; el alfa se aplana sobre blanco; se reduce con Lanczos3 a un máximo de 300x300 conservando la proporción (nunca se amplía); se codifica WebP con pérdida q=65, method=6 (como `comprimir_imagen` del legacy). GIF animado: solo el primer fotograma.
- **Escritura:** misma ruta que Fase 4 (backup de sesión antes de la primera escritura, `BEGIN IMMEDIATE`, comprobación de revisión, solo se toca `imagen`). Los triggers de FTS no cambian.
- **`revision`:** ahora usa la huella de la portada `length(imagen)` + primeros 16 + últimos 16 bytes del BLOB (antes solo la longitud), para que reemplazar una portada por otra del mismo tamaño también cambie la revisión. El valor de `revision` de un registro con portada cambia respecto a Fase 4; es un token opaco y nada lo persiste.

### Protocolo `cover`
- URL: `http://cover.localhost/<id>?r=<tag>` en Windows, `cover://localhost/<id>?r=<tag>` en Linux/macOS. El frontend decide por `navigator.userAgent`. `r` solo sirve para invalidar la caché del webview; el servidor lo ignora.
- Solo `GET` (otro método → 405). Respuestas: `200` con el BLOB, `Content-Type` por magic bytes, `Cache-Control: public, max-age=31536000, immutable` y `X-Content-Type-Options: nosniff`; `404` si el juego no existe o no tiene portada; `400` si el id no es un entero positivo; `503` si la DB no está lista; `500` ante un error de lectura.
- La lectura se hace en `spawn_blocking` con la conexión de la app. No se carga nada más que la fila pedida.
- CSP: `img-src 'self' data: cover: http://cover.localhost`. Ventana con `dragDropEnabled: false` para recibir los eventos HTML5 de arrastrar y soltar.

### Frontend
- `coverUrl(id, tag)` y un contador por id (`invalidateCover`) más una época global (`clearCoverCache`) forman la etiqueta `r`. Se invalida tras `set_cover`/`remove_cover`, un refresco externo, cambio de DB y migraciones. El LRU base64 se retiró; el webview gestiona la memoria de las imágenes (`loading="lazy"`, `decoding="async"`).
- Edición: selector de archivo, arrastrar y soltar, y pegar (Ctrl+V con una imagen en el portapapeles mientras el formulario de edición está abierto). La portada se guarda al instante, independiente de «Guardar». En «Nuevo juego» la sección aparece tras crear el juego.

## Fase 6 — Importador (ADR-015)

### Comandos nuevos
| Comando | Entrada | Retorno | Notas |
|---|---|---|---|
| `scan_import` | `{ root: string, onProgress: Channel<ScanProgress> }` | `ImportPreview` | Solo lectura (nombres de entradas). Un único análisis a la vez (`InvalidInput` si ya hay uno). Carpeta inexistente → `InvalidInput`. Cancelado → `cancelled: true` e `items: []` |
| `cancel_scan` | — | `null` | Pide detener el análisis en curso; sin efecto si no hay ninguno |
| `run_import` | `{ req: ImportRequest }` | `ImportResult` | Backup de sesión + un único `BEGIN IMMEDIATE`. Revalida cada elemento y vuelve a comprobar duplicados (código y ruta) dentro de la transacción; los inválidos o duplicados se omiten con motivo, no abortan el lote. Solo lectura → `ReadOnly` |
| `get_import_history` | — | `HistoryEntry[]` | Últimas 20 **de la DB abierta**, de la más reciente a la más antigua |
| `undo_last_import` | — | `UndoResult` | Borra solo los ids de la última importación cuya `revision` no cambió; los demás van en `kept` con motivo. Solo considera importaciones de la DB abierta (campo `db` del registro: ruta canónica normalizada; los registros sin `db` nunca se deshacen). Ya deshecha → `InvalidInput`; ninguna de esta DB → `NotFound`. La lectura del historial, la comprobación y el borrado se serializan con el bloqueo de escritura. Si falla anotar el deshacer tras el commit → `Ok` con `warning` |

### Tipos (camelCase)
```ts
type ScanProgress = { scanned: number; candidates: number; currentDir: string };
type ParsedName = { titulo: string; codigo: string; estado: string; idioma: string };
type ItemStatus = "nuevo" | "duplicado_codigo" | "duplicado_ruta" | "duplicado_lote" | "sin_codigo";
type ImportItem = { key: string; kind: "archivo" | "carpeta"; path: string; name: string;
  parsed: ParsedName; status: ItemStatus; duplicateOf: { id: number | null; titulo: string } | null };
type ImportPreview = { root: string; items: ImportItem[]; defaults: { pagina: string; tipo: string };
  counts: { total; nuevo; duplicadoCodigo; duplicadoRuta; duplicadoLote; sinCodigo: number };
  cancelled: boolean; truncated: boolean; scanned: number; unreadable: number };
type ImportRequest = { root: string; pagina: string; tipo: string;
  items: { key: string; path: string; titulo: string; codigo: string; estado: string; idioma: string }[] };
type ImportResult = { inserted: { id: number; titulo: string; codigo: string }[];
  skipped: { key: string; reason: string }[]; backupPath: string | null; importId: string | null; warning: string | null };
type HistoryEntry = { id: string; timestamp: string; root: string; insertedCount: number; skippedCount: number;
  backupPath: string | null; undone: boolean; undoneAt: string | null };
type UndoResult = { importId: string; deleted: number; kept: { id: number; titulo: string; reason: string }[]; backupPath: string | null; warning: string | null };
```

### Reglas
- Los registros nuevos: `descripcion = ''`, `imagen = NULL`, `puntuacion = 0`, sin tags; `ruta` se guarda tal cual. FTS y `juegos_busqueda` se sincronizan por los triggers existentes.
- Límites: 20 000 candidatos por análisis (`truncated`), 50 000 elementos por importación; título ≤ 300, código ≤ 100, estado/idioma ≤ 50, página ≤ 100, tipo ≤ 50 (vacío → `Otro`), ruta ≤ 1000 caracteres.
- Historial: `app_data_dir/import-history.jsonl`, una línea JSON por importación (`kind: "import"`) o deshacer (`kind: "undo"`); las líneas corruptas se ignoran. Si no se puede escribir, la importación queda hecha y `warning` lo explica (no se podrá deshacer).
- Selector de carpeta: `@tauri-apps/plugin-dialog` `open({ directory: true })`, único permiso `dialog:allow-open`. También se acepta escribir la ruta.

### Parser (`importer::parse_name`), ajustado tras validar con la DB real
- **Código:** `RY[- _]+` seguido de `[A-Z0-9][A-Z0-9-]*`; el `_` **termina** el código (`RY-3713480_R18` → `RY-3713480`, `RY-RJ…_ENG` → `RY-RJ…`), salvo que el cuerpo sea una sola letra seguida de `_` y dígitos (`RY-d_642996` → `RY-D_642996`, corrige el bug del legacy). Se quitan los `-` finales; los segmentos con guion como `-EN`, `-V1` o `-EN-CN` **se conservan** en el código, porque así los guarda la DB real (quitarlos costaba 5 aciertos). Para el título se elimina todo el tramo pegado al código hasta el siguiente espacio, `[` o `(` (los sufijos `_R18`, `_ENG`, `_RJ…` no son título).
- **Estado:** un grupo entre corchetes cuyas palabras incluyan `F` (→ Terminado) o, si no, `P` (→ Empezado), sin distinguir mayúsculas: `[F]`, `[P bug]`, `[Demo F]`, `[P casi F]` (→ Terminado, como guarda la DB). O bien `F`/`P` como primer token fuera de corchetes. `F` tiene prioridad sobre `P`.
- **Idioma:** los tokens de idioma se buscan en el nombre completo (incluido lo pegado al código: `RY-1_EN-this`, `RY-1-EN-CN`).
- **Validación (`import_validate` sobre una copia, 1159 filas con ruta)** — legacy / nuevo: estado 1158/1156, idioma 1069/1069, código exacto 1136/1135, código normalizado 1136/1136. Diferencias intencionales (3): `[F] RY-d_642996` → `RY-D_642996` (la DB tiene el `RY-D` del bug); `[F]_RY-…` y `[F]RY-…` → Terminado (la DB tiene Sin empezar porque el legacy no veía la marca pegada). El centro de mantenimiento (Fase 7) las mostrará como estado inconsistente.

## Fase 7 — Centro de mantenimiento (ADR-016)

### Comandos nuevos
| Comando | Entrada | Retorno | Notas |
|---|---|---|---|
| `maintenance_summary` | — | `CheckCount[]` | Conteo de cada detector SQL (todos salvo `ruta_inexistente`). Solo lectura, sin BLOBs (`imagen IS NULL OR length(imagen)=0`) |
| `maintenance_list` | `{ req: { check: CheckId, offset: number, limit: number } }` | `IssuePage` | Paginado (limit 1..500). `ruta_inexistente` → `InvalidInput` (usar `scan_missing_paths`) |
| `scan_missing_paths` | `{ onProgress: Channel<{ checked: number; total: number }> }` | `{ items: Issue[]; cancelled: boolean }` | Comprueba `Path::exists` por lotes en `spawn_blocking`, cancelable con `cancel_maintenance_scan`. Solo metadatos; nunca abre ni modifica archivos. En Linux una ruta `X:\…` se informa con `detail: "ruta de Windows"` y no se comprueba; en Windows una ruta `/…` análogamente con `"ruta de Linux"`. Rutas vacías no cuentan (ya las cubre otro criterio si hiciera falta) |
| `cancel_maintenance_scan` | — | `null` | |
| `list_orphan_tags` | — | `{ id: number; nombre: string }[]` | Tags sin ningún `juego_tags` |
| `search_index_status` | — | `{ juegos: number; busqueda: number; mismatched: number; ftsOk: boolean }` | `mismatched` = filas de `juegos_busqueda` que no coinciden con `juegos` (faltan, sobran o difieren en titulo/codigo/descripcion/pagina/tipo/tags); `ftsOk` = `integrity-check` de FTS sin error |
| `preview_batch` | `{ req: BatchRequest }` | `BatchPreview` | Calcula los cambios sin escribir. Ids inexistentes → en `skipped` |
| `apply_batch` | `{ req: BatchRequest, revisions: Record<number, string> }` | `BatchResult` | Backup de sesión + un `BEGIN IMMEDIATE`. Para cada id, si su revisión actual ≠ la de `revisions` (la que vio la vista previa) → se omite con motivo. Escribe el registro de deshacer |
| `delete_orphan_tags` | — | `BatchResult` | Borra las tags huérfanas (registro de deshacer con sus nombres) |
| `rebuild_search_index` | — | `{ juegos; busqueda; mismatched; ftsOk }` | Resincroniza `juegos_busqueda` desde `juegos` (+tags) en una transacción y luego `INSERT INTO juegos_fts(juegos_fts) VALUES('rebuild')`. Backup de sesión previo. No toca `juegos` |
| `list_batches` | — | `BatchEntry[]` | Últimos 20 lotes de la DB abierta, del más reciente al más antiguo |
| `undo_last_batch` | — | `UndoBatchResult` | Restaura solo los registros cuya revisión actual = `revisionAfter`; los demás → `kept` con motivo. Ya deshecho → `InvalidInput`; ninguno → `NotFound`. Serializado con el bloqueo de escritura; si falla marcarlo como deshecho tras el commit → `Ok` con `warning` |

### Tipos (camelCase)
```ts
type CheckId = "sin_descripcion" | "sin_portada" | "sin_tags" | "codigo_duplicado" | "codigo_vacio"
  | "titulo_ry" | "estado_inconsistente" | "ruta_inexistente";
type CheckCount = { check: CheckId; count: number };
// detail: p. ej. "comparte código con #124 «…»", "archivo: [F] … → Terminado", "ruta de Windows"
type Issue = { id: number; titulo: string; codigo: string; estado: string; ruta: string; detail: string | null };
type IssuePage = { items: Issue[]; total: number; offset: number; limit: number };
type BatchAction =
  | { kind: "set_field"; field: "estado" | "idioma" | "tipo" | "pagina"; value: string }
  | { kind: "add_tag"; tag: string }
  | { kind: "remove_tag"; tag: string }
  | { kind: "fix_ry_titles" };
type BatchRequest = { ids: number[]; action: BatchAction };            // 1..5000 ids
type BatchChange = { id: number; titulo: string; before: string; after: string; revision: string };
type BatchPreview = { changes: BatchChange[]; skipped: { id: number; reason: string }[] };
type BatchResult = { changed: number; skipped: { id: number; reason: string }[]; backupPath: string | null;
  batchId: string | null; warning: string | null };
type BatchEntry = { id: string; timestamp: string; description: string; changed: number; undone: boolean };
type UndoBatchResult = { batchId: string; restored: number; kept: { id: number; titulo: string; reason: string }[];
  backupPath: string | null; warning: string | null };
```

### Reglas
- **Detectores** (todos ordenados por `id`, sin BLOBs):
  - `sin_descripcion`: `trim(coalesce(descripcion,'')) = ''`;
  - `sin_portada`: `imagen IS NULL OR length(imagen) = 0`;
  - `sin_tags`: sin filas en `juego_tags`;
  - `codigo_duplicado`: `codigo_norm` no vacío compartido por ≥ 2 juegos (`detail` nombra los otros ids);
  - `codigo_vacio`: `trim(coalesce(codigo,'')) = ''`;
  - `titulo_ry`: título que empieza por `RY` seguido de `-`, `_`, espacio o fin (sin distinguir mayúsculas);
  - `estado_inconsistente`: el nombre de archivo de `ruta` (sin `.zip/.rar/.7z`) lleva marca de estado según `importer::parse_name` (estado ≠ "Sin empezar") y difiere del estado guardado. Es informativo: el usuario decide.
- **Acciones:** `set_field` valida con las mismas reglas de longitud y NFC que `write::validate_input`; valor vacío → `InvalidInput`. `add_tag`/`remove_tag` usan la misma normalización que `save_tags` (NOCASE). `fix_ry_titles` propone el título de `parse_name(nombre de archivo de ruta)` solo si mejora (no vacío, no empieza por `RY`, distinto del actual); si no, el id va a `skipped`. Ningún lote toca `imagen`, `descripcion`, `puntuacion` ni `ruta`.
- **Registro de deshacer:** `app_data_dir/undo/<timestamp>-<n>.json` con `{ db, id, timestamp, description, entries: [{ id, before: {campo: valor} | { tags: string[] }, revisionAfter }], undone }`. `db` = misma clave que el historial del importador (`importer::db_key`). Solo se listan y deshacen los lotes de la DB abierta. `delete_orphan_tags` guarda los nombres y el deshacer los recrea (sin vínculos, que no tenían).
- **Cola de descripciones:** en la Fase 8 la sustituye la «Revisión por tandas» (ADR-017), que es un superconjunto. Mantenimiento ofrece un acceso directo a ella con el criterio `sin_descripcion`.
- Solo lectura: detectores y `scan_missing_paths` permitidos; lotes, `delete_orphan_tags`, `rebuild_search_index` y deshacer → `ReadOnly`.

### Notas de implementación (Fase 7, verificadas)
- `fix_ry_titles` solo actúa sobre títulos que empiezan por `RY` y descarta las propuestas de una sola palabra de ≤ 4 caracteres («propuesta poco fiable»). En la copia de la DB real, los 906 títulos `RY …` vienen de nombres sin título, así que casi nunca hay propuesta útil; la vista previa permite desmarcar filas.
- `scan_missing_paths` no comprueba rutas de red (`\servidor\…`), para evitar autenticación SMB saliente y bloqueos, ni rutas relativas; las informa con su `detail`. Suelta el bloqueo de la DB antes de tocar el disco.
- `mismatched` compara también `codigo_norm`. Si la conexión es de solo lectura, `ftsOk` usa una comprobación alternativa (las filas de `juegos_busqueda` frente a `juegos_fts_docsize`) porque FTS rechaza `integrity-check`.
- El deshacer solo restaura `titulo`, `estado`, `idioma`, `tipo`, `pagina` y la lista de tags; cualquier otra clave del registro → `InvalidInput`. Los registros se guardan como `undo/<YYYYMMDD-HHMMSS>-<nnn>.json`.
- `apply_batch` se envía solo con los ids marcados en la vista previa y sus revisiones; las claves numéricas llegan como texto en JSON y se deserializan como `i64` (verificado).

## Fase 8 — Funciones complementarias (ADR-017)

### Comando nuevo
| Comando | Entrada | Retorno | Notas |
|---|---|---|---|
| `get_dashboard` | — | `Dashboard` | Solo lectura. Lee una sola vez las columnas anteriores a `imagen` más `length(imagen)` (nunca el BLOB) y los conteos de tags. Valores vacíos o NULL se agrupan como `""` |

```ts
type Dashboard = {
  total: number;
  estados: CountItem[]; idiomas: CountItem[]; tipos: CountItem[]; paginas: CountItem[];   // orden: count desc, value asc
  puntuaciones: CountItem[];          // value "0.0".."5.0" (pasos de 0.5), los 11 siempre presentes
  estadoIdioma: { estado: string; idioma: string; count: number }[];
  cobertura: { conPortada: number; conDescripcion: number; conTags: number; conRuta: number };
  topTags: CountItem[];               // las 15 tags más usadas
  elapsedMs: number;
};
```

### Frontend (sin backend nuevo)
- **Favoritos:** `FAVORITE_TAG = "Favorito"`. Comando `set_favorite { id, favorite: boolean, opts: { expectedRevision } } → GameDetail`. **Solo toca `juego_tags` (y crea la tag `Favorito` si no existe)**: no revalida ni normaliza el resto de campos, así que los registros antiguos con valores fuera de rango también se pueden marcar. Comprueba la revisión (otra → `Conflict`); solo lectura → `ReadOnly`; usa backup de sesión y `BEGIN IMMEDIATE`. La tag se busca sin distinguir mayúsculas (`NOCASE`). Conflicto en la interfaz → mensaje y recarga.
- **Revisión por tandas:** pide los ids con `maintenance_list` (páginas de 500) del detector elegido, los baraja (Fisher–Yates) y toma N. Edita con el `GameForm` existente.
- **Atajos:** una tabla única `SHORTCUTS` en `lib/` que alimenta tanto el manejador como el diálogo de ayuda.
