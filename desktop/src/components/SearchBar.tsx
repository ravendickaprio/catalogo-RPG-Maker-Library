import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import type { SortKey } from "../api/types";

export const SORT_OPTIONS: { value: SortKey; label: string }[] = [
  { value: "title_asc", label: "Nombre A-Z" },
  { value: "title_desc", label: "Nombre Z-A" },
  { value: "code", label: "Código" },
  { value: "status", label: "Estado" },
  { value: "language", label: "Idioma" },
  { value: "recent", label: "Más recientes" },
];

interface Props {
  query: string;
  /** Active sidebar filters (only used to enable "Limpiar"). */
  estado: string;
  idioma: string;
  tag: string;
  sort: SortKey;
  onQuery: (v: string) => void;
  onClear: () => void;
  onSort: (v: SortKey) => void;
  onSuggest: () => void;
}

const SearchBar = forwardRef<HTMLInputElement, Props>(function SearchBar(p, ref) {
  const inputRef = useRef<HTMLInputElement>(null);
  useImperativeHandle(ref, () => inputRef.current as HTMLInputElement);
  const hasFilters = !!(p.query || p.estado || p.idioma || p.tag);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      const typing = !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT");
      if (e.key === "/" && !typing && !e.ctrlKey && !e.metaKey && !e.altKey) {
        e.preventDefault();
        const el = inputRef.current;
        el?.focus();
        el?.select();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="searchbar" role="search">
      <input
        ref={inputRef}
        type="search"
        className="search-input"
        aria-label="Buscar"
        placeholder="Buscar por título, código, etiqueta…  (/)"
        value={p.query}
        maxLength={200}
        onChange={(e) => p.onQuery(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape" && p.query) {
            e.preventDefault();
            p.onQuery("");
          }
        }}
      />
      <select aria-label="Ordenar por" value={p.sort} onChange={(e) => p.onSort(e.target.value as SortKey)}>
        {SORT_OPTIONS.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      <button type="button" onClick={p.onClear} disabled={!hasFilters}>
        Limpiar
      </button>
      <button type="button" className="btn-suggest" onClick={p.onSuggest}>
        🎲 Juego sugerido del día
      </button>
    </div>
  );
});

export default SearchBar;
