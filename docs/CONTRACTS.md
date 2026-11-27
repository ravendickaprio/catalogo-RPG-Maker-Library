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
