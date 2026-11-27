import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { GameSummary } from "../api/types";
import { useCover } from "../hooks/useCover";
import { estadoClass } from "../lib/estado";
import { CARD_INFO_H, computeGridLayout, GRID_GAP, GRID_PAD, moveInGrid } from "../lib/gridLayout";
import StatusLine from "./StatusLine";

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
  focusTick?: number;
}

function Card({ game, width, selected, onSelect, onOpen }: {
  game: GameSummary;
  width: number;
  selected: boolean;
  onSelect: () => void;
  onOpen: () => void;
}) {
  // Rendered only for virtual-visible rows, so covers are fetched lazily and only if present.
  const cover = useCover(game.id, game.hasCover);
  const coverH = Math.round((width * 4) / 3);
  return (
    <div
      id={`card-${game.id}`}
      role="option"
      aria-selected={selected}
      className={"card" + (selected ? " selected" : "")}
      style={{ width, height: coverH + CARD_INFO_H }}
      onClick={onSelect}
      onDoubleClick={onOpen}
    >
      <div className="card-cover" style={{ height: coverH }}>
        {cover.src ? (
          <img src={cover.src} alt="" draggable={false} />
        ) : (
          <span className="card-ph" aria-hidden="true">
            {cover.loading ? "…" : game.hasCover ? "" : "Sin portada"}
          </span>
        )}
      </div>
      <div className="card-info">
        <span className="card-title" title={game.titulo}>
          {game.titulo}
        </span>
        <span className="card-meta">
          <span className={estadoClass(game.estado)}>{game.estado || "—"}</span>
          <span className="cell-code">{game.codigo}</span>
        </span>
      </div>
    </div>
  );
}

export default function GameGallery({
  items, total, loading, elapsedMs, strategy, selectedId, onSelect, onOpen, loadMore, focusTick = 0,
}: Props) {
  const parentRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(800);

  useLayoutEffect(() => {
    const el = parentRef.current;
    if (!el) return;
    if (el.clientWidth > 0) setWidth(el.clientWidth);
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver((entries) => {
      const w = entries[0]?.contentRect.width;
      if (w && w > 0) setWidth(w);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const layout = computeGridLayout(width);
  const rowCount = Math.ceil(items.length / layout.columns);
  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => parentRef.current,
    estimateSize: () => layout.rowHeight,
    overscan: 2,
  });
  useEffect(() => {
    virtualizer.measure();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [layout.rowHeight, layout.columns]);

  const vRows = virtualizer.getVirtualItems();
  const lastRow = vRows.length ? vRows[vRows.length - 1].index : -1;
  useEffect(() => {
    if (lastRow >= rowCount - 3 && items.length < total) loadMore();
  }, [lastRow, rowCount, items.length, total, loadMore]);

  const selectedIndex = selectedId == null ? -1 : items.findIndex((g) => g.id === selectedId);
  useEffect(() => {
    if (selectedIndex >= 0) virtualizer.scrollToIndex(Math.floor(selectedIndex / layout.columns), { align: "auto" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId, selectedIndex, layout.columns]);

  const handledFocus = useRef(0);
  useEffect(() => {
    if (focusTick === handledFocus.current || loading || selectedIndex < 0) return;
    handledFocus.current = focusTick;
    const t = setTimeout(() => parentRef.current?.focus(), 0);
    return () => clearTimeout(t);
  }, [focusTick, loading, selectedIndex]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (!items.length) return;
    if (e.key === "Enter") {
      if (selectedIndex >= 0) {
        e.preventDefault();
        onOpen(items[selectedIndex].id);
      }
      return;
    }
    const next = moveInGrid(e.key, selectedIndex, layout.columns, items.length);
    if (next != null) {
      e.preventDefault();
      onSelect(items[next].id);
    }
  };

  return (
    <section className="list-pane" aria-label="Galería de juegos">
      <div
        ref={parentRef}
        className="list-scroll"
        role="listbox"
        aria-label="Juegos (galería)"
        aria-busy={loading}
        aria-activedescendant={selectedIndex >= 0 ? `card-${selectedId}` : undefined}
        tabIndex={0}
        onKeyDown={onKeyDown}
      >
        {!loading && items.length === 0 && (
          <p className="empty">No hay resultados. Prueba con otros términos o pulsa «Limpiar» para quitar los filtros.</p>
        )}
        {loading && items.length === 0 && (
          <div className="skeleton-grid" aria-hidden="true">
            {Array.from({ length: 12 }, (_, i) => (
              <div key={i} className="skeleton-card" />
            ))}
          </div>
        )}
        <div style={{ height: virtualizer.getTotalSize(), position: "relative", width: "100%" }}>
          {vRows.map((v) => {
            const start = v.index * layout.columns;
            const rowItems = items.slice(start, start + layout.columns);
            return (
              <div
                key={v.key}
                role="presentation"
                className="grid-row"
                style={{
                  position: "absolute", top: 0, left: 0, width: "100%", height: layout.rowHeight,
                  transform: `translateY(${v.start}px)`, padding: `0 ${GRID_PAD}px`, gap: GRID_GAP,
                }}
              >
                {rowItems.map((g) => (
                  <Card
                    key={g.id}
                    game={g}
                    width={layout.cardWidth}
                    selected={g.id === selectedId}
                    onSelect={() => onSelect(g.id)}
                    onOpen={() => onOpen(g.id)}
                  />
                ))}
              </div>
            );
          })}
        </div>
      </div>
      <StatusLine loading={loading} total={total} count={items.length} elapsedMs={elapsedMs} strategy={strategy} />
    </section>
  );
}
