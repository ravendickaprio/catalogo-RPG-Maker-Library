import { forwardRef, useEffect, useState } from "react";
import { getGame, normalizeError } from "../api/client";
import type { AppError, GameDetail as Detail } from "../api/types";
import { useCover } from "../hooks/useCover";
import { estadoClass } from "../lib/estado";

interface Props {
  id: number | null;
}

const GameDetail = forwardRef<HTMLElement, Props>(function GameDetail({ id }, ref) {
  const [game, setGame] = useState<Detail | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (id == null) {
      setGame(null);
      setError(null);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    getGame(id).then(
      (g) => {
        if (cancelled) return;
        setGame(g);
        setLoading(false);
      },
      (e) => {
        if (cancelled) return;
        setGame(null);
        setError(normalizeError(e));
        setLoading(false);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [id]);

  const shown = game && game.id === id ? game : null;
  const cover = useCover(shown ? shown.id : null, !!shown?.hasCover);

  return (
    <aside className="detail-pane" ref={ref} tabIndex={-1} aria-label="Detalle del juego">
      {id == null && <p className="empty">Selecciona un juego para ver su detalle.</p>}
      {id != null && loading && <p className="empty">Cargando detalle…</p>}
      {error && (
        <p className="error-text" role="alert">
          No se pudo cargar el detalle: {error.message}
        </p>
      )}
      {shown && !loading && (
        <>
          <div className="cover-box">
            {cover.src ? <img src={cover.src} alt={`Portada de ${shown.titulo}`} /> : <div className="cover-placeholder">{cover.loading ? "Cargando portada…" : "Sin portada"}</div>}
          </div>
          <h2>{shown.titulo}</h2>
          <GameFields game={shown} />
          {shown.tags.length > 0 && (
            <ul className="tags" aria-label="Etiquetas">
              {shown.tags.map((t) => (
                <li key={t}>{t}</li>
              ))}
            </ul>
          )}
          <h3>Descripción</h3>
          <p className="description">{shown.descripcion || "Sin descripción."}</p>
          <h3>Ruta</h3>
          <p className="path-text">{shown.ruta || "—"}</p>
        </>
      )}
    </aside>
  );
});

export function GameFields({ game }: { game: Detail }) {
  return (
    <dl className="fields">
      <dt>Código</dt>
      <dd>{game.codigo || "—"}</dd>
      <dt>Estado</dt>
      <dd>
        <span className={estadoClass(game.estado)}>{game.estado || "—"}</span>
      </dd>
      <dt>Idioma</dt>
      <dd>{game.idioma || "—"}</dd>
      <dt>Tipo</dt>
      <dd>{game.tipo || "—"}</dd>
      <dt>Página</dt>
      <dd>{game.pagina || "—"}</dd>
      <dt>Puntuación</dt>
      <dd>{game.puntuacion}</dd>
    </dl>
  );
}

export default GameDetail;
