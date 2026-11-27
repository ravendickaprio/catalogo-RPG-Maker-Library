import { useEffect, useMemo, useRef, useState } from "react";
import { filterActions, type PaletteAction } from "../lib/palette";

interface Props {
  actions: PaletteAction[];
  onClose: () => void;
}

export default function CommandPalette({ actions, onClose }: Props) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const trigger = useRef<HTMLElement | null>(
    typeof document !== "undefined" ? (document.activeElement as HTMLElement | null) : null,
  );
  const shown = useMemo(() => filterActions(actions, query), [actions, query]);
  const idx = Math.min(active, Math.max(0, shown.length - 1));

  useEffect(() => {
    inputRef.current?.focus();
    const t = trigger.current;
    return () => t?.focus?.();
  }, []);

  useEffect(() => {
    listRef.current?.querySelector(`#palette-opt-${idx}`)?.scrollIntoView?.({ block: "nearest" });
  }, [idx, shown]);

  const run = (a: PaletteAction | undefined) => {
    if (!a) return;
    onClose();
    a.run();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive(Math.min(shown.length - 1, idx + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive(Math.max(0, idx - 1));
    } else if (e.key === "Enter") {
      e.preventDefault();
      run(shown[idx]);
    } else if (e.key === "Tab") {
      e.preventDefault(); // focus stays in the single input
    }
  };

  return (
    <div
      className="backdrop palette-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="dialog palette" role="dialog" aria-modal="true" aria-label="Paleta de comandos" onKeyDown={onKeyDown}>
        <input
          ref={inputRef}
          type="text"
          className="palette-input"
          role="combobox"
          aria-expanded="true"
          aria-controls="palette-list"
          aria-autocomplete="list"
          aria-label="Escribe un comando"
          placeholder="Escribe un comando…"
          value={query}
          aria-activedescendant={shown.length ? `palette-opt-${idx}` : undefined}
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
          }}
        />
        <ul id="palette-list" ref={listRef} role="listbox" aria-label="Comandos" className="palette-list">
          {shown.map((a, i) => (
            <li
              key={a.id}
              id={`palette-opt-${i}`}
              role="option"
              aria-selected={i === idx}
              className={"palette-opt" + (i === idx ? " active" : "")}
              onMouseMove={() => setActive(i)}
              onClick={() => run(a)}
            >
              {a.label}
            </li>
          ))}
        </ul>
        {shown.length === 0 && (
          <p className="empty" role="status">
            Ningún comando coincide.
          </p>
        )}
      </div>
    </div>
  );
}
