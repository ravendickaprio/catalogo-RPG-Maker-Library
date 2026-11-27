import { describe, it, expect, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import SearchBar from "../components/SearchBar";

function props(over: Partial<React.ComponentProps<typeof SearchBar>> = {}) {
  return {
    query: "", estado: "", idioma: "", tag: "", sort: "title_asc" as const,
    onQuery: vi.fn(), onSort: vi.fn(), onSuggest: vi.fn(), onClear: vi.fn(),
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

  it("no longer renders estado/idioma/tag selects (covered by the sidebar)", () => {
    render(<SearchBar {...props()} />);
    expect(screen.queryByLabelText("Estado")).toBeNull();
    expect(screen.queryByLabelText("Idioma")).toBeNull();
    expect(screen.queryByLabelText("Etiqueta")).toBeNull();
  });

  it("'/' focuses the search input but Ctrl+K does not", () => {
    render(<SearchBar {...props()} />);
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    expect(document.activeElement).not.toBe(screen.getByLabelText("Buscar"));
    fireEvent.keyDown(window, { key: "/" });
    expect(document.activeElement).toBe(screen.getByLabelText("Buscar"));
  });

  it("Escape in the search input clears the text", () => {
    const p = props({ query: "abc" });
    render(<SearchBar {...p} />);
    fireEvent.keyDown(screen.getByLabelText("Buscar"), { key: "Escape" });
    expect(p.onQuery).toHaveBeenCalledWith("");
  });
});
