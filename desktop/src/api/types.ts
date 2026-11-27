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

export interface BackupEntry {
  fileName: string;
  size: number;
  /** ISO timestamp */
  modified: string;
  preRestore: boolean;
}

export interface BackupCheck {
  ok: boolean;
  integrity: string;
  foreignKeys: number;
  ftsOk: boolean | null;
  games: number;
  schemaOk: boolean;
}

export interface RestoreResult {
  restoredFrom: string;
  preRestoreBackup: string;
  games: number;
  /** Copy succeeded but the database could not be reopened. */
  warning?: string | null;
}

export type ExportFormat = "csv" | "json";

export interface ExportResult {
  path: string;
  rows: number;
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

// Fase 7 — Centro de mantenimiento (ADR-016)
export type CheckId =
  | "sin_descripcion"
  | "sin_portada"
  | "sin_tags"
  | "codigo_duplicado"
  | "codigo_vacio"
  | "titulo_ry"
  | "estado_inconsistente"
  | "ruta_inexistente";
export interface CheckCount {
  check: CheckId;
  count: number;
}
export interface Issue {
  id: number;
  titulo: string;
  codigo: string;
  estado: string;
  ruta: string;
  detail: string | null;
}
export interface IssuePage {
  items: Issue[];
  total: number;
  offset: number;
  limit: number;
}
export type BatchField = "estado" | "idioma" | "tipo" | "pagina";
export type BatchAction =
  | { kind: "set_field"; field: BatchField; value: string }
  | { kind: "add_tag"; tag: string }
  | { kind: "remove_tag"; tag: string }
  | { kind: "fix_ry_titles" };
export interface BatchRequest {
  ids: number[];
  action: BatchAction;
}
export interface BatchChange {
  id: number;
  titulo: string;
  before: string;
  after: string;
  revision: string;
}
export interface BatchSkip {
  id: number;
  reason: string;
}
export interface BatchPreview {
  changes: BatchChange[];
  skipped: BatchSkip[];
}
export interface BatchResult {
  changed: number;
  skipped: BatchSkip[];
  backupPath: string | null;
  batchId: string | null;
  warning: string | null;
}
export interface BatchEntry {
  id: string;
  timestamp: string;
  description: string;
  changed: number;
  undone: boolean;
}
export interface UndoBatchResult {
  batchId: string;
  restored: number;
  kept: { id: number; titulo: string; reason: string }[];
  backupPath: string | null;
  warning: string | null;
}
export interface MissingScanProgress {
  checked: number;
  total: number;
}
export interface MissingScanResult {
  items: Issue[];
  cancelled: boolean;
}
export interface IndexStatus {
  juegos: number;
  busqueda: number;
  mismatched: number;
  ftsOk: boolean;
}
export interface OrphanTag {
  id: number;
  nombre: string;
}

// Fase 8 — Dashboard
export interface Dashboard {
  total: number;
  estados: CountItem[];
  idiomas: CountItem[];
  tipos: CountItem[];
  paginas: CountItem[];
  /** Always the 11 buckets "0.0".."5.0". */
  puntuaciones: CountItem[];
  estadoIdioma: { estado: string; idioma: string; count: number }[];
  cobertura: { conPortada: number; conDescripcion: number; conTags: number; conRuta: number };
  topTags: CountItem[];
  elapsedMs: number;
}
