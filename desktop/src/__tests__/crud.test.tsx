import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import App from "../App";
import GameDetail from "../components/GameDetail";
import TagEditor from "../components/TagEditor";
import SettingsDialog from "../components/SettingsDialog";
import { addTags, toInput, validate, defaultValues } from "../lib/gameInput";
import type { GameDetail as Detail, LibraryInfo, SessionInfo, Vocabulary } from "../api/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mockInvoke = vi.mocked(invoke);

const vocab: Vocabulary = {
  estados: ["Sin empezar", "Empezado", "Terminado"],
  idiomas: ["Japonés", "Español"],
  paginas: ["A", "B"],
  tipos: ["RPG", "Otro"],
  puntuaciones: [0, 0.5, 1],
};
const game = (over: Partial<Detail> = {}): Detail => ({
  id: 7, titulo: "Mi juego", codigo: "RJ001", estado: "Empezado", idioma: "Español", tipo: "RPG", pagina: "A",
  puntuacion: 3, hasCover: false, descripcion: "desc", ruta: "C:/x", tags: ["fantasía"], revision: "rev1", ...over,
});
const info = (readOnly = false): LibraryInfo => ({
  dbFileName: "x.db", readOnly, total: 2, ftsAvailable: true, ftsInSync: true,
  estados: [], idiomas: [], tags: [{ value: "fantasía", count: 1 }], sqliteVersion: "3",
});
const calls = (cmd: string) => mockInvoke.mock.calls.filter((c) => c[0] === cmd).map((c) => c[1] as Record<string, unknown>);
const err = (kind: string, message = "m") => Promise.reject({ kind, message });

let handlers: Record<string, (args: Record<string, unknown>) => unknown>;
beforeEach(() => {
  localStorage.clear();
  handlers = {};
  mockInvoke.mockReset();
  mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
    const h = handlers[cmd];
    if (h) return h((args ?? {}) as Record<string, unknown>);
    if (cmd === "get_game") return game();
    if (cmd === "get_cover") return null;
    if (cmd === "get_library_info") return info();
    if (cmd === "get_vocabulary") return vocab;
    if (cmd === "get_session_info") return { dbFileName: "x.db", dbDir: "C:/d", readOnly: false, lastBackup: null } satisfies SessionInfo;
    if (cmd === "search_games") return { items: [{ ...game(), hasCover: false }, { ...game({ id: 8, titulo: "Otro" }) }], total: 2, offset: 0, limit: 100, elapsedMs: 1, strategy: "none" };
    if (cmd === "check_data_version") return { version: 1, changed: false };
    if (cmd === "get_migration_status") return { current: 0, latest: 1, pending: [{ version: 1, description: "Índice por título" }] };
    throw new Error("unexpected " + cmd);
  });
});

function renderDetail(props: Partial<React.ComponentProps<typeof GameDetail>> = {}) {
  return render(<GameDetail id={7} vocab={vocab} tagSuggestions={["fantasía"]} {...props} />);
}
async function openEdit() {
  fireEvent.click(await screen.findByRole("button", { name: "Editar" }));
  return screen.findByLabelText(/^Título/);
}

describe("gameInput", () => {
  it("validates like the contract", () => {
    const v = defaultValues(vocab);
    expect(validate(v).titulo).toMatch(/obligatorio/);
    expect(validate({ ...v, titulo: "x".repeat(301) }).titulo).toBeTruthy();
    expect(validate({ ...v, titulo: "ok", puntuacion: 2.3 }).puntuacion).toBeTruthy();
    expect(validate({ ...v, titulo: "ok", tags: Array.from({ length: 51 }, (_, i) => `t${i}`) }).tags).toBeTruthy();
    expect(validate({ ...v, titulo: "ok" })).toEqual({});
    expect(defaultValues(vocab)).toMatchObject({ estado: "Sin empezar", idioma: "Japonés", tipo: "RPG", pagina: "", puntuacion: 0 });
  });
  it("addTags trims and dedupes case-insensitively", () => {
    expect(addTags(["Fantasía"], " fantasía, nuevo ,, NUEVO")).toEqual(["Fantasía", "nuevo"]);
    expect(toInput({ ...defaultValues(vocab), titulo: "  t ", tagDraft: "x" }).tags).toEqual(["x"]);
  });
});

describe("TagEditor", () => {
  function Host({ initial = ["a"] }: { initial?: string[] }) {
    const [state, setState] = (require_react().useState as typeof import("react").useState)({ tags: initial, draft: "" });
    return <TagEditor tags={state.tags} draft={state.draft} suggestions={["zz"]} onChange={(tags, draft) => setState({ tags, draft })} />;
  }
  const require_react = () => React;
  it("adds with Enter/comma, dedupes, removes with Backspace and ×", () => {
    render(<Host />);
    const input = screen.getByLabelText("Añadir etiqueta");
    fireEvent.change(input, { target: { value: "B" } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.change(input, { target: { value: "b" } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.change(input, { target: { value: "c," } });
    expect(screen.getAllByRole("listitem").map((l) => l.textContent?.replace(/\s*×$/, ""))).toEqual(["a", "B", "c"]);
    fireEvent.keyDown(input, { key: "Backspace" });
    expect(screen.getAllByRole("listitem")).toHaveLength(2);
    fireEvent.click(screen.getByRole("button", { name: "Quitar etiqueta a" }));
    expect(screen.getAllByRole("listitem").map((l) => l.textContent?.replace(/\s*×$/, ""))).toEqual(["B"]);
  });
});
import React from "react";

describe("GameDetail editing", () => {
  it("shows inline validation with aria-invalid and does not call the backend", async () => {
    renderDetail();
    const title = await openEdit();
    fireEvent.change(title, { target: { value: "   " } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
    expect(title.getAttribute("aria-invalid")).toBe("true");
    expect(screen.getByRole("alert").textContent).toMatch(/obligatorio/);
    expect(title.getAttribute("aria-describedby")).toBe(screen.getByRole("alert").id);
    expect(calls("update_game")).toHaveLength(0);
  });

  it("sends the exact normalized payload with expectedRevision", async () => {
    const onSaved = vi.fn();
    handlers.update_game = () => game({ titulo: "Nuevo", revision: "rev2" });
    renderDetail({ onSaved });
    const title = await openEdit();
    fireEvent.change(title, { target: { value: "  Nuevo  " } });
    fireEvent.change(screen.getByLabelText("Código"), { target: { value: " RJ9 " } });
    fireEvent.change(screen.getByLabelText("Puntuación"), { target: { value: "4.5" } });
    const tags = screen.getByLabelText("Añadir etiqueta");
    fireEvent.change(tags, { target: { value: "nueva" } });
    fireEvent.keyDown(screen.getByRole("form"), { key: "s", ctrlKey: true });
    await waitFor(() => expect(calls("update_game")).toHaveLength(1));
    expect(calls("update_game")[0]).toEqual({
      id: 7,
      input: { titulo: "Nuevo", descripcion: "desc", estado: "Empezado", idioma: "Español", ruta: "C:/x", codigo: "RJ9", pagina: "A", tipo: "RPG", puntuacion: 4.5, tags: ["fantasía", "nueva"] },
      opts: { allowDuplicateCode: false, expectedRevision: "rev1" },
    });
    await waitFor(() => expect(onSaved).toHaveBeenCalled());
    expect(screen.queryByRole("form")).toBeNull();
  });

  it("preserves an unknown estado in the select and in the payload", async () => {
    handlers.get_game = () => game({ estado: "Abandonado raro" });
    handlers.update_game = (a) => ({ ...game(), ...(a.input as object) });
    renderDetail();
    await openEdit();
    const sel = screen.getByLabelText(/^Estado/) as HTMLSelectElement;
    expect(sel.value).toBe("Abandonado raro");
    expect(within(sel).getAllByRole("option").map((o) => o.textContent)).toContain("Abandonado raro");
    fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
    await waitFor(() => expect(calls("update_game")).toHaveLength(1));
    expect((calls("update_game")[0].input as { estado: string }).estado).toBe("Abandonado raro");
  });

  it("DuplicateCode asks for confirmation and retries with allowDuplicateCode", async () => {
    let n = 0;
    handlers.update_game = () => (n++ === 0 ? err("DuplicateCode", "El código RJ001 ya lo usa «Otro»") : game({ revision: "r2" }));
    renderDetail();
    await openEdit();
    fireEvent.change(screen.getByLabelText("Código"), { target: { value: "RJ001x" } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
    const dlg = await screen.findByRole("alertdialog");
    expect(dlg.textContent).toContain("ya lo usa «Otro»");
    expect(dlg.textContent).toContain("¿Guardar de todos modos?");
    fireEvent.click(within(dlg).getByRole("button", { name: "Guardar de todos modos" }));
    await waitFor(() => expect(calls("update_game")).toHaveLength(2));
    expect((calls("update_game")[1].opts as { allowDuplicateCode: boolean }).allowDuplicateCode).toBe(true);
  });

  it("Conflict offers Recargar, which refetches and discards edits", async () => {
    handlers.update_game = () => err("Conflict");
    renderDetail();
    const title = await openEdit();
    fireEvent.change(title, { target: { value: "Editado" } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
    expect((await screen.findByRole("alert")).textContent).toContain("Este juego cambió en otra aplicación");
    handlers.get_game = () => game({ titulo: "Desde otra app", revision: "rev9" });
    const before = calls("get_game").length;
    fireEvent.click(screen.getByRole("button", { name: "Recargar" }));
    await waitFor(() => expect((screen.getByLabelText(/^Título/) as HTMLInputElement).value).toBe("Desde otra app"));
    expect(calls("get_game").length).toBeGreaterThan(before);
    // next save uses the fresh revision
    handlers.update_game = () => game();
    fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
    await waitFor(() => expect(calls("update_game")).toHaveLength(2));
    expect((calls("update_game")[1].opts as { expectedRevision: string }).expectedRevision).toBe("rev9");
  });

  it("Busy shows a toast message through onToast", async () => {
    handlers.update_game = () => err("Busy");
    const onToast = vi.fn();
    renderDetail({ onToast });
    await openEdit();
    fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
    await waitFor(() => expect(onToast).toHaveBeenCalledWith("La base está ocupada por otra aplicación; inténtalo de nuevo"));
  });

  it("read-only disables Editar and Eliminar", async () => {
    renderDetail({ readOnly: true });
    const btn = (await screen.findByRole("button", { name: "Editar" })) as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
    expect(btn.title).toMatch(/solo lectura/);
  });

  it("delete requires confirmation and sends expectedRevision", async () => {
    handlers.delete_game = () => null;
    const onDeleted = vi.fn();
    renderDetail({ onDeleted });
    fireEvent.click(await screen.findByRole("button", { name: "Eliminar" }));
    const dlg = await screen.findByRole("alertdialog");
    expect(dlg.textContent).toContain("¿Eliminar «Mi juego»?");
    expect(calls("delete_game")).toHaveLength(0);
    fireEvent.click(within(dlg).getByRole("button", { name: "Eliminar" }));
    await waitFor(() => expect(onDeleted).toHaveBeenCalledWith(7));
    expect(calls("delete_game")[0]).toEqual({ id: 7, opts: { expectedRevision: "rev1" } });
  });

  it("creating sends create_game with defaults and no expectedRevision", async () => {
    handlers.create_game = () => game({ id: 99 });
    const onSaved = vi.fn();
    render(<GameDetail id={null} creating vocab={vocab} onSaved={onSaved} />);
    fireEvent.change(screen.getByLabelText(/^Título/), { target: { value: "Nuevo" } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
    await waitFor(() => expect(onSaved).toHaveBeenCalledWith(expect.objectContaining({ id: 99 }), true));
    expect(calls("create_game")[0]).toEqual({
      input: { titulo: "Nuevo", descripcion: "", estado: "Sin empezar", idioma: "Japonés", ruta: "", codigo: "", pagina: "", tipo: "RPG", puntuacion: 0, tags: [] },
      opts: { allowDuplicateCode: false },
    });
  });
});

describe("App guard and read-only", () => {
  it("asks before discarding edits when starting another action", async () => {
    handlers.suggest_game = () => ({ game: game(), eligibleCount: 1, totalCount: 2 });
    render(<App />);
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    const input = await screen.findByLabelText("Escribe un comando");
    fireEvent.change(input, { target: { value: "sugerido" } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.click(await screen.findByRole("button", { name: "Ver en la lista" }));
    fireEvent.click(await screen.findByRole("button", { name: "Editar" }));
    fireEvent.change(await screen.findByLabelText(/^Título/), { target: { value: "Cambiado" } });
    fireEvent.click(screen.getByRole("button", { name: "Nuevo juego" }));
    const dlg = await screen.findByRole("alertdialog");
    expect(dlg.textContent).toMatch(/sin guardar/);
    fireEvent.click(within(dlg).getByRole("button", { name: "Cancelar" }));
    expect((screen.getByLabelText(/^Título/) as HTMLInputElement).value).toBe("Cambiado");
    fireEvent.click(screen.getByRole("button", { name: "Nuevo juego" }));
    fireEvent.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Descartar" }));
    const form = await screen.findByRole("form", { name: "Nuevo juego" });
    expect((within(form).getByLabelText(/^Título/) as HTMLInputElement).value).toBe("");
  });

  it("shows the edición badge and Nuevo juego opens an empty form (Ctrl+N)", async () => {
    render(<App />);
    expect(await screen.findByText("edición")).toBeTruthy();
    fireEvent.keyDown(window, { key: "n", ctrlKey: true });
    expect(await screen.findByRole("form", { name: "Nuevo juego" })).toBeTruthy();
  });

  it("read-only session disables Nuevo juego", async () => {
    handlers.get_session_info = () => ({ dbFileName: "x.db", dbDir: "C:/d", readOnly: true, lastBackup: null });
    render(<App />);
    expect(await screen.findByText("solo lectura")).toBeTruthy();
    expect((screen.getByRole("button", { name: "Nuevo juego" }) as HTMLButtonElement).disabled).toBe(true);
  });
});

describe("SettingsDialog", () => {
  const session: SessionInfo = { dbFileName: "x.db", dbDir: "C:/d", readOnly: false, lastBackup: null };
  const setup = () => {
    const p = { onClose: vi.fn(), onDbChanged: vi.fn(), onSessionChanged: vi.fn(), onMigrated: vi.fn() };
    render(<SettingsDialog session={session} {...p} />);
    return p;
  };

  it("set_db_path is called with the path; errors are shown verbatim", async () => {
    handlers.set_db_path = () => err("Config", "Faltan columnas: tags");
    const p = setup();
    fireEvent.change(screen.getByLabelText("Ruta de la base de datos"), { target: { value: "C:/otra.db" } });
    fireEvent.click(screen.getByRole("button", { name: "Abrir" }));
    expect((await screen.findByText("Faltan columnas: tags")).getAttribute("role")).toBe("alert");
    expect(calls("set_db_path")[0]).toEqual({ path: "C:/otra.db" });
    expect(p.onDbChanged).not.toHaveBeenCalled();
    handlers.set_db_path = () => info();
    fireEvent.click(screen.getByRole("button", { name: "Abrir" }));
    await waitFor(() => expect(p.onDbChanged).toHaveBeenCalled());
  });

  it("run_migrations requires confirmation and shows the result", async () => {
    handlers.run_migrations = () => ({ applied: [1], backupPath: "C:/d/backups/x.bak" });
    const p = setup();
    expect(await screen.findByText(/Índice por título/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Aplicar migraciones" }));
    const dlg = await screen.findByRole("alertdialog");
    expect(dlg.textContent).toMatch(/backup/);
    expect(calls("run_migrations")).toHaveLength(0);
    fireEvent.click(within(dlg).getByRole("button", { name: "Aplicar migraciones" }));
    expect(await screen.findByText(/C:\/d\/backups\/x\.bak/)).toBeTruthy();
    expect(p.onMigrated).toHaveBeenCalled();
  });

  it("toggles read-only and creates a backup", async () => {
    handlers.set_read_only = (a) => ({ ...session, readOnly: a.readOnly });
    handlers.create_backup = () => ({ path: "C:/d/backups/m.bak" });
    const p = setup();
    fireEvent.click(screen.getByRole("checkbox", { name: /solo lectura/ }));
    await waitFor(() => expect(p.onSessionChanged).toHaveBeenCalledWith(expect.objectContaining({ readOnly: true })));
    expect(calls("set_read_only")[0]).toEqual({ readOnly: true });
    fireEvent.click(screen.getByRole("button", { name: "Crear backup ahora" }));
    expect(await screen.findByText("C:/d/backups/m.bak")).toBeTruthy();
  });
});
