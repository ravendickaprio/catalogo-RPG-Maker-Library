import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import MaintenanceDialog from "../components/MaintenanceDialog";
import type { BatchPreview, Issue, Vocabulary } from "../api/types";

let lastChannel: { onmessage: ((m: unknown) => void) | null } | null = null;
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  Channel: class {
    onmessage: ((m: unknown) => void) | null = null;
    constructor() {
      // eslint-disable-next-line @typescript-eslint/no-this-alias
      lastChannel = this;
    }
  },
}));
const mockInvoke = vi.mocked(invoke);

const vocab: Vocabulary = { estados: ["Sin empezar", "Empezado", "Terminado"], idiomas: ["Japonés", "Español"], paginas: ["A"], tipos: ["RPG", "Otro"], puntuaciones: [0] };
const issue = (id: number, over: Partial<Issue> = {}): Issue => ({ id, titulo: `Juego ${id}`, codigo: `RY-${id}`, estado: "Sin empezar", ruta: `C:/j/${id}`, detail: null, ...over });
const calls = (cmd: string) => mockInvoke.mock.calls.filter((c) => c[0] === cmd).map((c) => c[1] as Record<string, unknown>);

let handlers: Record<string, (a: Record<string, unknown>) => unknown>;
beforeEach(() => {
  handlers = {};
  lastChannel = null;
  mockInvoke.mockReset();
  mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
    const h = handlers[cmd];
    if (h) return h((args ?? {}) as Record<string, unknown>);
    switch (cmd) {
      case "maintenance_summary":
        return [
          { check: "sin_descripcion", count: 3 },
          { check: "sin_portada", count: 7 },
          { check: "sin_tags", count: 0 },
          { check: "codigo_duplicado", count: 2 },
          { check: "codigo_vacio", count: 1 },
          { check: "titulo_ry", count: 4 },
          { check: "estado_inconsistente", count: 5 },
        ];
      case "search_index_status":
        return { juegos: 10, busqueda: 10, mismatched: 0, ftsOk: true };
      case "list_orphan_tags":
        return [{ id: 1, nombre: "huerfana" }];
      case "maintenance_list":
        return { items: [issue(1), issue(2), issue(3)], total: 3, offset: 0, limit: 200 };
      case "list_batches":
        return [];
      case "cancel_maintenance_scan":
        return null;
      default:
        throw new Error("unexpected " + cmd);
    }
  });
});

function setup(readOnly = false) {
  const onChanged = vi.fn();
  const onClose = vi.fn();
  const onOpenGame = vi.fn();
  const onOpenReview = vi.fn();
  render(<MaintenanceDialog vocab={vocab} readOnly={readOnly} onClose={onClose} onChanged={onChanged} onOpenGame={onOpenGame} onOpenReview={onOpenReview} />);
  return { onChanged, onClose, onOpenGame, onOpenReview };
}
async function openDetector(name: RegExp) {
  fireEvent.click(await screen.findByRole("button", { name }));
  await screen.findByRole("table", { name: "Problemas detectados" });
}
function chooseEstado(value: string) {
  fireEvent.change(screen.getByLabelText("Valor"), { target: { value } });
}

describe("MaintenanceDialog", () => {
  it("renders detector counts, index status and orphan tags", async () => {
    setup();
    const card = await screen.findByRole("button", { name: /Sin portada/ });
    await waitFor(() => expect(card.textContent).toContain("7"));
    expect(screen.getByRole("button", { name: /Código duplicado/ }).textContent).toContain("2");
    expect(screen.getByRole("button", { name: /Ruta inexistente/ }).textContent).toContain("—");
    await screen.findByText(/10 juegos · 10 en el índice · 0 desajustados/);
    await screen.findByText(/1 tags sin ningún juego: huerfana/);
  });

  it("lists a detector, pages and selects rows", async () => {
    handlers.maintenance_list = (a) => {
      const req = a.req as { offset: number };
      return req.offset === 0 ? { items: [issue(1), issue(2)], total: 3, offset: 0, limit: 200 } : { items: [issue(3, { detail: "comparte código con #1" })], total: 3, offset: 2, limit: 200 };
    };
    setup();
    await openDetector(/Sin portada/);
    fireEvent.click(screen.getByLabelText("Seleccionar Juego 1"));
    expect(screen.getByText(/1 seleccionados · 2 de 3 cargados/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cargar más" }));
    await screen.findByText("comparte código con #1");
    expect(calls("maintenance_list").map((c) => (c.req as { offset: number }).offset)).toEqual([0, 2]);
    fireEvent.click(screen.getByLabelText("Seleccionar todos los cargados"));
    expect(screen.getByText(/3 seleccionados · 3 de 3 cargados/)).toBeTruthy();
  });

  it("opens a game from a row", async () => {
    const { onOpenGame } = setup();
    await openDetector(/Sin portada/);
    fireEvent.click(screen.getByRole("button", { name: "Juego 2" }));
    expect(onOpenGame).toHaveBeenCalledWith({ id: 2, codigo: "RY-2", titulo: "Juego 2" });
  });

  it("previews then applies with ids, action and revisions from the preview, and shows skipped reasons", async () => {
    const pv: BatchPreview = {
      changes: [
        { id: 1, titulo: "Juego 1", before: "Sin empezar", after: "Terminado", revision: "r1" },
        { id: 2, titulo: "Juego 2", before: "Sin empezar", after: "Terminado", revision: "r2" },
      ],
      skipped: [{ id: 3, reason: "ya tiene ese valor" }],
    };
    handlers.preview_batch = () => pv;
    handlers.apply_batch = () => ({ changed: 1, skipped: [{ id: 2, reason: "modificado desde la vista previa" }], backupPath: "C:/bk.db", batchId: "b1", warning: "registro de deshacer no escrito" });
    const { onChanged } = setup();
    await openDetector(/Sin portada/);
    fireEvent.click(screen.getByLabelText("Seleccionar todos los cargados"));
    chooseEstado("Terminado");
    fireEvent.click(screen.getByRole("button", { name: "Vista previa" }));
    await screen.findByRole("table", { name: "Cambios propuestos" });
    expect(screen.getByText("ya tiene ese valor", { exact: false })).toBeTruthy();
    expect(calls("preview_batch")[0]).toEqual({ req: { ids: [1, 2, 3], action: { kind: "set_field", field: "estado", value: "Terminado" } } });
    expect(calls("apply_batch")).toHaveLength(0);
    fireEvent.click(screen.getByRole("button", { name: "Aplicar 2 cambios" }));
    fireEvent.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Aplicar 2 cambios" }));
    await screen.findByText(/Cambiados: 1 · omitidos: 1/);
    expect(calls("apply_batch")[0]).toEqual({
      req: { ids: [1, 2], action: { kind: "set_field", field: "estado", value: "Terminado" } },
      revisions: { 1: "r1", 2: "r2" },
    });
    expect(screen.getByText(/modificado desde la vista previa/)).toBeTruthy();
    expect(screen.getByText("C:/bk.db")).toBeTruthy();
    expect(screen.getByText("registro de deshacer no escrito")).toBeTruthy();
    await waitFor(() => expect(onChanged).toHaveBeenCalled());
    // counts reloaded
    await waitFor(() => expect(calls("maintenance_summary").length).toBeGreaterThan(1));
  });

  it("proposes RY titles on the titulo_ry detector", async () => {
    handlers.preview_batch = () => ({ changes: [{ id: 1, titulo: "RY-1 Algo", before: "RY-1 Algo", after: "Algo", revision: "r1" }], skipped: [] });
    setup();
    await openDetector(/Título con prefijo RY/);
    fireEvent.click(screen.getByLabelText("Seleccionar Juego 1"));
    fireEvent.click(screen.getByRole("button", { name: "Proponer títulos" }));
    await screen.findByRole("table", { name: "Cambios propuestos" });
    expect((calls("preview_batch")[0].req as { action: unknown }).action).toEqual({ kind: "fix_ry_titles" });
  });

  it("sends add_tag with the typed tag", async () => {
    handlers.preview_batch = () => ({ changes: [], skipped: [] });
    setup();
    await openDetector(/Sin portada/);
    fireEvent.click(screen.getByLabelText("Seleccionar Juego 1"));
    fireEvent.change(screen.getByLabelText("Tipo de acción"), { target: { value: "add_tag" } });
    fireEvent.change(screen.getByLabelText("Tag"), { target: { value: " nueva " } });
    fireEvent.click(screen.getByRole("button", { name: "Vista previa" }));
    await screen.findByRole("button", { name: "Aplicar 0 cambios" });
    expect(calls("preview_batch")[0]).toEqual({ req: { ids: [1], action: { kind: "add_tag", tag: "nueva" } } });
    expect((screen.getByRole("button", { name: "Aplicar 0 cambios" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("read-only disables batch, rebuild and tag deletion but lists issues", async () => {
    setup(true);
    await openDetector(/Sin portada/);
    fireEvent.click(screen.getByLabelText("Seleccionar Juego 1"));
    expect((screen.getByRole("button", { name: "Vista previa" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Reconstruir índice" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Eliminar tags huérfanas" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Comprobar rutas" }) as HTMLButtonElement).disabled).toBe(false);
    expect(screen.getAllByText(/solo lectura/).length).toBeGreaterThan(0);
  });

  it("rebuilds the index after confirmation and deletes orphan tags after confirmation", async () => {
    handlers.rebuild_search_index = () => ({ juegos: 10, busqueda: 10, mismatched: 0, ftsOk: true });
    handlers.delete_orphan_tags = () => ({ changed: 1, skipped: [], backupPath: null, batchId: "b2", warning: null });
    setup();
    await screen.findByText(/10 juegos/);
    fireEvent.click(screen.getByRole("button", { name: "Reconstruir índice" }));
    expect(calls("rebuild_search_index")).toHaveLength(0);
    fireEvent.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Reconstruir índice" }));
    await screen.findByText("Índice de búsqueda reconstruido.");
    await screen.findByText(/1 tags sin ningún juego/);
    fireEvent.click(screen.getByRole("button", { name: "Eliminar tags huérfanas" }));
    fireEvent.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Eliminar tags huérfanas" }));
    await screen.findByText(/Tags huérfanas eliminadas: 1/);
    expect(calls("delete_orphan_tags")).toHaveLength(1);
  });

  it("shows backend errors in an alert", async () => {
    handlers.maintenance_list = () => Promise.reject({ kind: "InvalidInput", message: "límite no válido" });
    setup();
    fireEvent.click(await screen.findByRole("button", { name: /Sin portada/ }));
    expect((await screen.findByRole("alert")).textContent).toBe("límite no válido");
  });

  it("undo asks for confirmation once and shows restored, kept and warning", async () => {
    handlers.list_batches = () => [{ id: "b1", timestamp: "2026-01-01T00:00:00Z", description: "estado → Terminado", changed: 2, undone: false }];
    let n = 0;
    handlers.undo_last_batch = async () => {
      n++;
      return { batchId: "b1", restored: 1, kept: [{ id: 3, titulo: "Editado", reason: "modificado" }], backupPath: "C:/u.db", warning: "no se marcó como deshecho" };
    };
    const { onChanged } = setup();
    fireEvent.click(screen.getByRole("button", { name: "Historial de lotes" }));
    await screen.findByText(/estado → Terminado/);
    const btn = screen.getByRole("button", { name: "Deshacer último lote" });
    await waitFor(() => expect((btn as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(btn);
    expect(calls("undo_last_batch")).toHaveLength(0);
    const confirmBtn = within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Deshacer lote" });
    fireEvent.click(confirmBtn);
    fireEvent.click(confirmBtn);
    await screen.findByText(/Restaurados: 1 · conservados: 1/);
    expect(n).toBe(1);
    expect(screen.getByText(/Editado: modificado/)).toBeTruthy();
    expect(screen.getByText("no se marcó como deshecho")).toBeTruthy();
    expect(onChanged).toHaveBeenCalled();
  });

  it("read-only disables undo", async () => {
    handlers.list_batches = () => [{ id: "b1", timestamp: "2026-01-01T00:00:00Z", description: "x", changed: 2, undone: false }];
    setup(true);
    fireEvent.click(screen.getByRole("button", { name: "Historial de lotes" }));
    await screen.findByText(/· x ·/);
    expect((screen.getByRole("button", { name: "Deshacer último lote" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("the review button opens the batch review preset to sin_descripcion", async () => {
    const { onOpenReview } = setup();
    fireEvent.click(await screen.findByRole("button", { name: "Revisión por tandas (sin descripción)" }));
    expect(onOpenReview).toHaveBeenCalledWith("sin_descripcion");
  });

  it("missing-path scan shows progress, can be cancelled and lists partial results", async () => {
    let finish: (r: unknown) => void = () => undefined;
    handlers.scan_missing_paths = () => new Promise((r) => (finish = r));
    setup();
    fireEvent.click(await screen.findByRole("button", { name: "Comprobar rutas" }));
    await screen.findByText(/Comprobando rutas… 0 de \?/);
    lastChannel?.onmessage?.({ checked: 50, total: 200 });
    await screen.findByText(/Comprobando rutas… 50 de 200/);
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    await waitFor(() => expect(calls("cancel_maintenance_scan")).toHaveLength(1));
    finish({ items: [issue(9, { detail: "ruta de Windows" })], cancelled: true });
    await screen.findByText(/Comprobación cancelada/);
    await screen.findByRole("table", { name: "Problemas detectados" });
    expect(screen.getByText("ruta de Windows")).toBeTruthy();
    expect(screen.getByRole("button", { name: /Ruta inexistente/ }).textContent).toContain("1");
  });

  it("Escape closes the dialog unless an operation is running", async () => {
    let finish: (r: unknown) => void = () => undefined;
    handlers.scan_missing_paths = () => new Promise((r) => (finish = r));
    const { onClose } = setup();
    const dlg = screen.getByRole("dialog");
    fireEvent.click(await screen.findByRole("button", { name: "Comprobar rutas" }));
    await screen.findByText(/Comprobando rutas/);
    fireEvent.keyDown(dlg, { key: "Escape" });
    expect(onClose).not.toHaveBeenCalled();
    finish({ items: [], cancelled: false });
    await screen.findByText(/Comprobación terminada/);
    await waitFor(() => {
      fireEvent.keyDown(dlg, { key: "Escape" }); // busy is released by an effect right after the result
      expect(onClose).toHaveBeenCalled();
    });
  });

  it("a late detector load does not overwrite the missing-paths scan result", async () => {
    let finishList: (r: unknown) => void = () => undefined;
    handlers.maintenance_list = () => new Promise((r) => (finishList = r));
    handlers.scan_missing_paths = () => ({ items: [issue(9, { detail: "no existe" })], cancelled: false });
    setup();
    fireEvent.click(await screen.findByRole("button", { name: /Sin portada/ }));
    await waitFor(() => expect(calls("maintenance_list").length).toBeGreaterThan(0));
    fireEvent.click(screen.getByRole("button", { name: "Comprobar rutas" }));
    await screen.findByText(/Comprobación terminada/);
    finishList({ items: [issue(1), issue(2)], total: 2, offset: 0, limit: 200 });
    await new Promise((r) => setTimeout(r, 20));
    expect(screen.getByText("no existe")).toBeTruthy();
    expect(screen.queryByLabelText("Seleccionar Juego 1")).toBeNull();
    expect(screen.getByLabelText("Seleccionar Juego 9")).toBeTruthy();
  });

  it("discards a preview that arrives after the selection changed", async () => {
    let finish: (r: unknown) => void = () => undefined;
    handlers.preview_batch = () => new Promise((r) => (finish = r));
    setup();
    await openDetector(/Sin portada/);
    fireEvent.click(screen.getByLabelText("Seleccionar Juego 1"));
    chooseEstado("Terminado");
    fireEvent.click(screen.getByRole("button", { name: "Vista previa" }));
    await screen.findByText(/Calculando vista previa/);
    fireEvent.click(screen.getByLabelText("Seleccionar Juego 2"));
    finish({ changes: [{ id: 1, titulo: "Juego 1", before: "a", after: "b", revision: "r1" }], skipped: [] });
    await new Promise((r) => setTimeout(r, 20));
    expect(screen.queryByRole("table", { name: "Cambios propuestos" })).toBeNull();
    expect(screen.queryByText(/Calculando vista previa/)).toBeNull();
  });

  it("unchecked preview rows are excluded from apply_batch ids and revisions", async () => {
    handlers.preview_batch = () => ({
      changes: [
        { id: 1, titulo: "Juego 1", before: "a", after: "b", revision: "r1" },
        { id: 2, titulo: "Juego 2", before: "a", after: "b", revision: "r2" },
        { id: 3, titulo: "Juego 3", before: "a", after: "b", revision: "r3" },
      ],
      skipped: [],
    });
    handlers.apply_batch = () => ({ changed: 2, skipped: [], backupPath: null, batchId: "b", warning: null });
    setup();
    await openDetector(/Sin portada/);
    fireEvent.click(screen.getByLabelText("Seleccionar todos los cargados"));
    chooseEstado("Terminado");
    fireEvent.click(screen.getByRole("button", { name: "Vista previa" }));
    await screen.findByRole("table", { name: "Cambios propuestos" });
    fireEvent.click(screen.getByLabelText("Aplicar a Juego 2"));
    fireEvent.click(screen.getByRole("button", { name: "Aplicar 2 cambios" }));
    fireEvent.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Aplicar 2 cambios" }));
    await screen.findByText(/Cambiados: 2/);
    expect(calls("apply_batch")[0]).toEqual({
      req: { ids: [1, 3], action: { kind: "set_field", field: "estado", value: "Terminado" } },
      revisions: { 1: "r1", 3: "r3" },
    });
  });

  it("unchecking every preview row disables apply; the toggle-all box restores them", async () => {
    handlers.preview_batch = () => ({ changes: [{ id: 1, titulo: "Juego 1", before: "a", after: "b", revision: "r1" }], skipped: [] });
    setup();
    await openDetector(/Sin portada/);
    fireEvent.click(screen.getByLabelText("Seleccionar Juego 1"));
    chooseEstado("Terminado");
    fireEvent.click(screen.getByRole("button", { name: "Vista previa" }));
    await screen.findByRole("table", { name: "Cambios propuestos" });
    const all = screen.getByLabelText("Marcar o desmarcar todos los cambios");
    fireEvent.click(all);
    const btn = screen.getByRole("button", { name: "Aplicar 0 cambios" }) as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
    fireEvent.click(all);
    expect((screen.getByRole("button", { name: "Aplicar 1 cambios" }) as HTMLButtonElement).disabled).toBe(false);
  });
});
