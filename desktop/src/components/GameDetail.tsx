import { forwardRef, useCallback, useEffect, useState } from "react";
import { createGame, deleteGame, getGame, normalizeError, setFavorite, updateGame } from "../api/client";
import type { AppError, GameDetail as Detail, GameInput, Vocabulary } from "../api/types";
import CoverImage from "./CoverImage";
import { estadoClass } from "../lib/estado";
import ConfirmDialog from "./ConfirmDialog";
import GameForm from "./GameForm";
import { isFavorite } from "../lib/favorites";

export interface DetailCommand {
  kind: "edit" | "delete" | "favorite";
  n: number;
}

interface Props {
  id: number | null;
  /** Show the empty "new game" form instead of a record. */
  creating?: boolean;
  readOnly?: boolean;
  vocab?: Vocabulary | null;
  tagSuggestions?: string[];
  /** Changes when the library was modified externally: the detail is fetched again. */
  reloadToken?: number;
  /** Palette-driven commands (edit/delete the shown game). */
  command?: DetailCommand | null;
  /** Called when the selected game no longer exists (NotFound). */
  onMissing?: () => void;
  onDirtyChange?: (dirty: boolean) => void;
  onSaved?: (game: Detail, created: boolean) => void;
  /** The cover was replaced/removed (saved immediately, independent of the text fields). */
  onCoverChanged?: (game: Detail) => void;
  onDeleted?: (id: number) => void;
  /** The favorite tag was added/removed (saved immediately). */
  onFavoriteChanged?: (game: Detail, favorite: boolean) => void;
  onCancelCreate?: () => void;
  onToast?: (msg: string) => void;
}

const GameDetail = forwardRef<HTMLElement, Props>(function GameDetail(
  {
    id,
    creating = false,
    readOnly = false,
    vocab = null,
    tagSuggestions = [],
    reloadToken = 0,
    command = null,
    onMissing,
    onDirtyChange,
    onSaved,
    onCoverChanged,
    onDeleted,
    onFavoriteChanged,
    onCancelCreate,
    onToast,
  },
  ref,
) {
  const [game, setGame] = useState<Detail | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [loading, setLoading] = useState(false);
  const [editing, setEditing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [formKey, setFormKey] = useState(0);
  const [formError, setFormError] = useState<string | null>(null);
  const [conflict, setConflict] = useState(false);
  const [dupMessage, setDupMessage] = useState<{ input: GameInput; message: string } | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  useEffect(() => {
    setEditing(false);
    setConflict(false);
    setFormError(null);
    setDupMessage(null);
    setConfirmDelete(false);
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
        const err = normalizeError(e);
        if (err.kind === "NotFound" && onMissing) {
          onMissing();
          return;
        }
        setError(err);
        setLoading(false);
      },
    );
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  // External refresh: refetch silently, but never replace what the user is editing.
  useEffect(() => {
    if (id == null || reloadToken === 0) return;
    let cancelled = false;
    getGame(id).then(
      (g) => {
        if (!cancelled && !editing) setGame(g);
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [reloadToken]);

  const shown = game && game.id === id ? game : null;
  const canWrite = !readOnly;

  const startEdit = useCallback(() => {
    setFormError(null);
    setConflict(false);
    setEditing(true);
  }, []);

  useEffect(() => {
    if (!command || creating || !shown) return;
    if (command.kind === "favorite") {
      if (!canWrite) onToast?.("Modo solo lectura: no se puede modificar la biblioteca");
      else if (editing) onToast?.("Termina o cancela la edición antes de cambiar los favoritos");
      else void toggleFavorite();
    } else if (command.kind === "edit" && canWrite) startEdit();
    else if (command.kind === "delete" && canWrite) setConfirmDelete(true);
    else if (!canWrite) onToast?.("Modo solo lectura: no se puede modificar la biblioteca");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [command?.n]);

  const handleError = (e: unknown, input?: GameInput) => {
    const err = normalizeError(e);
    if (err.kind === "DuplicateCode" && input) setDupMessage({ input, message: err.message });
    else if (err.kind === "Conflict") setConflict(true);
    else if (err.kind === "Busy") onToast?.("La base está ocupada por otra aplicación; inténtalo de nuevo");
    else if (err.kind === "ReadOnly") onToast?.("La base está en modo solo lectura: no se guardó nada");
    else if (err.kind === "NotFound" && game) setFormError("El juego ya no existe en la base de datos.");
    else setFormError(err.message);
  };

  const save = async (input: GameInput, allowDuplicateCode = false) => {
    setSaving(true);
    setFormError(null);
    setDupMessage(null);
    try {
      if (creating) {
        const g = await createGame(input, { allowDuplicateCode });
        onSaved?.(g, true);
      } else if (game) {
        const g = await updateGame(game.id, input, { allowDuplicateCode, expectedRevision: game.revision });
        setGame(g);
        setEditing(false);
        onSaved?.(g, false);
      }
    } catch (e) {
      handleError(e, input);
    } finally {
      setSaving(false);
    }
  };

  const reloadFresh = async () => {
    if (!game) return;
    try {
      const g = await getGame(game.id);
      setGame(g);
      setConflict(false);
      setFormKey((k) => k + 1); // remounts the form with the fresh record: edits are discarded
    } catch (e) {
      const err = normalizeError(e);
      if (err.kind === "NotFound") {
        onMissing?.();
        onToast?.("El juego fue eliminado por otra aplicación");
      } else setFormError(err.message);
    }
  };

  /** Flips the Favorito link only (`set_favorite`, checked against the revision shown; other fields untouched). */
  const toggleFavorite = async () => {
    if (!game || saving || readOnly) return;
    setSaving(true);
    setFormError(null);
    try {
      const favorite = !isFavorite(game.tags);
      const g = await setFavorite(game.id, favorite, game.revision);
      setGame(g);
      onFavoriteChanged?.(g, favorite);
    } catch (e) {
      const err = normalizeError(e);
      if (err.kind === "Conflict") {
        onToast?.("El juego cambió en otra aplicación; se recargó. Inténtalo de nuevo.");
        try {
          setGame(await getGame(game.id));
        } catch {
          /* ignored: the next refresh will show the real state */
        }
      } else if (err.kind === "NotFound") {
        onMissing?.();
        onToast?.("El juego ya no existe");
      } else if (err.kind === "Busy") onToast?.("La base está ocupada por otra aplicación; inténtalo de nuevo");
      else onToast?.(err.message);
    } finally {
      setSaving(false);
    }
  };

  const doDelete = async () => {
    if (!game) return;
    setSaving(true);
    try {
      await deleteGame(game.id, { expectedRevision: game.revision });
      setConfirmDelete(false);
      onDeleted?.(game.id);
    } catch (e) {
      setConfirmDelete(false);
      handleError(e);
    } finally {
      setSaving(false);
    }
  };

  const showForm = creating || (editing && !!shown);

  return (
    <aside className="detail-pane" ref={ref} tabIndex={-1} aria-label="Detalle del juego">
      {!creating && id == null && <p className="empty">Selecciona un juego para ver su detalle.</p>}
      {!creating && id != null && loading && <p className="empty">Cargando detalle…</p>}
      {!creating && error && (
        <p className="error-text" role="alert">
          No se pudo cargar el detalle: {error.message}
        </p>
      )}
      {conflict && (
        <div className="banner error" role="alert">
          <p>Este juego cambió en otra aplicación.</p>
          <div className="dialog-actions">
            <button type="button" onClick={reloadFresh}>
              Recargar
            </button>
            <button type="button" onClick={() => setConflict(false)}>
              Seguir editando
            </button>
          </div>
        </div>
      )}
      {formError && (
        <p className="error-text" role="alert">
          {formError}
        </p>
      )}
      {showForm && (
        <GameForm
          key={`${creating ? "new" : (shown?.id ?? 0)}-${formKey}`}
          game={creating ? null : shown}
          vocab={vocab}
          tagSuggestions={tagSuggestions}
          saving={saving}
          onCoverChanged={(g) => {
            setGame(g);
            onCoverChanged?.(g);
          }}
          onCoverError={(e) => handleError(e)}
          onSubmit={(input) => void save(input)}
          onDirtyChange={onDirtyChange}
          onCancel={() => {
            setConflict(false);
            setFormError(null);
            if (creating) onCancelCreate?.();
            else setEditing(false);
          }}
        />
      )}
      {!showForm && shown && !loading && (
        <>
          <div className="cover-box">
            <CoverImage id={shown.id} hasCover={shown.hasCover} alt={`Portada de ${shown.titulo}`} />
          </div>
          <h2>{shown.titulo}</h2>
          <div className="dialog-actions detail-actions">
            <button
              type="button"
              className="fav-btn"
              aria-pressed={isFavorite(shown.tags)}
              aria-label="Favorito"
              title={canWrite ? (isFavorite(shown.tags) ? "Quitar de favoritos" : "Marcar como favorito") : "Modo solo lectura: actívalo en Ajustes para editar"}
              disabled={!canWrite || saving}
              onClick={() => void toggleFavorite()}
            >
              {isFavorite(shown.tags) ? "★" : "☆"}
            </button>
            <button type="button" onClick={startEdit} disabled={!canWrite || saving} title={canWrite ? "Editar este juego" : "Modo solo lectura: actívalo en Ajustes para editar"}>
              Editar
            </button>
            <button type="button" className="danger" onClick={() => setConfirmDelete(true)} disabled={!canWrite} title={canWrite ? "Eliminar este juego" : "Modo solo lectura: actívalo en Ajustes para eliminar"}>
              Eliminar
            </button>
          </div>
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
      {dupMessage && (
        <ConfirmDialog
          title="Código duplicado"
          confirmLabel="Guardar de todos modos"
          busy={saving}
          onCancel={() => setDupMessage(null)}
          onConfirm={() => void save(dupMessage.input, true)}
        >
          <p>{dupMessage.message}</p>
          <p>¿Guardar de todos modos?</p>
        </ConfirmDialog>
      )}
      {confirmDelete && shown && (
        <ConfirmDialog title={`¿Eliminar «${shown.titulo}»?`} confirmLabel="Eliminar" danger busy={saving} onCancel={() => setConfirmDelete(false)} onConfirm={() => void doDelete()}>
          <p>Esta acción no se puede deshacer desde la app (se creó un backup automático al inicio de la sesión).</p>
        </ConfirmDialog>
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
