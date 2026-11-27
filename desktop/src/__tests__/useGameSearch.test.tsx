import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { PAGE_SIZE, useGameSearch, type SearchParams } from "../hooks/useGameSearch";
import type { GameSummary, SearchPage, SearchRequest } from "../api/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mockInvoke = vi.mocked(invoke);

const game = (id: number): GameSummary => ({
  id, titulo: `G${id}`, codigo: `RJ${id}`, estado: "", idioma: "", tipo: "", pagina: "", puntuacion: 0, hasCover: false,
});
const page = (ids: number[], total: number, offset = 0): SearchPage => ({
  items: ids.map(game), total, offset, limit: PAGE_SIZE, elapsedMs: 1, strategy: "none",
});
const base: SearchParams = { query: "", estado: "", idioma: "", tag: "", sort: "title_asc" };
const reqs = () => mockInvoke.mock.calls.filter((c) => c[0] === "search_games").map((c) => (c[1] as { req: SearchRequest }).req);

describe("useGameSearch", () => {
  beforeEach(() => mockInvoke.mockReset());

  it("sends tag, estado and idioma (empty strings become null)", async () => {
    mockInvoke.mockResolvedValue(page([1], 1));
    const { result, rerender } = renderHook((p: SearchParams) => useGameSearch(p), { initialProps: base });
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(reqs()[0]).toMatchObject({ estado: null, idioma: null, tag: null, offset: 0, limit: PAGE_SIZE });
    rerender({ ...base, estado: "Terminado", idioma: "Español", tag: "fantasía" });
    await waitFor(() => expect(reqs().length).toBe(2));
    expect(reqs()[1]).toMatchObject({ estado: "Terminado", idioma: "Español", tag: "fantasía", offset: 0 });
  });

  it("ignores a stale response resolved after a filter change", async () => {
    let resolveOld!: (p: SearchPage) => void;
    mockInvoke.mockImplementationOnce(() => new Promise((r) => (resolveOld = r)));
    mockInvoke.mockResolvedValueOnce(page([2], 1));
    const { result, rerender } = renderHook((p: SearchParams) => useGameSearch(p), { initialProps: base });
    rerender({ ...base, tag: "x" });
    await waitFor(() => expect(result.current.items.map((g) => g.id)).toEqual([2]));
    await act(async () => resolveOld(page([1, 99], 2)));
    expect(result.current.items.map((g) => g.id)).toEqual([2]);
    expect(result.current.total).toBe(1);
  });

  it("loadMore appends using the loaded count as offset", async () => {
    const first = Array.from({ length: PAGE_SIZE }, (_, i) => i + 1);
    mockInvoke.mockResolvedValueOnce(page(first, PAGE_SIZE + 2));
    mockInvoke.mockResolvedValueOnce(page([1001, 1002], PAGE_SIZE + 2, PAGE_SIZE));
    const { result } = renderHook((p: SearchParams) => useGameSearch(p), { initialProps: { ...base, tag: "t" } });
    await waitFor(() => expect(result.current.loading).toBe(false));
    act(() => result.current.loadMore());
    await waitFor(() => expect(result.current.items.length).toBe(PAGE_SIZE + 2));
    expect(reqs()[1]).toMatchObject({ offset: PAGE_SIZE, tag: "t" });
    expect(result.current.items.slice(-2).map((g) => g.id)).toEqual([1001, 1002]);
  });
});
