import { useId } from "react";
import { addTags } from "../lib/gameInput";

interface Props {
  tags: string[];
  draft: string;
  suggestions: string[];
  invalid?: boolean;
  describedBy?: string;
  onChange: (tags: string[], draft: string) => void;
}

/** Chip editor: Enter/comma adds, Backspace on empty removes the last, × removes one. */
export default function TagEditor({ tags, draft, suggestions, invalid, describedBy, onChange }: Props) {
  const listId = useId();
  const commit = (raw: string) => onChange(addTags(tags, raw), "");
  return (
    <div className="tag-editor">
      <ul className="tags" aria-label="Etiquetas">
        {tags.map((t) => (
          <li key={t.toLowerCase()}>
            {t}{" "}
            <button type="button" className="chip-x" aria-label={`Quitar etiqueta ${t}`} onClick={() => onChange(tags.filter((x) => x !== t), draft)}>
              ×
            </button>
          </li>
        ))}
      </ul>
      <input
        type="text"
        aria-label="Añadir etiqueta"
        aria-invalid={invalid || undefined}
        aria-describedby={describedBy}
        list={listId}
        placeholder="Escribe y pulsa Enter o coma"
        value={draft}
        onChange={(e) => {
          const v = e.target.value;
          if (v.includes(",")) {
            const at = v.lastIndexOf(",");
            onChange(addTags(tags, v.slice(0, at)), v.slice(at + 1).replace(/^\s+/, ""));
          } else onChange(tags, v);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            e.stopPropagation();
            if (draft.trim()) commit(draft);
          } else if (e.key === "Backspace" && draft === "" && tags.length) {
            onChange(tags.slice(0, -1), "");
          }
        }}
        onBlur={() => {
          if (draft.trim()) commit(draft);
        }}
      />
      <datalist id={listId}>
        {suggestions.map((s) => (
          <option key={s} value={s} />
        ))}
      </datalist>
    </div>
  );
}
