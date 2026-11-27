# Contratos frontend ↔ backend (Fase 1)

Comandos Tauri invocados con `invoke(nombre, args)`. Los DTO se serializan en **camelCase** (`#[serde(rename_all = "camelCase")]`). Los errores siguen siempre la forma `AppError { kind, message }`. Todos los comandos son `async` y ejecutan SQLite con `spawn_blocking`. La conexión es de solo lectura (ADR-003).

## Tipos

```ts
type SortKey = "title_asc" | "title_desc" | "code" | "status" | "language" | "recent";

interface SearchRequest {
  query: string;          // ≤ 200 caracteres tras trim; "" = sin texto
  estado?: string | null; // valor literal de juegos.estado; null = todos
  idioma?: string | null; // valor literal de juegos.idioma; null = todos
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
  sqliteVersion: string;
}

interface SuggestRequest { excludeId?: number | null }
interface Suggestion {
  game: GameDetail | null;   // null = sin candidatos
  eligibleCount: number;     // juegos no terminados
  totalCount: number;
}

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

## Reglas de backend
- Nunca seleccionar `imagen` excepto en `get_cover`. `hasCover = coalesce(length(imagen),0) > 0`.
- Sin N+1: las tags del detalle se obtienen en una sola consulta. Los listados no incluyen tags.
- Orden: `title_asc` = `titulo COLLATE NOCASE, id`; `title_desc` = inverso; `code` = sin código al final, luego `codigo COLLATE NOCASE, titulo`; `status` = `CASE estado WHEN 'Sin empezar' THEN 0 WHEN 'Empezado' THEN 1 WHEN 'Terminado' THEN 2 ELSE 3 END, titulo`; `language` = `idioma COLLATE NOCASE, titulo`; `recent` = `id DESC`.
- Validación: query ≤ 200 caracteres, limit 1..=200, offset ≥ 0, id > 0. Si falla → `InvalidInput`.
- Elegibilidad de la sugerencia: `lower(trim(coalesce(estado,''))) <> 'terminado'`. Con `excludeId` y más de un candidato, se excluye ese id.

## Reglas de frontend
- Debounce de 150 ms en la búsqueda. Cada petición lleva un número de secuencia y se descartan respuestas obsoletas.
- Lista virtualizada (TanStack Virtual) con paginación incremental de 100 en 100 al hacer scroll.
- La portada se pide solo al abrir el detalle o la sugerencia.
- La sugerencia se muestra en un diálogo accesible (`role="dialog"`, `aria-modal`, Esc para cerrar, foco inicial). Botones "Otra sugerencia" y "Ver en la lista". Nada se escribe.
