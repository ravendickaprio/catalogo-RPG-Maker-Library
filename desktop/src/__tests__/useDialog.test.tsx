import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { useDialog } from "../hooks/useDialog";

function Dlg({ label, onEsc }: { label: string; onEsc: () => void }) {
  const { ref, onKeyDown } = useDialog(onEsc);
  return (
    <div ref={ref} role="dialog" aria-modal="true" aria-label={label} tabIndex={-1} onKeyDown={onKeyDown}>
      <button>{label}-btn</button>
    </div>
  );
}

describe("useDialog", () => {
  it("Escape with focus on body closes the dialog once", () => {
    const esc = vi.fn();
    render(<Dlg label="a" onEsc={esc} />);
    (document.activeElement as HTMLElement).blur();
    fireEvent.keyDown(document.body, { key: "Escape" });
    expect(esc).toHaveBeenCalledTimes(1);
  });
  it("does not double-fire when focus is inside", () => {
    const esc = vi.fn();
    render(<Dlg label="a" onEsc={esc} />);
    fireEvent.keyDown(screen.getByText("a-btn"), { key: "Escape" });
    expect(esc).toHaveBeenCalledTimes(1);
  });
  it("only the topmost dialog reacts to a body Escape", () => {
    const a = vi.fn();
    const b = vi.fn();
    render(
      <>
        <Dlg label="a" onEsc={a} />
        <Dlg label="b" onEsc={b} />
      </>,
    );
    (document.activeElement as HTMLElement).blur();
    fireEvent.keyDown(document.body, { key: "Escape" });
    expect(b).toHaveBeenCalledTimes(1);
    expect(a).not.toHaveBeenCalled();
  });
  it("returns focus to the dialog when it falls to body", async () => {
    render(<Dlg label="a" onEsc={() => undefined} />);
    const btn = screen.getByText("a-btn");
    btn.focus();
    btn.blur();
    await new Promise((r) => setTimeout(r, 10));
    expect(document.activeElement).toBe(screen.getByRole("dialog"));
  });
});
