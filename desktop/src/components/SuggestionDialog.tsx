import { useCallback, useEffect, useRef, useState } from "react";
import { suggestGame, normalizeError } from "../api/client";
import type { AppError, GameDetail, Suggestion } from "../api/types";
import CoverImage from "./CoverImage";
import { RequestSequencer } from "../lib/requestSequencer";
import { GameFields } from "./GameDetail";

interface Props {
  onClose: () => void;
  onShowInList: (game: GameDetail) => void;
}

const FOCUSABLE = 'button:not([disabled]), [href], input, select, textarea, [tabindex]:not([tabindex="-1"])';

export default function SuggestionDialog({ onClose, onShowInList }: Props) {
  const [result, setResult] = useState<Suggestion | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [loading, setLoading] = useState(true);
  const dialogRef = useRef<HTMLDivElement>(null);
  const seq = useRef(new RequestSequencer());
  const lastId = useRef<number | null>(null);
  const trigger = useRef<HTMLElement | null>(
    typeof document !== "undefined" ? (document.activeElement as HTMLElement | null) : null,
  );

  const load = useCallback(() => {
    const token = seq.current.next();
    setLoading(true);
    setError(null);
    suggestGame({ excludeId: lastId.current }).then(
      (s) => {
        if (!seq.current.isLatest(token)) return;
        lastId.current = s.game ? s.game.id : null;
        setResult(s);
        setLoading(false);
      },
      (e) => {
        if (!seq.current.isLatest(token)) return;
        setError(normalizeError(e));
        setLoading(false);
      },
    );
  }, []);

  useEffect(() => {
    load();
    const t = trigger.current;
    dialogRef.current?.focus();
    return () => {
      seq.current.invalidate();
      t?.focus?.();
    };
  }, [load]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      onClose();
    } else if (e.key === "Tab" && dialogRef.current) {
      const els = Array.from(dialogRef.current.querySelectorAll<HTMLElement>(FOCUSABLE));
      if (!els.length) return;
      const first = els[0];
      const last = els[els.length - 1];
      const active = document.activeElement;
      if (e.shiftKey && (active === first || active === dialogRef.current)) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && active === last) {
        e.preventDefault();
        first.focus();
      }
    }
  };

  const game = result?.game ?? null;

  return (
    <div
      className="backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={dialogRef}
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="suggest-title"
        tabIndex={-1}
        onKeyDown={onKeyDown}
      >
        <h2 id="suggest-title">Juego sugerido del día</h2>
        {loading && !result && <p className="empty">Buscando una sugerencia…</p>}
        {error && (
          <p className="error-text" role="alert">
            No se pudo obtener la sugerencia: {error.message}
          </p>
        )}
        {result && !game && (
          <p className="empty" role="status">
            No hay juegos pendientes: todos los juegos están terminados o la biblioteca está vacía.
          </p>
        )}
        {result && game && (
          <div className={"suggest-body" + (loading ? " dim" : "")}>
            <div className="cover-box">
              <CoverImage id={game.id} hasCover={game.hasCover} alt={`Portada de ${game.titulo}`} />
            </div>
            <div className="suggest-info">
              <h3 className="suggest-game-title">{game.titulo}</h3>
              <GameFields game={game} />
              {game.tags.length > 0 && (
                <ul className="tags" aria-label="Etiquetas">
                  {game.tags.map((t) => (
                    <li key={t}>{t}</li>
                  ))}
                </ul>
              )}
              <p className="description scroll">{game.descripcion || "Sin descripción."}</p>
            </div>
          </div>
        )}
        {result && (
          <p className="cell-muted">
            Elegido entre {result.eligibleCount} juegos no terminados de {result.totalCount}
          </p>
        )}
        <p className="note">La sugerencia no modifica tu biblioteca.</p>
        <div className="dialog-actions">
          <button type="button" onClick={load} disabled={loading}>
            Otra sugerencia
          </button>
          <button type="button" disabled={!game} onClick={() => game && onShowInList(game)}>
            Ver en la lista
          </button>
          <button type="button" className="primary" onClick={onClose}>
            Cerrar
          </button>
        </div>
      </div>
    </div>
  );
}
