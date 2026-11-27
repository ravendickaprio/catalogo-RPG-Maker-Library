import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import SuggestionDialog from "../components/SuggestionDialog";
import type { GameDetail, Suggestion } from "../api/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mockInvoke = vi.mocked(invoke);

const game = (id: number, titulo: string): GameDetail => ({
  id, titulo, codigo: `RJ${id}`, estado: "Sin empezar", idioma: "Español", tipo: "RPG", pagina: "p",
  puntuacion: 3, hasCover: false, descripcion: "Una descripción larga", ruta: "C:/x", tags: ["fantasía"], revision: "r1",
});

function setup(responses: Suggestion[]) {
  let i = 0;
  mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === "suggest_game") return responses[Math.min(i++, responses.length - 1)];
    if (cmd === "get_cover") return null;
    throw new Error("unexpected " + cmd);
  });
}

describe("SuggestionDialog", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("renders the suggested game and requests another excluding the previous", async () => {
    setup([
      { game: game(1, "Juego Uno"), eligibleCount: 10, totalCount: 20 },
      { game: game(2, "Juego Dos"), eligibleCount: 10, totalCount: 20 },
    ]);
    render(<SuggestionDialog onClose={vi.fn()} onShowInList={vi.fn()} />);
    expect(await screen.findByText("Juego Uno")).toBeTruthy();
    expect(screen.getByText("Elegido entre 10 juegos no terminados de 20")).toBeTruthy();
    expect(screen.getByText("La sugerencia no modifica tu biblioteca.")).toBeTruthy();
    expect(screen.getByRole("dialog").getAttribute("aria-modal")).toBe("true");

    fireEvent.click(screen.getByRole("button", { name: "Otra sugerencia" }));
    expect(await screen.findByText("Juego Dos")).toBeTruthy();
    const calls = mockInvoke.mock.calls.filter((c) => c[0] === "suggest_game");
    expect(calls[0][1]).toEqual({ req: { excludeId: null } });
    expect(calls[1][1]).toEqual({ req: { excludeId: 1 } });
  });

  it("shows the empty message when game is null", async () => {
    setup([{ game: null, eligibleCount: 0, totalCount: 5 }]);
    render(<SuggestionDialog onClose={vi.fn()} onShowInList={vi.fn()} />);
    expect(
      await screen.findByText("No hay juegos pendientes: todos los juegos están terminados o la biblioteca está vacía."),
    ).toBeTruthy();
  });

  it("closes on Escape, backdrop click, and shows in list via callback", async () => {
    setup([{ game: game(9, "Nueve"), eligibleCount: 1, totalCount: 1 }]);
    const onClose = vi.fn();
    const onShow = vi.fn();
    const { container } = render(<SuggestionDialog onClose={onClose} onShowInList={onShow} />);
    await screen.findByText("Nueve");
    fireEvent.click(screen.getByRole("button", { name: "Ver en la lista" }));
    expect(onShow).toHaveBeenCalledWith(expect.objectContaining({ id: 9, codigo: "RJ9" }));
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
    fireEvent.mouseDown(container.querySelector(".backdrop")!);
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it("only ever invokes read commands", async () => {
    setup([{ game: game(1, "Uno"), eligibleCount: 1, totalCount: 1 }]);
    render(<SuggestionDialog onClose={vi.fn()} onShowInList={vi.fn()} />);
    await screen.findByText("Uno");
    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());
    const allowed = ["suggest_game", "get_cover", "get_game"];
    for (const c of mockInvoke.mock.calls) expect(allowed).toContain(c[0]);
  });
});
