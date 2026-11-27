import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import App from "../App";
import type { LibraryInfo } from "../api/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mockInvoke = vi.mocked(invoke);

const info = (total: number): LibraryInfo => ({
  dbFileName: "x.db", readOnly: true, total, ftsAvailable: true, ftsInSync: true,
  estados: [{ value: "Terminado", count: 1 }, { value: "Empezado", count: 2 }],
  idiomas: [{ value: "Español", count: 3 }], tags: [{ value: "fantasía", count: 4 }], sqliteVersion: "3",
});

let infoTotal = 3;
let dataChanged = false;
const searchCalls = () => mockInvoke.mock.calls.filter((c) => c[0] === "search_games").map((c) => (c[1] as { req: Record<string, unknown> }).req);

beforeEach(() => {
  localStorage.clear();
  localStorage.setItem("rpglib.welcome", "off");
  infoTotal = 3;
  dataChanged = false;
  mockInvoke.mockReset();
  mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === "get_library_info") return info(infoTotal);
    if (cmd === "search_games") return { items: [], total: 0, offset: 0, limit: 100, elapsedMs: 1, strategy: "none" };
    if (cmd === "check_data_version") return { version: 1, changed: dataChanged };
    throw new Error("unexpected " + cmd);
  });
});

describe("App", () => {
  it("sidebar filter click drives the search params and toggles off", async () => {
    render(<App />);
    const btn = await screen.findByRole("button", { name: /^Terminado/ });
    expect(btn.getAttribute("aria-pressed")).toBe("false");
    fireEvent.click(btn);
    await waitFor(() => expect(searchCalls().some((r) => r.estado === "Terminado")).toBe(true));
    expect(screen.getByRole("button", { name: /^Terminado/ }).getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: /^fantasía/ }));
    await waitFor(() => expect(searchCalls().some((r) => r.estado === "Terminado" && r.tag === "fantasía")).toBe(true));
    fireEvent.click(screen.getByRole("button", { name: /^Todos/ }));
    await waitFor(() => expect(searchCalls().at(-1)).toMatchObject({ estado: null, tag: null }));
  });

  it("view toggle persists and Ctrl+2 switches to gallery", async () => {
    render(<App />);
    await screen.findByRole("button", { name: /^Terminado/ });
    fireEvent.keyDown(window, { key: "2", ctrlKey: true });
    expect(screen.getByRole("button", { name: "Galería" }).getAttribute("aria-pressed")).toBe("true");
    expect(localStorage.getItem("rpglib.view")).toBe("gallery");
    expect(screen.getByRole("listbox", { name: "Juegos (galería)" })).toBeTruthy();
  });

  it("Ctrl+K opens the palette; a command runs", async () => {
    render(<App />);
    await screen.findByRole("button", { name: /^Terminado/ });
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    const input = screen.getByLabelText("Escribe un comando");
    fireEvent.change(input, { target: { value: "tema noche" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(document.documentElement.getAttribute("data-theme")).toBe("night");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("reloads info and search and shows a toast when the data version changed", async () => {
    render(<App />);
    await screen.findByText("3 juegos");
    const before = searchCalls().length;
    infoTotal = 4;
    dataChanged = true;
    window.dispatchEvent(new Event("focus"));
    expect(await screen.findByText("Biblioteca actualizada")).toBeTruthy();
    await screen.findByText("4 juegos");
    await waitFor(() => expect(searchCalls().length).toBeGreaterThan(before));
  });
});
