import { invoke } from "@tauri-apps/api/core";
import type {
  AppError,
  Cover,
  DataVersion,
  GameDetail,
  LibraryInfo,
  SearchPage,
  SearchRequest,
  SuggestRequest,
  Suggestion,
} from "./types";

const KINDS = ["NotFound", "InvalidInput", "Database", "Config"];

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
export const suggestGame = (req: SuggestRequest) => call<Suggestion>("suggest_game", { req });
export const checkDataVersion = () => call<DataVersion>("check_data_version");
