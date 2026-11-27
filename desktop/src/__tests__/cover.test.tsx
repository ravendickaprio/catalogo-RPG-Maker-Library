import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import GameDetail from "../components/GameDetail";
import CoverImage from "../components/CoverImage";
import { MAX_COVER_BYTES, clearCoverCache, coverUrl, invalidateCover, validateCoverFile } from "../lib/cover";
import type { GameDetail as Detail, Vocabulary } from "../api/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mockInvoke = vi.mocked(invoke);

const vocab: Vocabulary = { estados: ["Sin empezar", "Terminado"], idiomas: ["Español"], paginas: ["A"], tipos: ["RPG"], puntuaciones: [0] };
const game = (over: Partial<Detail> = {}): Detail => ({
  id: 7, titulo: "Mi juego", codigo: "RJ001", estado: "Sin empezar", idioma: "Español", tipo: "RPG", pagina: "A",
  puntuacion: 3, hasCover: false, descripcion: "d", ruta: "", tags: [], revision: "rev1", ...over,
});
const png = (size = 4, type = "image/png") => new File([new Uint8Array(size).fill(1)], "c.png", { type });
const calls = (cmd: string) => mockInvoke.mock.calls.filter((c) => c[0] === cmd);

let current: Detail;
beforeEach(() => {
  mockInvoke.mockReset();
  current = game();
  mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === "get_game") return current;
    throw new Error("unexpected " + cmd);
  });
});

async function openEdit() {
  render(<GameDetail id={7} vocab={vocab} />);
  fireEvent.click(await screen.findByRole("button", { name: "Editar" }));
  return screen.findByTestId("cover-file");
}
const pick = (input: HTMLElement, f: File) => fireEvent.change(input, { target: { files: [f] } });

describe("coverUrl / validation", () => {
  it("builds the protocol URL per platform with a cache-busting tag", () => {
    expect(coverUrl(5, "0.2", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")).toBe("http://cover.localhost/5?r=0.2");
    expect(coverUrl(5, 3, "Mozilla/5.0 (X11; Linux x86_64)")).toBe("cover://localhost/5?r=3");
  });
  it("rejects non-images and files over 20 MiB client-side", () => {
    expect(validateCoverFile({ type: "image/png", size: 10 })).toBeNull();
    expect(validateCoverFile({ type: "", size: 10 })).toBeNull(); // backend checks magic bytes
    expect(validateCoverFile({ type: "application/pdf", size: 10 })).toMatch(/no es una imagen/);
    expect(validateCoverFile({ type: "image/png", size: MAX_COVER_BYTES + 1 })).toMatch(/20 MB/);
    expect(validateCoverFile({ type: "image/png", size: MAX_COVER_BYTES })).toBeNull();
  });
});

describe("CoverImage", () => {
  it("uses lazy async <img>, changes URL on invalidation and falls back on error", () => {
    const ph = (failed: boolean) => <span data-testid="ph">{failed ? "falló" : "vacío"}</span>;
    const { rerender } = render(<CoverImage id={42} hasCover alt="x" placeholder={ph} />);
    const img = screen.getByRole("img") as HTMLImageElement;
    expect(img.getAttribute("loading")).toBe("lazy");
    expect(img.getAttribute("decoding")).toBe("async");
    expect(img.src).toMatch(/\/42\?r=/);
    const before = img.src;
    act(() => invalidateCover(42));
    expect((screen.getByRole("img") as HTMLImageElement).src).not.toBe(before);
    act(() => clearCoverCache());
    fireEvent.error(screen.getByRole("img"));
    expect(screen.queryByRole("img")).toBeNull();
    expect(screen.getByTestId("ph").textContent).toBe("falló");
    rerender(<CoverImage id={42} hasCover={false} alt="x" placeholder={ph} />);
    expect(screen.getByTestId("ph").textContent).toBe("vacío");
  });
});

describe("cover editing in the form", () => {
  it("sends raw bytes with id and revision headers, then uses the new revision", async () => {
    const input = await openEdit();
    expect(screen.getByText(/se guarda al instante/)).toBeTruthy();
    const updated = game({ hasCover: true, revision: "rev2" });
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "set_cover") return updated;
      if (cmd === "remove_cover") return game({ revision: "rev3" });
      throw new Error("unexpected " + cmd);
    });
    pick(input, png(4));
    await waitFor(() => expect(calls("set_cover")).toHaveLength(1));
    const [, body, opts] = calls("set_cover")[0] as [string, Uint8Array, { headers: Record<string, string> }];
    expect(body).toBeInstanceOf(Uint8Array);
    expect(body.length).toBe(4);
    expect(opts.headers).toEqual({ "x-game-id": "7", "x-expected-revision": "rev1" });
    // the record now has a cover: "Quitar portada" is enabled and confirms before calling
    const rm = await screen.findByRole("button", { name: "Quitar portada" });
    await waitFor(() => expect((rm as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(rm);
    expect(calls("remove_cover")).toHaveLength(0);
    fireEvent.click(await screen.findByRole("button", { name: "Quitar" }));
    await waitFor(() => expect(calls("remove_cover")).toHaveLength(1));
    expect(calls("remove_cover")[0][1]).toEqual({ id: 7, opts: { expectedRevision: "rev2" } });
  });

  it("rejects wrong type and oversize files without calling the backend", async () => {
    const input = await openEdit();
    pick(input, png(4, "application/pdf"));
    expect((await screen.findByRole("alert")).textContent).toMatch(/no es una imagen/);
    const big = png(4);
    Object.defineProperty(big, "size", { value: MAX_COVER_BYTES + 1 });
    pick(input, big);
    await waitFor(() => expect(screen.getByRole("alert").textContent).toMatch(/20 MB/));
    expect(calls("set_cover")).toHaveLength(0);
  });

  it("accepts an image pasted from the clipboard but leaves text paste alone", async () => {
    await openEdit();
    mockInvoke.mockImplementation(async () => game({ hasCover: true, revision: "rev2" }));
    const text = fireEvent.paste(screen.getByLabelText(/^Título/), { clipboardData: { files: [] } });
    expect(text).toBe(true); // not prevented
    expect(calls("set_cover")).toHaveLength(0);
    fireEvent.paste(screen.getByLabelText(/^Título/), { clipboardData: { files: [png(4)] } });
    await waitFor(() => expect(calls("set_cover")).toHaveLength(1));
  });

  it("accepts a dropped image", async () => {
    await openEdit();
    mockInvoke.mockImplementation(async () => game({ hasCover: true, revision: "rev2" }));
    fireEvent.drop(screen.getByRole("group", { name: /Zona de portada/ }), { dataTransfer: { files: [png(4)] } });
    await waitFor(() => expect(calls("set_cover")).toHaveLength(1));
  });

  it("shows the conflict banner when the revision is stale", async () => {
    const input = await openEdit();
    mockInvoke.mockImplementation(async () => Promise.reject({ kind: "Conflict", message: "x" }));
    pick(input, png(4));
    expect(await screen.findByText(/cambió en otra aplicación/)).toBeTruthy();
  });

  it("shows backend InvalidInput next to the cover", async () => {
    const input = await openEdit();
    mockInvoke.mockImplementation(async () => Promise.reject({ kind: "InvalidInput", message: "Formato no soportado" }));
    pick(input, png(4));
    expect((await screen.findByRole("alert")).textContent).toBe("Formato no soportado");
  });

  it("has no cover section for a new game", async () => {
    render(<GameDetail id={null} creating vocab={vocab} />);
    expect(await screen.findByText(/después de crear el juego/)).toBeTruthy();
    expect(screen.queryByTestId("cover-file")).toBeNull();
  });
});
