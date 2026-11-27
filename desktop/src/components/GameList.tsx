import { useEffect, useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { GameSummary } from "../api/types";
import { estadoClass } from "../lib/estado";

const ROW_H = 44;

interface Props {
  items: GameSummary[];
  total: number;
  loading: boolean;
  elapsedMs: number | null;
  strategy: string | null;
  selectedId: number | null;
  onSelect: (id: number) => void;
  onOpen: (id: number) => void;
  loadMore: () => void;
}

export default function GameList({
  items,
  total,
  loading,
  elapsedMs,
  strategy,
  selectedId,
  onSelect,
  onOpen,
  loadMore,
}: Props) {
  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_H,
    overscan: 10,
  });
  const vItems = virtualizer.getVirtualItems();
  const lastIndex = vItems.length ? vItems[vItems.length - 1].index : -1;

  useEffect(() => {
    if (lastIndex >= items.length - 20 && items.length < total) loadMore();
  }, [lastIndex, items.length, total, loadMore]);

  const selectedIndex = selectedId == null ? -1 : items.findIndex((g) => g.id === selectedId);

  useEffect(() => {
    if (selectedIndex >= 0) virtualizer.scrollToIndex(selectedIndex, { align: "auto" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (!items.length) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const d = e.key === "ArrowDown" ? 1 : -1;
      const next = Math.min(items.length - 1, Math.max(0, (selectedIndex < 0 ? (d > 0 ? -1 : items.length) : selectedIndex) + d));
      onSelect(items[next].id);
    } else if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      onSelect(items[e.key === "Home" ? 0 : items.length - 1].id);
    } else if (e.key === "Enter" && selectedIndex >= 0) {
      e.preventDefault();
      onOpen(items[selectedIndex].id);
    }
  };

  return (
    <section className="list-pane" aria-label="Lista de juegos">
      <div className="list-header" aria-hidden="true">
        <span>Código</span>
        <span>Título</span>
        <span>Estado</span>
        <span>Idioma</span>
        <span>Tipo</span>
        <span title="Portada">🖼</span>
      </div>
      <div
        ref={parentRef}
        className="list-scroll"
        role="listbox"
        aria-label="Juegos"
        aria-activedescendant={selectedIndex >= 0 ? `game-${selectedId}` : undefined}
        tabIndex={0}
        onKeyDown={onKeyDown}
      >
        {!loading && items.length === 0 && <p className="empty">No hay resultados.</p>}
        <div style={{ height: virtualizer.getTotalSize(), position: "relative", width: "100%" }}>
          {vItems.map((v) => {
            const g = items[v.index];
            if (!g) return null;
            const selected = g.id === selectedId;
            return (
              <div
                key={g.id}
                id={`game-${g.id}`}
                role="option"
                aria-selected={selected}
                aria-posinset={v.index + 1}
                aria-setsize={total}
                className={"row" + (selected ? " selected" : "")}
                style={{ position: "absolute", top: 0, left: 0, width: "100%", height: ROW_H, transform: `translateY(${v.start}px)` }}
                onClick={() => onSelect(g.id)}
                onDoubleClick={() => onOpen(g.id)}
              >
                <span className="cell-code">{g.codigo}</span>
                <span className="cell-title" title={g.titulo}>
                  {g.titulo}
                </span>
                <span>
                  <span className={estadoClass(g.estado)}>{g.estado || "—"}</span>
                </span>
                <span className="cell-muted">{g.idioma}</span>
                <span className="cell-muted">{g.tipo}</span>
                <span aria-label={g.hasCover ? "Con portada" : "Sin portada"} title={g.hasCover ? "Con portada" : "Sin portada"}>
                  {g.hasCover ? "●" : "○"}
                </span>
              </div>
            );
          })}
        </div>
      </div>
      <div className="statusline" role="status" aria-live="polite">
        {loading ? "Cargando…" : `Mostrando ${items.length} de ${total}`}
        {elapsedMs != null && !loading && (
          <span className="cell-muted">
            {" "}
            · {elapsedMs.toFixed(1)} ms · estrategia: {strategy}
          </span>
        )}
      </div>
    </section>
  );
}
