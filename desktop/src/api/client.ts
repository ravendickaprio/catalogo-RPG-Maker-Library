import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  AppError,
  BatchEntry,
  BatchPreview,
  BatchRequest,
  BatchResult,
  CheckCount,
  CheckId,
  Dashboard,
  IndexStatus,
  IssuePage,
  MissingScanProgress,
  MissingScanResult,
  OrphanTag,
  UndoBatchResult,
  BackupResult,
  BackupEntry,
  BackupCheck,
  RestoreResult,
  ExportFormat,
  ExportResult,
  Cover,
  DataVersion,
  GameDetail,
  GameInput,
  HistoryEntry,
  ImportPreview,
  ImportRequest,
  ImportResult,
  ScanProgress,
  UndoResult,
  LibraryInfo,
  MigrationResult,
  MigrationStatus,
  SearchPage,
  SearchRequest,
  SessionInfo,
  SuggestRequest,
  Suggestion,
  Vocabulary,
  WriteOptions,
} from "./types";

const KINDS = ["NotFound", "InvalidInput", "Database", "Config", "Conflict", "DuplicateCode", "ReadOnly", "Busy"];

export function isAppError(e: unknown): e is AppError {
  if (typeof e !== "object" || e === null) return false;
  const o = e as Record<string, unknown>;
  return typeof o.kind === "string" && KINDS.includes(o.kind) && typeof o.message === "string";
}

export function normalizeError(e: unknown): AppError {
  if (isAppError(e)) return { kind: e.kind, message: e.message };
  const message = e instanceof Error ? e.message : String(e);
  return { kind: "Database", message };
}

async function callRaw<T>(cmd: string, body: Uint8Array, headers: Record<string, string>): Promise<T> {
  try {
    return await invoke<T>(cmd, body, { headers });
  } catch (e) {
    throw normalizeError(e);
  }
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw normalizeError(e);
  }
}

export const getLibraryInfo = () => call<LibraryInfo>("get_library_info");
export const searchGames = (req: SearchRequest) => call<SearchPage>("search_games", { req });
export const getGame = (id: number) => call<GameDetail>("get_game", { id });
export const getCover = (id: number) => call<Cover | null>("get_cover", { id });
/** Raw bytes (no base64); the backend decodes, resizes to 300x300 WebP and stores it (ADR-014). */
export const setCover = (id: number, bytes: Uint8Array, expectedRevision: string) =>
  callRaw<GameDetail>("set_cover", bytes, { "x-game-id": String(id), "x-expected-revision": expectedRevision });
export const removeCover = (id: number, expectedRevision: string) =>
  call<GameDetail>("remove_cover", { id, opts: { expectedRevision } });
export const suggestGame = (req: SuggestRequest) => call<Suggestion>("suggest_game", { req });
export const checkDataVersion = () => call<DataVersion>("check_data_version");

export const getVocabulary = () => call<Vocabulary>("get_vocabulary");
export const getSessionInfo = () => call<SessionInfo>("get_session_info");
export const createGame = (input: GameInput, opts: WriteOptions = {}) => call<GameDetail>("create_game", { input, opts });
export const updateGame = (id: number, input: GameInput, opts: WriteOptions = {}) =>
  call<GameDetail>("update_game", { id, input, opts });
export const setFavorite = (id: number, favorite: boolean, expectedRevision: string) =>
  call<GameDetail>("set_favorite", { id, favorite, opts: { expectedRevision } });
export const deleteGame = (id: number, opts: WriteOptions) => call<null>("delete_game", { id, opts });
export const createBackup = () => call<BackupResult>("create_backup");
export const listBackups = () => call<BackupEntry[]>("list_backups");
export const verifyBackup = (fileName: string) => call<BackupCheck>("verify_backup", { fileName });
export const restoreBackup = (fileName: string) => call<RestoreResult>("restore_backup", { fileName });
export const checkDatabase = () => call<BackupCheck>("check_database");
export const exportLibrary = (path: string, format: ExportFormat) => call<ExportResult>("export_library", { path, format });
export const setDbPath = (path: string) => call<LibraryInfo>("set_db_path", { path });
export const setReadOnly = (readOnly: boolean) => call<SessionInfo>("set_read_only", { readOnly });
export const getMigrationStatus = () => call<MigrationStatus>("get_migration_status");
export const runMigrations = () => call<MigrationResult>("run_migrations");

export const scanImport = (root: string, onProgress: (p: ScanProgress) => void) => {
  const channel = new Channel<ScanProgress>();
  channel.onmessage = onProgress;
  return call<ImportPreview>("scan_import", { root, onProgress: channel });
};
export const cancelScan = () => call<null>("cancel_scan");
export const runImport = (req: ImportRequest) => call<ImportResult>("run_import", { req });
export const getImportHistory = () => call<HistoryEntry[]>("get_import_history");
export const undoLastImport = () => call<UndoResult>("undo_last_import");

// Fase 7 — Centro de mantenimiento
export const maintenanceSummary = () => call<CheckCount[]>("maintenance_summary");
export const maintenanceList = (check: CheckId, offset: number, limit: number) =>
  call<IssuePage>("maintenance_list", { req: { check, offset, limit } });
export const scanMissingPaths = (onProgress: (p: MissingScanProgress) => void) => {
  const channel = new Channel<MissingScanProgress>();
  channel.onmessage = onProgress;
  return call<MissingScanResult>("scan_missing_paths", { onProgress: channel });
};
export const cancelMaintenanceScan = () => call<null>("cancel_maintenance_scan");
export const listOrphanTags = () => call<OrphanTag[]>("list_orphan_tags");
export const searchIndexStatus = () => call<IndexStatus>("search_index_status");
export const previewBatch = (req: BatchRequest) => call<BatchPreview>("preview_batch", { req });
export const applyBatch = (req: BatchRequest, revisions: Record<number, string>) => call<BatchResult>("apply_batch", { req, revisions });
export const deleteOrphanTags = () => call<BatchResult>("delete_orphan_tags");
export const rebuildSearchIndex = () => call<IndexStatus>("rebuild_search_index");
export const listBatches = () => call<BatchEntry[]>("list_batches");
export const undoLastBatch = () => call<UndoBatchResult>("undo_last_batch");

// Fase 8
export const getDashboard = () => call<Dashboard>("get_dashboard");
