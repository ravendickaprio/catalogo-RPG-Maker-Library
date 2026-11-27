import { describe, it, expect, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import SearchBar from "../components/SearchBar";
import type { LibraryInfo } from "../api/types";

const info = (tags: LibraryInfo["tags"]): LibraryInfo => ({
  dbFileName: "x.db", readOnly: true, total: 3, ftsAvailable: true, ftsInSync: true,
  estados: [{ value: "Terminado", count: 1 }], idiomas: [{ value: "Español", count: 2 }], tags, sqliteVersion: "3",
});

function props(over: Partial<React.ComponentProps<typeof SearchBar>> = {}) {
  return {
    info: info([{ value: "fantasía", count: 4 }]), query: "", estado: "", idioma: "", tag: "", sort: "title_asc" as const,
    onQuery: vi.fn(), onEstado: vi.fn(), onIdioma: vi.fn(), onTag: vi.fn(), onSort: vi.fn(), onSuggest: vi.fn(), onClear: vi.fn(),
    ...over,
  };
}

describe("SearchBar", () => {
  it("disables Limpiar with no filters and enables it when one is set", () => {
    const { rerender } = render(<SearchBar {...props()} />);
    expect((screen.getByRole("button", { name: "Limpiar" }) as HTMLButtonElement).disabled).toBe(true);
    const p = props({ tag: "fantasía" });
    rerender(<SearchBar {...p} />);
    const btn = screen.getByRole("button", { name: "Limpiar" }) as HTMLButtonElement;
    expect(btn.disabled).toBe(false);
    fireEvent.click(btn);
    expect(p.onClear).toHaveBeenCalledTimes(1);
  });

  it("renders tag options with counts and reports selection", () => {
    const p = props();
    render(<SearchBar {...p} />);
    expect(screen.getByRole("option", { name: "fantasía (4)" })).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Etiqueta"), { target: { value: "fantasía" } });
    expect(p.onTag).toHaveBeenCalledWith("fantasía");
  });

  it("hides the tag select when there are no tags", () => {
    render(<SearchBar {...props({ info: info([]) })} />);
    expect(screen.queryByLabelText("Etiqueta")).toBeNull();
  });

  it("Escape in the search input clears the text", () => {
    const p = props({ query: "abc" });
    render(<SearchBar {...p} />);
    fireEvent.keyDown(screen.getByLabelText("Buscar"), { key: "Escape" });
    expect(p.onQuery).toHaveBeenCalledWith("");
  });
});
