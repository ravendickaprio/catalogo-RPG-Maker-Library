# Contrato Fase 4 — CRUD (se fusionará en CONTRACTS.md)

## Modo de apertura y configuración
- **Ajustes** en `<app_config_dir>/settings.json`, donde `app_config_dir` es la ruta de Tauri (`%APPDATA%\local.rpgmakerlibrary.desktop` en Windows y `~/.config/local.rpgmakerlibrary.desktop` en Linux). Contenido: `{ "dbPath": string | null, "readOnly": boolean }`.
- **Orden de resolución de la ruta:** `RPGLIB_DB_PATH` → `settings.dbPath` → copia de desarrollo. Ya no se rechaza `juegos.db`, porque el usuario lo autorizó el 2026-11-27. El ADR-003 queda reemplazado por el ADR-012.
- **Modo:** lectura y escritura salvo `settings.readOnly = true` o `RPGLIB_READ_ONLY=1`.
- **Conexión de escritura:** `foreign_keys = ON` y `busy_timeout = 5000`. Las escrituras usan `BEGIN IMMEDIATE`.
- **Validación de esquema al abrir:** `juegos` debe tener las 11 columnas del legacy, más `tags` y `juego_tags`. Si falta algo → `Config`, con la lista de faltantes. La app no crea ni altera nada al abrir.

## Tipos nuevos
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

## Comandos nuevos
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

## Backups (ADR-012)
- Antes de la **primera escritura de cada sesión** se hace un backup automático con la API de backup: `<dbDir>/backups/<nombre>.bak-AAAAMMDD-HHMMSS`. Se conservan los 20 más recientes con ese prefijo; los más antiguos se eliminan solo dentro de `backups/`.
- Si el backup falla, la escritura se aborta con un error `Database` y no se escribe nada.

## Reglas transversales
- Tras cada escritura: invalidar la caché de `library_info` y actualizar el `data_version` conocido, para no disparar el aviso de "cambio externo" por cambios propios.
- Los triggers existentes mantienen `juegos_busqueda` y `juegos_fts`. En los tests, tras cada operación, `integrity-check` de FTS y `juegos_busqueda` deben quedar sincronizados.
- No se escribe `imagen` en esta fase (portadas: Fase 5).

## Migraciones (ADR-009 aplicado)
| Comando | Retorno |
|---|---|
| `get_migration_status` | `{ current: number, latest: number, pending: { version: number, description: string }[] }` |
| `run_migrations` | `{ applied: number[], backupPath: string }` (requiere modo escritura; solo por acción explícita del usuario) |

- **v1:** `CREATE INDEX IF NOT EXISTS ix_juegos_titulo_nocase ON juegos(titulo COLLATE NOCASE)` + `ANALYZE`. Antes hay que ajustar la consulta del orden por código para que no empeore con el índice (ADR-011) y volver a medir.
- Versión en `app_metadata('desktop_schema_version')`. Proceso: backup → `BEGIN IMMEDIATE` → DDL → `UPDATE`/`INSERT` de la versión → `COMMIT` → `integrity_check` y `integrity-check` de FTS. Si algo falla, rollback.
