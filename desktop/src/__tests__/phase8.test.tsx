import { describe, it, expect, vi, beforeEach } from "vitest";
import { StrictMode } from "react";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import App from "../App";
import GameDetail from "../components/GameDetail";
import DashboardDialog from "../components/DashboardDialog";
import ReviewDialog from "../components/ReviewDialog";
import ShortcutsDialog from "../components/ShortcutsDialog";
import { SHORTCUTS, formatShortcut, matchShortcut } from "../lib/shortcuts";
import type { Dashboard, GameDetail as Detail, Issue, LibraryInfo, Vocabulary } from "../api/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mockInvoke = vi.mocked(invoke);

const vocab: Vocabulary = { estados: ["Sin empezar", "Empezado", "Terminado"], idiomas: ["Japonés", "Español"], paginas: ["A"], tipos: ["RPG", "Otro"], puntuaciones: [0] };
const game = (id = 7, over: Partial<Detail> = {}): Detail => ({
  id, titulo: `Juego ${id}`, codigo: `RJ${id}`, estado: "Empezado", idioma: "Español", tipo: "RPG", pagina: "A",
  puntuacion: 3, hasCover: false, descripcion: "", ruta: "C:/x", tags: ["fantasía"], revision: `rev${id}`, ...over,
});
const info = (tags = [{ value: "fantasía", count: 1 }]): LibraryInfo => ({
  dbFileName: "x.db", readOnly: false, total: 2, ftsAvailable: true, ftsInSync: true,
  estados: [{ value: "Terminado", count: 1 }], idiomas: [], tags, sqliteVersion: "3",
});
const dash: Dashboard = {
  total: 200,
  estados: [{ value: "Terminado", count: 120 }, { value: "Empezado", count: 60 }, { value: "", count: 20 }],
  idiomas: [{ value: "Español", count: 150 }, { value: "Japonés", count: 50 }],
  tipos: [{ value: "RPG", count: 200 }],
  paginas: [{ value: "A", count: 100 }],
  puntuaciones: Array.from({ length: 11 }, (_, i) => ({ value: (i / 2).toFixed(1), count: i === 10 ? 200 : 0 })),
  estadoIdioma: [
    { estado: "Terminado", idioma: "Español", count: 100 },
    { estado: "Terminado", idioma: "Japonés", count: 20 },
    { estado: "Empezado", idioma: "Español", count: 50 },
  ],
  cobertura: { conPortada: 50, conDescripcion: 100, conTags: 150, conRuta: 200 },
  topTags: [{ value: "fantasía", count: 30 }],
  elapsedMs: 12,
};
const calls = (cmd: string) => mockInvoke.mock.calls.filter((c) => c[0] === cmd).map((c) => c[1] as Record<string, unknown>);
const err = (kind: string, message = "m") => Promise.reject({ kind, message });

let handlers: Record<string, (a: Record<string, unknown>) => unknown>;
beforeEach(() => {
  localStorage.clear();
  localStorage.setItem("rpglib.welcome", "off");
  handlers = {};
  mockInvoke.mockReset();
  mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
    const h = handlers[cmd];
    if (h) return h((args ?? {}) as Record<string, unknown>);
    switch (cmd) {
      case "get_game": return game((args as { id: number }).id);
      case "get_cover": return null;
      case "get_library_info": return info();
      case "get_vocabulary": return vocab;
      case "get_session_info": return { dbFileName: "x.db", dbDir: "C:/d", readOnly: false, lastBackup: null };
      case "search_games": return { items: [], total: 0, offset: 0, limit: 100, elapsedMs: 1, strategy: "none" };
      case "check_data_version": return { version: 1, changed: false };
      case "get_dashboard": return dash;
      case "update_game": return game((args as { id: number }).id, { revision: "new" });
      default: throw new Error("unexpected " + cmd);
    }
  });
});

async function paletteRun(label: string) {
  fireEvent.keyDown(window, { key: "k", ctrlKey: true });
  const input = await screen.findByLabelText("Escribe un comando");
  fireEvent.change(input, { target: { value: label } });
  fireEvent.keyDown(input, { key: "Enter" });
}

describe("Dashboard", () => {
  it("renders total, coverage percentages, bars as tables and empty values", async () => {
    render(<DashboardDialog onClose={vi.fn()} onFilter={vi.fn()} />);
    await screen.findByRole("list", { name: "Cobertura de datos" });
    expect(screen.getByRole("status").textContent).toContain("200");
    const tiles = within(screen.getByRole("list", { name: "Cobertura de datos" }));
    expect(tiles.getByText("25 %")).toBeTruthy(); // portada 50/200
    expect(tiles.getByText("50 %")).toBeTruthy();
    expect(tiles.getByText("75 %")).toBeTruthy();
    expect(tiles.getByText("100 %")).toBeTruthy();
    expect(screen.getByText(/calculado en 12,0 ms/)).toBeTruthy();
    const estados = within(screen.getByRole("table", { name: "Estados" }));
    const row = estados.getByRole("row", { name: /Terminado/ });
    expect(row.textContent).toContain("120");
    expect(row.textContent).toContain("60 %");
    expect(estados.getByText("(vacío)")).toBeTruthy();
    expect(estados.queryByRole("button", { name: /vacío/ })).toBeNull();
    expect(within(screen.getByRole("table", { name: "Distribución de puntuaciones" })).getAllByRole("row")).toHaveLength(12); // header + 11
    const pivot = within(screen.getByRole("table", { name: "Estado por idioma" }));
    expect(pivot.getByRole("row", { name: /Terminado/ }).textContent).toContain("100");
  });

  it("an older load never overwrites a newer one; the pivot scroller is keyboard reachable", async () => {
    let n = 0;
    let late: (d: Dashboard) => void = () => {};
    handlers.get_dashboard = () => (++n === 1 ? new Promise<Dashboard>((r) => (late = r)) : { ...dash, total: 200 });
    render(
      <StrictMode>
        <DashboardDialog onClose={vi.fn()} onFilter={vi.fn()} />
      </StrictMode>,
    );
    await screen.findByRole("list", { name: "Cobertura de datos" });
    await act(async () => late({ ...dash, total: 1 }));
    expect(screen.getByRole("status").textContent).toContain("200");
    const region = screen.getByRole("region", { name: /desplazable/ });
    expect(region.getAttribute("tabindex")).toBe("0");
  });

  it("clicking an estado bar filters the main view and closes the dashboard", async () => {
    render(<App />);
    await paletteRun("Estadísticas");
    const dlg = await screen.findByRole("dialog", { name: "Estadísticas de la biblioteca" });
    fireEvent.click(within(within(dlg).getByRole("table", { name: "Estados" })).getByRole("button", { name: "Terminado" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Estadísticas de la biblioteca" })).toBeNull());
    await waitFor(() => expect(calls("search_games").some((c) => (c.req as { estado?: string }).estado === "Terminado")).toBe(true));
  });

  it("clicking a tag bar sets the tag filter", async () => {
    render(<App />);
    await paletteRun("Estadísticas");
    const dlg = await screen.findByRole("dialog", { name: "Estadísticas de la biblioteca" });
    fireEvent.click(within(within(dlg).getByRole("table", { name: "Etiquetas más usadas" })).getByRole("button", { name: "fantasía" }));
    await waitFor(() => expect(calls("search_games").some((c) => (c.req as { tag?: string }).tag === "fantasía")).toBe(true));
  });
});

describe("Favoritos", () => {
  const renderDetail = (props: Partial<React.ComponentProps<typeof GameDetail>> = {}) =>
    render(<GameDetail id={7} vocab={vocab} tagSuggestions={[]} {...props} />);

  it("adds then removes the favorite with set_favorite and expectedRevision, never update_game", async () => {
    let current = game(7, { tags: ["fantasía", "ñu"], descripcion: "d" });
    handlers.get_game = () => current;
    handlers.set_favorite = (a) => {
      const tags = a.favorite ? [...current.tags, "Favorito"] : current.tags.filter((t) => t.toLowerCase() !== "favorito");
      current = { ...current, tags, revision: current.revision + "+" };
      return current;
    };
    const onFavoriteChanged = vi.fn();
    renderDetail({ onFavoriteChanged });
    const btn = await screen.findByRole("button", { name: "Favorito" });
    expect(btn.getAttribute("aria-pressed")).toBe("false");
    fireEvent.click(btn);
    await waitFor(() => expect(calls("set_favorite")).toHaveLength(1));
    expect(calls("set_favorite")[0]).toEqual({ id: 7, favorite: true, opts: { expectedRevision: "rev7" } });
    await waitFor(() => expect(screen.getByRole("button", { name: "Favorito" }).getAttribute("aria-pressed")).toBe("true"));
    fireEvent.click(screen.getByRole("button", { name: "Favorito" }));
    await waitFor(() => expect(calls("set_favorite")).toHaveLength(2));
    expect(calls("set_favorite")[1]).toEqual({ id: 7, favorite: false, opts: { expectedRevision: "rev7+" } });
    expect(calls("update_game")).toHaveLength(0);
    expect(onFavoriteChanged).toHaveBeenLastCalledWith(expect.anything(), false);
  });

  it("matches the tag case-insensitively when removing", async () => {
    const current = game(7, { tags: ["favorito", "x"] });
    handlers.get_game = () => current;
    handlers.set_favorite = () => ({ ...current, tags: ["x"] });
    renderDetail();
    const btn = await screen.findByRole("button", { name: "Favorito" });
    expect(btn.getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(btn);
    await waitFor(() => expect(calls("set_favorite")).toHaveLength(1));
    expect(calls("set_favorite")[0]).toMatchObject({ favorite: false });
    expect(calls("update_game")).toHaveLength(0);
  });

  it("disables Editar while the favorite write is in flight", async () => {
    let release: (g: Detail) => void = () => {};
    handlers.set_favorite = () => new Promise<Detail>((r) => (release = r));
    renderDetail();
    fireEvent.click(await screen.findByRole("button", { name: "Favorito" }));
    await waitFor(() => expect((screen.getByRole("button", { name: "Editar" }) as HTMLButtonElement).disabled).toBe(true));
    release(game(7, { tags: ["Favorito"], revision: "n" }));
    await waitFor(() => expect((screen.getByRole("button", { name: "Editar" }) as HTMLButtonElement).disabled).toBe(false));
  });

  it("is disabled in read-only mode", async () => {
    renderDetail({ readOnly: true });
    expect((await screen.findByRole("button", { name: "Favorito" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("reloads and tells the user on a conflict", async () => {
    handlers.set_favorite = () => err("Conflict");
    const onToast = vi.fn();
    renderDetail({ onToast });
    fireEvent.click(await screen.findByRole("button", { name: "Favorito" }));
    await waitFor(() => expect(onToast).toHaveBeenCalledWith(expect.stringMatching(/cambió en otra aplicación/)));
    expect(calls("get_game").length).toBeGreaterThanOrEqual(2); // initial + reload
  });

  it("palette 'Mostrar favoritos' uses the stored spelling; toast when none", async () => {
    handlers.get_library_info = () => info([{ value: "favorito", count: 2 }]);
    const { unmount } = render(<App />);
    await screen.findByText("edición");
    await paletteRun("Mostrar favoritos");
    await waitFor(() => expect(calls("search_games").some((c) => (c.req as { tag?: string }).tag === "favorito")).toBe(true));
    unmount();
    handlers.get_library_info = () => info();
    render(<App />);
    await screen.findByText("edición");
    await paletteRun("Mostrar favoritos");
    expect(await screen.findByText("Aún no hay favoritos")).toBeTruthy();
  });
});

describe("Revisión por tandas", () => {
  const issues = (n: number): Issue[] => Array.from({ length: n }, (_, i) => ({ id: i + 1, titulo: `Juego ${i + 1}`, codigo: `C${i + 1}`, estado: "Empezado", ruta: "", detail: null }));
  const setup = (readOnly = false, total = 5, criterion?: React.ComponentProps<typeof ReviewDialog>["initialCriterion"]) => {
    handlers.maintenance_list = (a) => {
      const all = issues(total);
      const off = a.req ? ((a.req as { offset: number }).offset) : 0;
      return { items: all.slice(off, off + 500), total: all.length, offset: off, limit: 500 };
    };
    const onChanged = vi.fn();
    const onClose = vi.fn();
    render(<ReviewDialog vocab={vocab} readOnly={readOnly} tagSuggestions={[]} onClose={onClose} onChanged={onChanged} initialCriterion={criterion} />);
    return { onChanged, onClose };
  };
  const order = () => within(screen.getByRole("navigation", { name: "Juegos de la tanda" })).getAllByRole("button").map((b) => b.textContent ?? "");

  it("loads a random batch of the chosen size (default 20) from the detector", async () => {
    setup(false, 30);
    await screen.findByRole("form", { name: "Editar juego" });
    expect(calls("maintenance_list")[0].req).toMatchObject({ check: "sin_descripcion", offset: 0, limit: 500 });
    expect(order()).toHaveLength(20);
    expect(screen.getByRole("status", { name: "" }).textContent).toBeTruthy();
    expect(screen.getByText(/1 de 20 · 30 pendientes en total/)).toBeTruthy();
  });

  it("save & next, previous, skip and new batch", async () => {
    const { onChanged } = setup(false, 5);
    await screen.findByText(/1 de 5/);
    const first = order()[0];
    await screen.findByRole("form", { name: "Editar juego" });
    fireEvent.change(screen.getByLabelText("Descripción"), { target: { value: "Nueva desc" } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar y siguiente" }));
    await screen.findByText(/2 de 5/);
    const upd = calls("update_game");
    expect(upd).toHaveLength(1);
    expect(upd[0].input).toMatchObject({ descripcion: "Nueva desc" });
    expect((upd[0].opts as { expectedRevision: string }).expectedRevision).toMatch(/^rev/);
    expect(onChanged).toHaveBeenCalledTimes(1);
    expect(within(screen.getByRole("navigation", { name: "Juegos de la tanda" })).getAllByLabelText("guardado")).toHaveLength(1);
    expect(order()[0]).toBe(first.includes("✓") ? first : first + "✓");
    // skip does not save
    await screen.findByRole("form", { name: "Editar juego" });
    fireEvent.click(screen.getByRole("button", { name: "Omitir" }));
    await screen.findByText(/3 de 5/);
    expect(calls("update_game")).toHaveLength(1);
    // previous
    fireEvent.click(screen.getByRole("button", { name: "Anterior" }));
    await screen.findByText(/2 de 5/);
    // new batch
    const before = calls("maintenance_list").length;
    fireEvent.click(screen.getByRole("button", { name: "Nueva tanda" }));
    await waitFor(() => expect(calls("maintenance_list").length).toBeGreaterThan(before));
    await screen.findByText(/1 de 5/);
    expect(within(screen.getByRole("navigation", { name: "Juegos de la tanda" })).queryAllByLabelText("guardado")).toHaveLength(0);
  });

  it("Ctrl+Enter saves and goes next; finishing shows the end state", async () => {
    setup(false, 1);
    const desc = await screen.findByLabelText("Descripción");
    fireEvent.change(desc, { target: { value: "x" } });
    fireEvent.keyDown(desc, { key: "Enter", ctrlKey: true });
    await screen.findByText(/Tanda terminada: 1 guardados de 1/);
  });

  it("asks before discarding unsaved edits when leaving the game", async () => {
    setup(false, 3);
    await screen.findByText(/1 de 3/);
    await screen.findByRole("form", { name: "Editar juego" });
    fireEvent.click(screen.getByRole("button", { name: "Omitir" }));
    await screen.findByText(/2 de 3/);
    fireEvent.change(await screen.findByLabelText("Descripción"), { target: { value: "sin guardar" } });
    fireEvent.click(screen.getByRole("button", { name: "Anterior" }));
    const dlg = await screen.findByRole("alertdialog");
    fireEvent.click(within(dlg).getByRole("button", { name: "Cancelar" }));
    expect(screen.getByText(/2 de 3/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Anterior" }));
    fireEvent.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Descartar" }));
    await screen.findByText(/1 de 3/);
    expect(calls("update_game")).toHaveLength(0);
  });

  it("opens preset to initialCriterion", async () => {
    setup(false, 3, "sin_portada");
    await screen.findByText(/1 de 3/);
    expect(calls("maintenance_list")[0].req).toMatchObject({ check: "sin_portada" });
    expect((screen.getByLabelText("Criterio") as HTMLSelectElement).value).toBe("sin_portada");
  });

  it("pages maintenance_list until every issue is collected", async () => {
    setup(false, 1100);
    await screen.findByText(/1 de 20 · 1100 pendientes/);
    expect(calls("maintenance_list").map((c) => (c.req as { offset: number }).offset)).toEqual([0, 500, 1000]);
  });

  it("saves with expectedRevision and the full current input", async () => {
    setup(false, 2);
    await screen.findByRole("form", { name: "Editar juego" });
    fireEvent.change(screen.getByLabelText("Descripción"), { target: { value: "Una historia" } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar y siguiente" }));
    await screen.findByText(/2 de 2/);
    const c = calls("update_game")[0] as { id: number; input: Record<string, unknown>; opts: Record<string, unknown> };
    expect(c.opts).toEqual({ allowDuplicateCode: false, expectedRevision: `rev${c.id}` });
    expect(c.input).toMatchObject({ titulo: `Juego ${c.id}`, descripcion: "Una historia", estado: "Empezado", idioma: "Español", codigo: `RJ${c.id}`, tags: ["fantasía"] });
  });

  it("on Conflict shows the banner; Recargar asks before discarding edits and reloads", async () => {
    setup(false, 2);
    await screen.findByRole("form", { name: "Editar juego" });
    handlers.update_game = () => err("Conflict", "cambió");
    fireEvent.change(screen.getByLabelText("Descripción"), { target: { value: "mi texto" } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar y siguiente" }));
    await screen.findByText("Este juego cambió en otra aplicación.");
    const gets = calls("get_game").length;
    fireEvent.click(screen.getByRole("button", { name: "Recargar" }));
    const dlg = await screen.findByRole("alertdialog");
    expect(calls("get_game")).toHaveLength(gets); // nothing discarded yet
    fireEvent.click(within(dlg).getByRole("button", { name: "Cancelar" }));
    expect((screen.getByLabelText("Descripción") as HTMLTextAreaElement).value).toBe("mi texto");
    fireEvent.click(screen.getByRole("button", { name: "Recargar" }));
    fireEvent.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Descartar" }));
    await waitFor(() => expect(calls("get_game").length).toBe(gets + 1));
    await waitFor(() => expect((screen.getByLabelText("Descripción") as HTMLTextAreaElement).value).toBe(""));
    expect(screen.queryByText("Este juego cambió en otra aplicación.")).toBeNull();
  });

  it("Recargar needs no confirmation when the form is clean", async () => {
    setup(false, 2);
    await screen.findByRole("form", { name: "Editar juego" });
    handlers.update_game = () => err("Conflict");
    fireEvent.click(screen.getByRole("button", { name: "Guardar y siguiente" }));
    await screen.findByText("Este juego cambió en otra aplicación.");
    fireEvent.click(screen.getByRole("button", { name: "Recargar" }));
    await waitFor(() => expect(screen.queryByText("Este juego cambió en otra aplicación.")).toBeNull());
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("global shortcuts (Ctrl+N, /) do nothing while a dialog is open", async () => {
    const { container } = render(<App />);
    await screen.findByText("edición");
    const search = container.querySelector<HTMLInputElement>('[role="search"] input')!;
    handlers.maintenance_list = () => ({ items: [], total: 0, offset: 0, limit: 500 });
    await paletteRun("Revisión por tandas");
    await screen.findByRole("dialog", { name: "Revisión por tandas" });
    fireEvent.keyDown(window, { key: "n", ctrlKey: true });
    fireEvent.keyDown(window, { key: "/" });
    fireEvent.keyDown(window, { key: "?", shiftKey: true });
    expect(screen.queryByRole("form", { name: "Nuevo juego" })).toBeNull();
    expect(screen.queryByRole("dialog", { name: "Atajos de teclado" })).toBeNull();
    expect(document.activeElement).not.toBe(search);
    fireEvent.click(screen.getByRole("button", { name: "Cerrar" }));
    expect(screen.queryByRole("dialog", { name: "Revisión por tandas" })).toBeNull();
    fireEvent.keyDown(window, { key: "/" });
    expect(document.activeElement).toBe(search);
  });

  it("is view-only in read-only mode", async () => {
    setup(true, 3);
    await screen.findByText(/1 de 3/);
    await screen.findByRole("button", { name: "Siguiente" });
    expect(screen.queryByRole("form", { name: "Editar juego" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Guardar y siguiente" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Siguiente" }));
    await screen.findByText(/2 de 3/);
  });
});

describe("Atajos", () => {
  it("every SHORTCUTS entry appears in the help dialog", () => {
    render(<ShortcutsDialog onClose={vi.fn()} />);
    const table = within(screen.getByRole("table", { name: "Atajos de teclado" }));
    for (const s of SHORTCUTS) {
      const row = table.getByRole("row", { name: new RegExp(s.description.replace(/[.*+?^${}()|[\]\;]/g, "\\$&")) });
      expect(row.textContent).toContain(formatShortcut(s));
    }
  });

  it("matchShortcut keeps the previous behaviour", () => {
    const e = (key: string, o: Partial<KeyboardEvent> = {}) => ({ key, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...o });
    expect(matchShortcut(e("k", { ctrlKey: true }))?.id).toBe("palette");
    expect(matchShortcut(e("p", { ctrlKey: true, shiftKey: true }))?.id).toBe("palette");
    expect(matchShortcut(e("p", { ctrlKey: true }))).toBeNull();
    expect(matchShortcut(e("n", { ctrlKey: true }))?.id).toBe("new-game");
    expect(matchShortcut(e("n", { ctrlKey: true, shiftKey: true }))).toBeNull();
    expect(matchShortcut(e("1", { metaKey: true }))?.id).toBe("view-list");
    expect(matchShortcut(e("2", { ctrlKey: true }))?.id).toBe("view-gallery");
    expect(matchShortcut(e("k", { ctrlKey: true, altKey: true }))).toBeNull();
    expect(matchShortcut(e("/"))?.id).toBe("focus-search");
    expect(matchShortcut(e("/", { ctrlKey: true }))).toBeNull();
    expect(matchShortcut(e("F1"))?.id).toBe("help");
  });

  it("'?' and F1 open the help from App; '?' is ignored while typing", async () => {
    const { container } = render(<App />);
    await screen.findByText("edición");
    const search = container.querySelector<HTMLInputElement>('[role="search"] input')!;
    search.focus();
    fireEvent.keyDown(search, { key: "?", shiftKey: true });
    expect(screen.queryByRole("dialog", { name: "Atajos de teclado" })).toBeNull();
    fireEvent.keyDown(window, { key: "?", shiftKey: true });
    expect(await screen.findByRole("dialog", { name: "Atajos de teclado" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cerrar" }));
    expect(screen.queryByRole("dialog", { name: "Atajos de teclado" })).toBeNull();
    fireEvent.keyDown(search, { key: "F1" });
    expect(await screen.findByRole("dialog", { name: "Atajos de teclado" })).toBeTruthy();
  });

  it("the palette action opens the help and the Herramientas menu opens the tools", async () => {
    render(<App />);
    await screen.findByText("edición");
    await paletteRun("Atajos de teclado");
    expect(await screen.findByRole("dialog", { name: "Atajos de teclado" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cerrar" }));
    fireEvent.click(screen.getByRole("button", { name: /Herramientas/ }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Estadísticas" }));
    expect(await screen.findByRole("dialog", { name: "Estadísticas de la biblioteca" })).toBeTruthy();
  });
});
