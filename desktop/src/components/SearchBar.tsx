import { useEffect, useRef } from "react";
import type { LibraryInfo, SortKey } from "../api/types";

export const SORT_OPTIONS: { value: SortKey; label: string }[] = [
  { value: "title_asc", label: "Nombre A-Z" },
  { value: "title_desc", label: "Nombre Z-A" },
  { value: "code", label: "Código" },
  { value: "status", label: "Estado" },
  { value: "language", label: "Idioma" },
  { value: "recent", label: "Más recientes" },
];

interface Props {
  info: LibraryInfo | null;
  query: string;
  estado: string;
  idioma: string;
  sort: SortKey;
  onQuery: (v: string) => void;
  onEstado: (v: string) => void;
  onIdioma: (v: string) => void;
  onSort: (v: SortKey) => void;
  onSuggest: () => void;
}

export default function SearchBar(p: Props) {
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      const typing = !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT");
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        inputRef.current?.focus();
        inputRef.current?.select();
      } else if (e.key === "/" && !typing && !e.ctrlKey && !e.metaKey && !e.altKey) {
        e.preventDefault();
        inputRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="searchbar" role="search">
      <input
        ref={inputRef}
        type="search"
        className="search-input"
        aria-label="Buscar"
        placeholder="Buscar por título, código, etiqueta…  (Ctrl+K o /)"
        value={p.query}
        maxLength={200}
        onChange={(e) => p.onQuery(e.target.value)}
      />
      <select aria-label="Estado" value={p.estado} onChange={(e) => p.onEstado(e.target.value)}>
        <option value="">Todos los estados</option>
        {p.info?.estados.map((s) => (
          <option key={s.value} value={s.value}>
            {s.value || "(sin estado)"} ({s.count})
          </option>
        ))}
      </select>
      <select aria-label="Idioma" value={p.idioma} onChange={(e) => p.onIdioma(e.target.value)}>
        <option value="">Todos los idiomas</option>
        {p.info?.idiomas.map((s) => (
          <option key={s.value} value={s.value}>
            {s.value || "(sin idioma)"} ({s.count})
          </option>
        ))}
      </select>
      <select aria-label="Ordenar por" value={p.sort} onChange={(e) => p.onSort(e.target.value as SortKey)}>
        {SORT_OPTIONS.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      <button type="button" className="btn-suggest" onClick={p.onSuggest}>
        🎲 Juego sugerido del día
      </button>
    </div>
  );
}
