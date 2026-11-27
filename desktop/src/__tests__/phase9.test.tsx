import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import App from "../App";
import type { BackupCheck, BackupEntry, LibraryInfo, SessionInfo } from "../api/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));
const mockInvoke = vi.mocked(invoke);
const mockOpen = vi.mocked(open);
const mockSave = vi.mocked(save);

const info: LibraryInfo = {
  dbFileName: "x.db", readOnly: false, total: 3, ftsAvailable: true, ftsInSync: true,
  estados: [], idiomas: [], tags: [], sqliteVersion: "3",
};
const session = (readOnly: boolean): SessionInfo => ({ dbFileName: "x.db", dbDir: "C:/db", readOnly, lastBackup: null }) as SessionInfo;
const check: BackupCheck = { ok: true, integrity: "ok", foreignKeys: 0, ftsOk: true, games: 3, schemaOk: true };
const backups: BackupEntry[] = [
  { fileName: "juegos-1.db", size: 2048 * 1024, modified: "2026-01-02T10:00:00Z", preRestore: false },
  { fileName: "pre-restore-2.db", size: 4096, modified: "2026-01-01T10:00:00Z", preRestore: true },
];
const calls = (cmd: string) => mockInvoke.mock.calls.filter((c) => c[0] === cmd).map((c) => c[1] as Record<string, unknown>);

let handlers: Record<string, (a: Record<string, unknown>) => unknown>;
let ro = false;
beforeEach(() => {
  localStorage.clear();
  localStorage.setItem("rpglib.welcome", "off");
  handlers = {};
  ro = false;
  mockInvoke.mockReset();
  mockOpen.mockReset();
  mockSave.mockReset();
  mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
    const h = handlers[cmd];
    if (h) return h((args ?? {}) as Record<string, unknown>);
    switch (cmd) {
      case "get_library_info": return info;
      case "get_session_info": return session(ro);
      case "get_vocabulary": return { estados: [], idiomas: [], paginas: [], tipos: [], puntuaciones: [] };
      case "search_games": return { items: [], total: 0, offset: 0, limit: 100, elapsedMs: 1, strategy: "none" };
      case "check_data_version": return { version: 1, changed: false };
      case "get_migration_status": return { current: 1, latest: 1, pending: [] };
      case "list_backups": return backups;
    }
    throw new Error("unexpected " + cmd);
  });
});

async function openSettings() {
  render(<App />);
  await screen.findByText("3 juegos");
  fireEvent.click(screen.getByRole("button", { name: "Ajustes" }));
  await screen.findByRole("list", { name: "Copias de seguridad" });
}

describe("first run", () => {
  const configErr = () => Promise.reject({ kind: "Config", message: "No hay base de datos configurada" });
  it("renders instead of the library on a Config error, picks a file and opens it", async () => {
    let configured = false;
    handlers.get_library_info = () => (configured ? info : configErr());
    handlers.get_session_info = () => (configured ? session(false) : configErr());
    handlers.set_db_path = () => {
      configured = true;
      return info;
    };
    mockOpen.mockResolvedValue("C:/db/juegos.db");
    render(<App />);
    await screen.findByRole("heading", { name: "Elegir base de datos" });
    expect(screen.queryByRole("button", { name: "Ajustes" })).toBeNull();
    expect(screen.getByText("No hay base de datos configurada")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Elegir archivo…" }));
    await waitFor(() => expect((screen.getByLabelText("Ruta de la base de datos") as HTMLInputElement).value).toBe("C:/db/juegos.db"));
    expect(mockOpen).toHaveBeenCalledWith(expect.objectContaining({ filters: [{ name: "SQLite", extensions: ["db", "sqlite", "sqlite3"] }] }));
    fireEvent.click(screen.getByRole("button", { name: "Abrir" }));
    await screen.findByText("3 juegos");
    expect(calls("set_db_path")).toEqual([{ path: "C:/db/juegos.db" }]);
    expect(screen.queryByRole("heading", { name: "Elegir base de datos" })).toBeNull();
  });

  it("shows set_db_path errors in an alert and stays", async () => {
    handlers.get_library_info = configErr;
    handlers.get_session_info = configErr;
    handlers.set_db_path = () => Promise.reject({ kind: "Config", message: "No es una base válida" });
    render(<App />);
    await screen.findByRole("heading", { name: "Elegir base de datos" });
    expect((screen.getByRole("button", { name: "Abrir" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("Ruta de la base de datos"), { target: { value: "C:/x.db" } });
    fireEvent.click(screen.getByRole("button", { name: "Abrir" }));
    expect((await screen.findByRole("alert")).textContent).toContain("No es una base válida");
    expect(screen.getByRole("heading", { name: "Elegir base de datos" })).toBeTruthy();
  });

  it("does not show first-run for other errors", async () => {
    handlers.get_library_info = () => Promise.reject({ kind: "Database", message: "boom" });
    render(<App />);
    await screen.findByText(/boom/);
    expect(screen.queryByRole("heading", { name: "Elegir base de datos" })).toBeNull();
  });
});

describe("backups in settings", () => {
  it("lists backups with size, date and pre-restore badge", async () => {
    await openSettings();
    const list = screen.getByRole("list", { name: "Copias de seguridad" });
    expect(within(list).getByText(/2,0 MB/)).toBeTruthy();
    expect(within(list).getByText(/4 KB/)).toBeTruthy();
    expect(within(list).getAllByText("pre-restauración")).toHaveLength(1);
  });

  it("verifies a backup and the current database", async () => {
    handlers.verify_backup = () => check;
    handlers.check_database = () => ({ ...check, ok: false, integrity: "corrupt" });
    await openSettings();
    fireEvent.click(screen.getByRole("button", { name: "Verificar juegos-1.db" }));
    expect((await screen.findByText(/Verificación correcta/)).textContent).toContain("juegos: 3");
    expect(calls("verify_backup")).toEqual([{ fileName: "juegos-1.db" }]);
    fireEvent.click(screen.getByRole("button", { name: "Verificar base actual" }));
    expect((await screen.findByText(/Verificación con problemas/)).textContent).toContain("corrupt");
  });

  it("requires confirmation, restores once and refreshes everything", async () => {
    handlers.restore_backup = () => ({ restoredFrom: "juegos-1.db", preRestoreBackup: "pre-restore-9.db", games: 3 });
    await openSettings();
    const before = calls("get_library_info").length;
    fireEvent.click(screen.getByRole("button", { name: "Restaurar juegos-1.db" }));
    expect(calls("restore_backup")).toHaveLength(0);
    const dlg = await screen.findByRole("alertdialog");
    expect(dlg.textContent).toContain("Se sustituirá el contenido actual");
    const go = within(dlg).getByRole("button", { name: "Restaurar" });
    fireEvent.click(go);
    fireEvent.click(go);
    expect((await screen.findByText(/Restaurado desde juegos-1.db/)).textContent).toContain("pre-restore-9.db");
    expect(calls("restore_backup")).toEqual([{ fileName: "juegos-1.db" }]);
    await waitFor(() => expect(calls("get_library_info").length).toBeGreaterThan(before));
  });

  it("disables verify/restore buttons while a restore runs and shows a reopen warning", async () => {
    let release: (v: unknown) => void = () => {};
    handlers.restore_backup = () =>
      new Promise((r) => {
        release = r;
      }) as never;
    await openSettings();
    fireEvent.click(screen.getByRole("button", { name: "Restaurar juegos-1.db" }));
    fireEvent.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Restaurar" }));
    await waitFor(() => expect(calls("restore_backup")).toHaveLength(1));
    const dis = (name: string) => (screen.getByRole("button", { name, hidden: true }) as HTMLButtonElement).disabled;
    expect(dis("Verificar juegos-1.db")).toBe(true);
    expect(dis("Restaurar juegos-1.db")).toBe(true);
    expect(dis("Verificar base actual")).toBe(true);
    release({ restoredFrom: "juegos-1.db", preRestoreBackup: "pre-restore-9.db", games: 3, warning: "no se pudo reabrir la base: x; reinicie la aplicación." });
    expect((await screen.findByText(/no se pudo reabrir la base/)).textContent).toContain("reinicie");
    expect(dis("Verificar base actual")).toBe(false);
  });

  it("cancel does not restore; read-only disables restore", async () => {
    await openSettings();
    fireEvent.click(screen.getByRole("button", { name: "Restaurar juegos-1.db" }));
    fireEvent.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Cancelar" }));
    expect(calls("restore_backup")).toHaveLength(0);
  });

  it("restore is disabled in read-only mode", async () => {
    ro = true;
    await openSettings();
    expect((screen.getByRole("button", { name: "Restaurar juegos-1.db" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Verificar juegos-1.db" }) as HTMLButtonElement).disabled).toBe(false);
  });

  it("refreshes the list after creating a backup; shows empty and error states", async () => {
    handlers.create_backup = () => ({ path: "C:/db/backups/n.db" });
    await openSettings();
    const n = calls("list_backups" as never).length;
    fireEvent.click(screen.getByRole("button", { name: "Crear backup ahora" }));
    await screen.findByText(/Backup creado/);
    await waitFor(() => expect(mockInvoke.mock.calls.filter((c) => c[0] === "list_backups").length).toBe(n + 1));
  });

  it("shows the empty state", async () => {
    handlers.list_backups = () => [];
    render(<App />);
    await screen.findByText("3 juegos");
    fireEvent.click(screen.getByRole("button", { name: "Ajustes" }));
    await screen.findByText("Todavía no hay copias de seguridad.");
  });

  it("shows list errors", async () => {
    handlers.list_backups = () => Promise.reject({ kind: "Database", message: "sin carpeta" });
    render(<App />);
    await screen.findByText("3 juegos");
    fireEvent.click(screen.getByRole("button", { name: "Ajustes" }));
    await waitFor(() => expect(screen.getAllByRole("alert").some((a) => a.textContent === "sin carpeta")).toBe(true));
  });
});

describe("export", () => {
  async function openExport(readOnly = false) {
    ro = readOnly;
    render(<App />);
    await screen.findByText("3 juegos");
    fireEvent.click(screen.getByRole("button", { name: /Herramientas/ }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Exportar biblioteca…" }));
    return screen.findByRole("dialog", { name: "Exportar biblioteca" });
  }
  it("cancelled save does not export", async () => {
    mockSave.mockResolvedValue(null);
    const dlg = await openExport();
    fireEvent.click(within(dlg).getByRole("button", { name: "Exportar…" }));
    await waitFor(() => expect(mockSave).toHaveBeenCalled());
    expect(calls("export_library")).toHaveLength(0);
  });

  it("exports JSON with the chosen path, in read-only too, and toasts", async () => {
    mockSave.mockResolvedValue("C:/out/lib.json");
    handlers.export_library = () => ({ path: "C:/out/lib.json", rows: 3 });
    const dlg = await openExport(true);
    fireEvent.click(within(dlg).getByLabelText("JSON"));
    fireEvent.click(within(dlg).getByRole("button", { name: "Exportar…" }));
    await screen.findByText("Exportados 3 juegos a lib.json");
    expect(calls("export_library")).toEqual([{ path: "C:/out/lib.json", format: "json" }]);
    const arg = mockSave.mock.calls[0][0] as { defaultPath: string; filters: unknown };
    expect(arg.defaultPath).toMatch(/^biblioteca-\d{8}\.json$/);
    expect(arg.filters).toEqual([{ name: "JSON", extensions: ["json"] }]);
  });

  it("shows export errors", async () => {
    mockSave.mockResolvedValue("C:/out/lib.csv");
    handlers.export_library = () => Promise.reject({ kind: "InvalidInput", message: "Ruta no permitida" });
    const dlg = await openExport();
    fireEvent.click(within(dlg).getByRole("button", { name: "Exportar…" }));
    expect((await within(dlg).findByRole("alert")).textContent).toBe("Ruta no permitida");
  });

  it("is available from the palette", async () => {
    render(<App />);
    await screen.findByText("3 juegos");
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    const input = screen.getByLabelText("Escribe un comando");
    fireEvent.change(input, { target: { value: "exportar" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await screen.findByRole("dialog", { name: "Exportar biblioteca" });
  });
});
