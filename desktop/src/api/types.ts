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
  /** Record fingerprint used for optimistic concurrency (expectedRevision). */
  revision: string;
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

export type AppErrorKind =
  | "NotFound"
  | "InvalidInput"
  | "Database"
  | "Config"
  | "Conflict"
  | "DuplicateCode"
  | "ReadOnly"
  | "Busy";

export interface AppError {
  kind: AppErrorKind;
  message: string;
}

export interface DataVersion {
  version: number;
  changed: boolean;
}

export interface GameInput {
  titulo: string;
  descripcion: string;
  estado: string;
  idioma: string;
  ruta: string;
  codigo: string;
  pagina: string;
  tipo: string;
  puntuacion: number;
  tags: string[];
}

export interface WriteOptions {
  allowDuplicateCode?: boolean;
  expectedRevision?: string | null;
}

export interface Vocabulary {
  estados: string[];
  idiomas: string[];
  paginas: string[];
  tipos: string[];
  puntuaciones: number[];
}

export interface SessionInfo {
  dbFileName: string;
  dbDir: string;
  readOnly: boolean;
  lastBackup: string | null;
}

export interface BackupResult {
  path: string;
}

export interface MigrationStatus {
  current: number;
  latest: number;
  pending: { version: number; description: string }[];
}

export interface MigrationResult {
  applied: number[];
  backupPath: string;
}

// Fase 6 — Importador (ADR-015)
export interface ScanProgress {
  scanned: number;
  candidates: number;
  currentDir: string;
}
export interface ParsedName {
  titulo: string;
  codigo: string;
  estado: string;
  idioma: string;
}
export type ItemStatus = "nuevo" | "duplicado_codigo" | "duplicado_ruta" | "duplicado_lote" | "sin_codigo";
export interface ImportItem {
  key: string;
  kind: "archivo" | "carpeta";
  path: string;
  name: string;
  parsed: ParsedName;
  status: ItemStatus;
  duplicateOf: { id: number | null; titulo: string } | null;
}
export interface ImportPreview {
  root: string;
  items: ImportItem[];
  defaults: { pagina: string; tipo: string };
  counts: { total: number; nuevo: number; duplicadoCodigo: number; duplicadoRuta: number; duplicadoLote: number; sinCodigo: number };
  cancelled: boolean;
  truncated: boolean;
  scanned: number;
  unreadable: number;
}
export interface ImportRequest {
  root: string;
  pagina: string;
  tipo: string;
  items: { key: string; path: string; titulo: string; codigo: string; estado: string; idioma: string }[];
}
export interface ImportResult {
  inserted: { id: number; titulo: string; codigo: string }[];
  skipped: { key: string; reason: string }[];
  backupPath: string | null;
  importId: string | null;
  warning: string | null;
}
export interface HistoryEntry {
  id: string;
  timestamp: string;
  root: string;
  insertedCount: number;
  skippedCount: number;
  backupPath: string | null;
  undone: boolean;
  undoneAt: string | null;
}
export interface UndoResult {
  importId: string;
  deleted: number;
  kept: { id: number; titulo: string; reason: string }[];
  backupPath: string | null;
  warning: string | null;
}
