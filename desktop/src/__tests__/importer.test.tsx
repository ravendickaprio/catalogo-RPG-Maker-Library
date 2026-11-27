import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import ImportDialog from "../components/ImportDialog";
import type { ImportItem, ImportPreview, ImportResult, Vocabulary } from "../api/types";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  Channel: class {
    onmessage: ((m: unknown) => void) | null = null;
  },
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
const mockInvoke = vi.mocked(invoke);
const mockOpen = vi.mocked(open);

const vocab: Vocabulary = { estados: ["Sin empezar", "Empezado", "Terminado"], idiomas: ["Japonés", "Español"], paginas: ["A"], tipos: ["RPG", "Otro"], puntuaciones: [0] };
const item = (key: string, status: ImportItem["status"], over: Partial<ImportItem> = {}): ImportItem => ({
  key, kind: "carpeta", path: `C:/juegos/${key}`, name: key, status, duplicateOf: null,
  parsed: { titulo: `Titulo ${key}`, codigo: status === "sin_codigo" ? "" : `RY-${key}`, estado: "Sin empezar", idioma: "Japonés" }, ...over,
});
const preview = (): ImportPreview => ({
  root: "C:/juegos",
  items: [item("1", "nuevo"), item("2", "duplicado_codigo", { duplicateOf: { id: 5, titulo: "Existente" } }), item("3", "sin_codigo")],
  defaults: { pagina: "DLsite", tipo: "RPG" },
  counts: { total: 3, nuevo: 1, duplicadoCodigo: 1, duplicadoRuta: 0, duplicadoLote: 0, sinCodigo: 1 },
  cancelled: false, truncated: false, scanned: 10, unreadable: 0,
});
const calls = (cmd: string) => mockInvoke.mock.calls.filter((c) => c[0] === cmd).map((c) => c[1] as Record<string, unknown>);

let handlers: Record<string, (a: Record<string, unknown>) => unknown>;
beforeEach(() => {
  handlers = {};
  mockInvoke.mockReset();
  mockOpen.mockReset();
  mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
    const h = handlers[cmd];
    if (h) return h((args ?? {}) as Record<string, unknown>);
    if (cmd === "scan_import") return preview();
    if (cmd === "get_import_history") return [];
    if (cmd === "cancel_scan") return null;
    throw new Error("unexpected " + cmd);
  });
});

function setup(readOnly = false) {
  const onChanged = vi.fn();
  const onClose = vi.fn();
  render(<ImportDialog vocab={vocab} readOnly={readOnly} onClose={onClose} onChanged={onChanged} />);
  return { onChanged, onClose };
}
async function scan() {
  fireEvent.change(screen.getByLabelText("Carpeta a analizar"), { target: { value: "C:/juegos" } });
  fireEvent.click(screen.getByRole("button", { name: "Analizar" }));
  await screen.findByRole("table", { name: "Elementos detectados" });
}

describe("ImportDialog", () => {
  it("uses the folder picker and previews with only new items checked", async () => {
    mockOpen.mockResolvedValue("C:/juegos");
    setup();
    fireEvent.click(screen.getByRole("button", { name: "Elegir carpeta…" }));
    await waitFor(() => expect((screen.getByLabelText("Carpeta a analizar") as HTMLInputElement).value).toBe("C:/juegos"));
    expect(mockOpen).toHaveBeenCalledWith({ directory: true, multiple: false });
    fireEvent.click(screen.getByRole("button", { name: "Analizar" }));
    await screen.findByRole("table");
    expect((screen.getByLabelText("Importar 1") as HTMLInputElement).checked).toBe(true);
    expect((screen.getByLabelText("Importar 2") as HTMLInputElement).checked).toBe(false);
    expect((screen.getByLabelText("Importar 3") as HTMLInputElement).checked).toBe(false);
    expect(screen.getByText(/Duplicado \(código\): «Existente»/)).toBeTruthy();
    expect(screen.getByDisplayValue("DLsite")).toBeTruthy();
  });

  it("disables duplicates and sin_codigo", async () => {
    setup();
    await scan();
    expect((screen.getByLabelText("Importar 2") as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByLabelText("Importar 3") as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByLabelText("Importar 1") as HTMLInputElement).disabled).toBe(false);
  });

  it("sends edited title and shows the summary with skipped reasons", async () => {
    handlers.run_import = () =>
      ({ inserted: [{ id: 9, titulo: "Nuevo titulo", codigo: "RY-1" }], skipped: [{ key: "1", reason: "código duplicado" }], backupPath: "C:/bk.db", importId: "i1", warning: "no se pudo escribir el historial" }) satisfies ImportResult;
    const { onChanged } = setup();
    await scan();
    fireEvent.change(screen.getByLabelText("Título de 1"), { target: { value: "Nuevo titulo" } });
    fireEvent.click(screen.getByRole("button", { name: "Importar 1 juegos" }));
    const dlg = await screen.findByRole("alertdialog");
    fireEvent.click(within(dlg).getByRole("button", { name: "Importar 1 juegos" }));
    await screen.findByText(/Importados: 1/);
    const req = calls("run_import")[0].req as { pagina: string; items: { titulo: string; codigo: string }[] };
    expect(req.pagina).toBe("DLsite");
    expect(req.items).toHaveLength(1);
    expect(req.items[0]).toMatchObject({ titulo: "Nuevo titulo", codigo: "RY-1" });
    expect(screen.getByText(/1: código duplicado/)).toBeTruthy();
    expect(screen.getByText("C:/bk.db")).toBeTruthy();
    expect(screen.getByText(/no se pudo escribir el historial/)).toBeTruthy();
    expect(onChanged).toHaveBeenCalled();
  });

  it("cancel calls cancel_scan", async () => {
    let finish: (p: ImportPreview) => void = () => undefined;
    handlers.scan_import = () => new Promise<ImportPreview>((r) => (finish = r));
    setup();
    fireEvent.change(screen.getByLabelText("Carpeta a analizar"), { target: { value: "C:/juegos" } });
    fireEvent.click(screen.getByRole("button", { name: "Analizar" }));
    fireEvent.click(await screen.findByRole("button", { name: "Cancelar" }));
    await waitFor(() => expect(calls("cancel_scan")).toHaveLength(1));
    finish({ ...preview(), items: [], cancelled: true });
    await screen.findByText("Análisis cancelado.");
  });

  it("read-only allows scanning but disables import", async () => {
    setup(true);
    await scan();
    expect((screen.getByRole("button", { name: "Importar 1 juegos" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getAllByText(/solo lectura/).length).toBeGreaterThan(0);
  });

  it("undo asks for confirmation and shows the result", async () => {
    handlers.get_import_history = () => [{ id: "i1", timestamp: "2026-01-01T00:00:00Z", root: "C:/juegos", insertedCount: 2, skippedCount: 0, backupPath: null, undone: false, undoneAt: null }];
    handlers.undo_last_import = () => ({ importId: "i1", deleted: 1, kept: [{ id: 3, titulo: "Editado", reason: "modificado" }], backupPath: null, warning: "historial no actualizado" });
    const { onChanged } = setup();
    fireEvent.click(screen.getByRole("button", { name: "Historial" }));
    const btn = await screen.findByRole("button", { name: "Deshacer última importación" });
    await waitFor(() => expect((btn as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(btn);
    expect(calls("undo_last_import")).toHaveLength(0);
    fireEvent.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Deshacer importación" }));
    await screen.findByText(/Eliminados: 1/);
    expect(screen.getByText(/Editado: modificado/)).toBeTruthy();
    expect(screen.getByText("historial no actualizado")).toBeTruthy();
    expect(onChanged).toHaveBeenCalled();
  });

  it("shows a NotFound undo error as a message", async () => {
    handlers.get_import_history = () => [{ id: "i1", timestamp: "2026-01-01T00:00:00Z", root: "C:/j", insertedCount: 1, skippedCount: 0, backupPath: null, undone: false, undoneAt: null }];
    handlers.undo_last_import = () => Promise.reject({ kind: "NotFound", message: "No hay importaciones de esta base que deshacer." });
    setup();
    fireEvent.click(screen.getByRole("button", { name: "Historial" }));
    const btn = await screen.findByRole("button", { name: "Deshacer última importación" });
    await waitFor(() => expect((btn as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(btn);
    fireEvent.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Deshacer importación" }));
    await screen.findByText("No hay importaciones de esta base que deshacer.");
  });
});
