import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import PasswordsDialog from "../components/PasswordsDialog";
import PagePassword from "../components/PagePassword";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mockInvoke = vi.mocked(invoke);
const calls = (cmd: string) => mockInvoke.mock.calls.filter((c) => c[0] === cmd);

// Fake keyring keyed like the backend (trim + lowercase).
let store: Map<string, string>;
let unavailable = false;
const key = (p: unknown) => String(p).trim().toLowerCase();
beforeEach(() => {
  store = new Map();
  unavailable = false;
  mockInvoke.mockReset();
  mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
    const a = (args ?? {}) as Record<string, unknown>;
    if (unavailable) throw { kind: "SecureStorage", message: "El almacén seguro de contraseñas del sistema no está disponible." };
    switch (cmd) {
      case "page_passwords_status": return (a.paginas as string[]).filter((p) => store.has(key(p)));
      case "get_page_password": return store.get(key(a.pagina)) ?? null;
      case "set_page_password": store.set(key(a.pagina), String(a.password)); return null;
      case "delete_page_password": store.delete(key(a.pagina)); return null;
      case "copy_page_password": if (!store.has(key(a.pagina))) throw { kind: "NotFound", message: "sin" }; return null;
      default: throw new Error(`unexpected ${cmd}`);
    }
  });
});

const row = (name: string) => screen.getByText(name, { selector: "strong" }).closest("li") as HTMLElement;

describe("PasswordsDialog", () => {
  it("adds, reveals, copies, edits and deletes a page password", async () => {
    const toast = vi.fn();
    render(<PasswordsDialog paginas={["ryuugames", "Otome"]} onClose={() => undefined} onToast={toast} />);
    await waitFor(() => expect(within(row("ryuugames")).getByText("Sin contraseña")).toBeTruthy());

    fireEvent.click(within(row("ryuugames")).getByRole("button", { name: "Añadir" }));
    const input = within(row("ryuugames")).getByLabelText("Contraseña de ryuugames") as HTMLInputElement;
    expect(input.type).toBe("password");
    fireEvent.change(input, { target: { value: "abc123" } });
    fireEvent.click(within(row("ryuugames")).getByRole("button", { name: "Guardar" }));
    await waitFor(() => expect(within(row("ryuugames")).getByText("Guardada")).toBeTruthy());
    expect(within(row("Otome")).getByText("Sin contraseña")).toBeTruthy();

    // Hidden by default; nothing read from the store until «Mostrar».
    expect(screen.queryByText("abc123")).toBeNull();
    expect(calls("get_page_password")).toHaveLength(0);
    fireEvent.click(screen.getByRole("button", { name: "Mostrar contraseña de ryuugames" }));
    await screen.findByText("abc123");
    fireEvent.click(screen.getByRole("button", { name: "Ocultar contraseña de ryuugames" }));
    expect(screen.queryByText("abc123")).toBeNull();

    // Copy is done by the backend: the frontend never receives the secret for it.
    fireEvent.click(screen.getByRole("button", { name: "Copiar contraseña de ryuugames" }));
    await waitFor(() => expect(toast).toHaveBeenCalledWith("Contraseña de «ryuugames» copiada"));
    expect(calls("copy_page_password")[0][1]).toEqual({ pagina: "ryuugames" });

    fireEvent.click(within(row("ryuugames")).getByRole("button", { name: "Cambiar" }));
    fireEvent.change(within(row("ryuugames")).getByLabelText("Contraseña de ryuugames"), { target: { value: "nueva" } });
    fireEvent.click(within(row("ryuugames")).getByRole("button", { name: "Guardar" }));
    await waitFor(() => expect(store.get("ryuugames")).toBe("nueva"));

    fireEvent.click(within(row("ryuugames")).getByRole("button", { name: "Eliminar" }));
    fireEvent.click(within(screen.getByRole("alertdialog")).getByRole("button", { name: "Eliminar" }));
    await waitFor(() => expect(within(row("ryuugames")).getByText("Sin contraseña")).toBeTruthy());
    expect(store.size).toBe(0);
  });

  it("reports an unavailable secure store instead of falling back", async () => {
    unavailable = true;
    render(<PasswordsDialog paginas={["ryuugames"]} onClose={() => undefined} onToast={() => undefined} />);
    expect((await screen.findByRole("alert")).textContent).toMatch(/no está disponible/);
    expect((within(row("ryuugames")).getByRole("button", { name: "Añadir" }) as HTMLButtonElement).disabled).toBe(true);
  });
});

describe("PagePassword (game detail)", () => {
  it("shows the page password actions only when the game's page has one", async () => {
    store.set("ryuugames", "pw");
    const { rerender } = render(<PagePassword pagina="Ryuugames" />);
    await screen.findByRole("button", { name: "Copiar contraseña de Ryuugames" });
    expect(screen.queryByText("pw")).toBeNull();

    rerender(<PagePassword pagina="Otome" />);
    await waitFor(() => expect(screen.queryByRole("button", { name: /Copiar/ })).toBeNull());
    rerender(<PagePassword pagina="" />);
    expect(calls("page_passwords_status").every((c) => (c[1] as { paginas: string[] }).paginas[0] !== "")).toBe(true);
  });
});
