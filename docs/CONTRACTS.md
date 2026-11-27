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
