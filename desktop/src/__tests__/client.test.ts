import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { getGame, isAppError, searchGames } from "../api/client";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mockInvoke = vi.mocked(invoke);

describe("client", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("passes command names and args per contract", async () => {
    mockInvoke.mockResolvedValue({ items: [] });
    const req = { query: "x", sort: "code" as const, offset: 0, limit: 100 };
    await searchGames(req);
    expect(mockInvoke).toHaveBeenCalledWith("search_games", { req });
    await getGame(7);
    expect(mockInvoke).toHaveBeenCalledWith("get_game", { id: 7 });
  });

  it("keeps backend AppError as is", async () => {
    mockInvoke.mockRejectedValue({ kind: "NotFound", message: "no existe" });
    await expect(getGame(1)).rejects.toEqual({ kind: "NotFound", message: "no existe" });
  });

  it("normalizes unknown errors to Database", async () => {
    mockInvoke.mockRejectedValue("boom");
    await expect(getGame(1)).rejects.toEqual({ kind: "Database", message: "boom" });
    mockInvoke.mockRejectedValue(new Error("fail"));
    await expect(getGame(1)).rejects.toEqual({ kind: "Database", message: "fail" });
    mockInvoke.mockRejectedValue({ kind: "Weird", message: 3 });
    await expect(getGame(1)).rejects.toMatchObject({ kind: "Database" });
  });

  it("isAppError validates shape", () => {
    expect(isAppError({ kind: "Config", message: "m" })).toBe(true);
    expect(isAppError({ kind: "X", message: "m" })).toBe(false);
    expect(isAppError(null)).toBe(false);
  });
});
