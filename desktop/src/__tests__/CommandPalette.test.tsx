import { describe, it, expect, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import CommandPalette from "../components/CommandPalette";
import { filterActions, type PaletteAction } from "../lib/palette";

const mk = (): PaletteAction[] => [
  { id: "a", label: "Vista lista", run: vi.fn() },
  { id: "b", label: "Vista galería", run: vi.fn() },
  { id: "c", label: "Tema: Noche", run: vi.fn() },
];

describe("palette", () => {
  it("filters ignoring case and accents, all terms required", () => {
    const a = mk();
    expect(filterActions(a, "galeria").map((x) => x.id)).toEqual(["b"]);
    expect(filterActions(a, "VISTA").map((x) => x.id)).toEqual(["a", "b"]);
    expect(filterActions(a, "vista noche")).toEqual([]);
    expect(filterActions(a, "  ")).toHaveLength(3);
  });

  it("types to filter, Enter runs the active action and closes", () => {
    const a = mk();
    const onClose = vi.fn();
    render(<CommandPalette actions={a} onClose={onClose} />);
    expect(screen.getByRole("dialog")).toBeTruthy();
    const input = screen.getByRole("combobox");
    fireEvent.change(input, { target: { value: "vista" } });
    expect(screen.getAllByRole("option")).toHaveLength(2);
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(input.getAttribute("aria-activedescendant")).toBe("palette-opt-1");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(a[1].run).toHaveBeenCalledTimes(1);
    expect(a[0].run).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("Escape closes and an empty result shows a message", () => {
    const onClose = vi.fn();
    render(<CommandPalette actions={mk()} onClose={onClose} />);
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "zzz" } });
    expect(screen.getByText("Ningún comando coincide.")).toBeTruthy();
    fireEvent.keyDown(screen.getByRole("combobox"), { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });
});
