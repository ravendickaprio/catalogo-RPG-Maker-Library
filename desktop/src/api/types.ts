export type SortKey = "title_asc" | "title_desc" | "code" | "status" | "language" | "recent";

export interface SearchRequest {
  query: string;
  estado?: string | null;
  idioma?: string | null;
  tag?: string | null;
  sort: SortKey;
  offset: number;
  limit: number;
}

export interface GameSummary {
  id: number;
  titulo: string;
  codigo: string;
  estado: string;
  idioma: string;
  tipo: string;
  pagina: string;
  puntuacion: number;
  hasCover: boolean;
}

export interface SearchPage {
  items: GameSummary[];
  total: number;
  offset: number;
  limit: number;
  elapsedMs: number;
  strategy: "fts" | "like" | "none";
}

export interface GameDetail extends GameSummary {
  descripcion: string;
  ruta: string;
  tags: string[];
}

export interface Cover {
  mime: string;
  dataBase64: string;
}

export interface CountItem {
  value: string;
  count: number;
}

export interface LibraryInfo {
  dbFileName: string;
  readOnly: boolean;
  total: number;
  ftsAvailable: boolean;
  ftsInSync: boolean;
  estados: CountItem[];
  idiomas: CountItem[];
  tags: CountItem[];
  sqliteVersion: string;
}

export interface SuggestRequest {
  excludeId?: number | null;
}

export interface Suggestion {
  game: GameDetail | null;
  eligibleCount: number;
  totalCount: number;
}

export type AppErrorKind = "NotFound" | "InvalidInput" | "Database" | "Config";

export interface AppError {
  kind: AppErrorKind;
  message: string;
}
