import { useEffect, useId, useRef, useState } from "react";

export interface ToolItem {
  id: string;
  label: string;
  title?: string;
  run: () => void;
}

/** "Herramientas" menu button: Enter/Space/ArrowDown opens, arrows move, Esc closes and returns focus. */
export default function ToolsMenu({ items }: { items: ToolItem[] }) {
  const uid = useId();
  const [open, setOpen] = useState(false);
  const btnRef = useRef<HTMLButtonElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const itemRefs = useRef<(HTMLButtonElement | null)[]>([]);

  useEffect(() => {
    if (!open) return;
    itemRefs.current[0]?.focus();
    const away = (e: MouseEvent) => {
      if (wrapRef.current && !wrapRef.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", away);
    return () => document.removeEventListener("mousedown", away);
  }, [open]);

  const close = (refocus: boolean) => {
    setOpen(false);
    if (refocus) btnRef.current?.focus();
  };
  const move = (from: number, d: number) => itemRefs.current[(from + d + items.length) % items.length]?.focus();

  return (
    <div className="tools-menu" ref={wrapRef}>
      <button
        ref={btnRef}
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? `${uid}-m` : undefined}
        onClick={() => setOpen((o) => !o)}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" && !open) {
            e.preventDefault();
            setOpen(true);
          }
        }}
      >
        Herramientas ▾
      </button>
      {open && (
        <div id={`${uid}-m`} role="menu" aria-label="Herramientas" className="tools-list">
          {items.map((it, i) => (
            <button
              key={it.id}
              ref={(el) => {
                itemRefs.current[i] = el;
              }}
              type="button"
              role="menuitem"
              title={it.title}
              onClick={() => {
                close(false);
                it.run();
              }}
              onKeyDown={(e) => {
                if (e.key === "ArrowDown") {
                  e.preventDefault();
                  move(i, 1);
                } else if (e.key === "ArrowUp") {
                  e.preventDefault();
                  move(i, -1);
                } else if (e.key === "Home") {
                  e.preventDefault();
                  move(0, 0);
                } else if (e.key === "End") {
                  e.preventDefault();
                  move(items.length - 1, 0);
                } else if (e.key === "Escape") {
                  e.preventDefault();
                  e.stopPropagation();
                  close(true);
                } else if (e.key === "Tab") {
                  close(false);
                }
              }}
            >
              {it.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
