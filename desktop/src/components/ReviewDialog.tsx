import { useCallback, useEffect, useId, useRef, useState } from "react";
import { getGame, normalizeError, updateGame } from "../api/client";
import type { CheckId, GameDetail, GameInput, Issue, Vocabulary } from "../api/types";
import { useDialog } from "../hooks/useDialog";
import { loadAllIssueIds } from "../lib/issues";
import { takeRandom } from "../lib/shuffle";
import ConfirmDialog from "./ConfirmDialog";
import CoverImage from "./CoverImage";
import { GameFields } from "./GameDetail";
import GameForm from "./GameForm";

export type ReviewCriterion = Extract<CheckId, "sin_descripcion" | "sin_portada" | "sin_tags" | "titulo_ry">;

export const REVIEW_CRITERIA: { value: ReviewCriterion; label: string }[] = [
  { value: "sin_descripcion", label: "Sin descripción" },
  { value: "sin_portada", label: "Sin portada" },
  { value: "sin_tags", label: "Sin tags" },
  { value: "titulo_ry", label: "Título con prefijo RY" },
];
export const DEFAULT_BATCH_SIZE = 20;
const MAX_BATCH_SIZE = 200;

interface Props {
  vocab: Vocabulary | null;
  readOnly: boolean;
  tagSuggestions: string[];
  onClose: () => void;
  /** Data changed (a save or a cover change): the host refreshes counts, list and detail. */
  onChanged: (id?: number) => void;
  /** Criterion preselected when the dialog opens (default: sin_descripcion). */
  initialCriterion?: ReviewCriterion;
}

/** All ids matching a detector, then a random sample of `size`. */
async function loadBatch(check: ReviewCriterion, size: number): Promise<{ batch: Issue[]; pending: number }> {
  const all = await loadAllIssueIds(check);
  return { batch: takeRandom(all, size), pending: all.length };
}

/** Batch review (ADR-017): the in-app equivalent of the Python editor: random batch, edit every field, save and next. */
export default function ReviewDialog({ vocab, readOnly, tagSuggestions, onClose, onChanged, initialCriterion = "sin_descripcion" }: Props) {
  const uid = useId();
  const [criterion, setCriterion] = useState<ReviewCriterion>(initialCriterion);
  const [sizeText, setSizeText] = useState(String(DEFAULT_BATCH_SIZE));
  const [batch, setBatch] = useState<Issue[] | null>(null);
  const [pending, setPending] = useState(0);
  const [idx, setIdx] = useState(0);
  const [saved, setSaved] = useState<Set<number>>(() => new Set());
  const [game, setGame] = useState<GameDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [conflict, setConflict] = useState(false);
  const [saving, setSaving] = useState(false);
  const [formKey, setFormKey] = useState(0);
  const [dup, setDup] = useState<{ input: GameInput; message: string } | null>(null);
  const [confirm, setConfirm] = useState<(() => void) | null>(null);
  const dirtyRef = useRef(false);
  const busyRef = useRef(false);
  const loadSeq = useRef(0);

  /** Runs `action` now, or after the user confirms discarding the unsaved edits. */
  const guardNav = useCallback((action: () => void) => {
    if (dirtyRef.current) setConfirm(() => action);
    else action();
  }, []);
  const { ref, onKeyDown } = useDialog(() => guardNav(onClose), "select");

  const size = Math.min(MAX_BATCH_SIZE, Math.max(1, Math.floor(Number(sizeText)) || DEFAULT_BATCH_SIZE));

  const newBatch = useCallback(async (check: ReviewCriterion, n: number) => {
    const seq = ++loadSeq.current;
    setBatch(null);
    setError(null);
    setConflict(false);
    setIdx(0);
    setSaved(new Set());
    setGame(null);
    dirtyRef.current = false;
    try {
      const r = await loadBatch(check, n);
      if (seq !== loadSeq.current) return;
      setBatch(r.batch);
      setPending(r.pending);
    } catch (e) {
      if (seq !== loadSeq.current) return;
      setBatch([]);
      setPending(0);
      setError(normalizeError(e).message);
    }
  }, []);
  useEffect(() => {
    void newBatch(initialCriterion, DEFAULT_BATCH_SIZE);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [newBatch]);

  const current = batch?.[idx] ?? null;
  const currentId = current?.id ?? null;
  useEffect(() => {
    if (currentId == null) {
      setGame(null);
      return;
    }
    let dead = false;
    setGame(null);
    setFormKey((k) => k + 1);
    getGame(currentId).then(
      (g) => !dead && setGame(g),
      (e) => !dead && setError(normalizeError(e).message),
    );
    return () => {
      dead = true;
    };
  }, [currentId]);

  const go = (i: number) =>
    guardNav(() => {
      dirtyRef.current = false;
      setError(null);
      setConflict(false);
      setIdx(i);
    });
  const forward = () => {
    // Called by the form's own "Omitir" (it asks before discarding edits) or after a save.
    dirtyRef.current = false;
    setError(null);
    setConflict(false);
    setIdx((i) => i + 1);
  };

  const save = async (input: GameInput, allowDuplicateCode = false) => {
    if (!game || readOnly || busyRef.current) return;
    busyRef.current = true;
    setSaving(true);
    setError(null);
    setDup(null);
    try {
      await updateGame(game.id, input, { allowDuplicateCode, expectedRevision: game.revision });
      setSaved((s) => new Set(s).add(game.id));
      onChanged(game.id);
      forward();
    } catch (e) {
      const err = normalizeError(e);
      if (err.kind === "DuplicateCode") setDup({ input, message: err.message });
      else if (err.kind === "Conflict") setConflict(true);
      else setError(err.kind === "Busy" ? "La base está ocupada por otra aplicación; inténtalo de nuevo." : err.message);
    } finally {
      busyRef.current = false;
      setSaving(false);
    }
  };
  const reloadFresh = async () => {
    if (!game) return;
    try {
      setGame(await getGame(game.id));
      setConflict(false);
      setFormKey((k) => k + 1);
      dirtyRef.current = false;
    } catch (e) {
      setError(normalizeError(e).message);
    }
  };

  const total = batch?.length ?? 0;
  const finished = batch !== null && idx >= total;
  const nextLabel = readOnly ? "Siguiente" : "Omitir";

  return (
    <div className="backdrop" onMouseDown={(e) => e.target === e.currentTarget && guardNav(onClose)}>
      <div ref={ref} className="dialog review-dialog" role="dialog" aria-modal="true" aria-labelledby={`${uid}-t`} tabIndex={-1} onKeyDown={onKeyDown}>
        <h2 id={`${uid}-t`}>Revisión por tandas</h2>
        <div className="review-controls">
          <div className="form-row">
            <label htmlFor={`${uid}-c`}>Criterio</label>
            <select id={`${uid}-c`} value={criterion} onChange={(e) => setCriterion(e.target.value as ReviewCriterion)}>
              {REVIEW_CRITERIA.map((c) => (
                <option key={c.value} value={c.value}>
                  {c.label}
                </option>
              ))}
            </select>
          </div>
          <div className="form-row">
            <label htmlFor={`${uid}-n`}>Tamaño de la tanda</label>
            <input id={`${uid}-n`} type="number" min={1} max={MAX_BATCH_SIZE} value={sizeText} onChange={(e) => setSizeText(e.target.value)} />
          </div>
          <button type="button" onClick={() => guardNav(() => void newBatch(criterion, size))} disabled={saving || batch === null}>
            Nueva tanda
          </button>
          <button type="button" onClick={() => go(idx - 1)} disabled={saving || idx <= 0 || batch === null}>
            Anterior
          </button>
        </div>
        {readOnly && (
          <p className="note" role="note">
            Modo solo lectura: puedes revisar la tanda pero no guardar cambios.
          </p>
        )}
        {error && (
          <p className="error-text" role="alert">
            {error}
          </p>
        )}
        {conflict && (
          <div className="banner error" role="alert">
            <p>Este juego cambió en otra aplicación.</p>
            <div className="dialog-actions">
              <button type="button" onClick={() => guardNav(() => void reloadFresh())}>
                Recargar
              </button>
              <button type="button" onClick={() => setConflict(false)}>
                Seguir editando
              </button>
            </div>
          </div>
        )}
        {batch === null ? (
          <p role="status">Cargando tanda…</p>
        ) : total === 0 ? (
          <p role="status">No hay juegos que cumplan este criterio.</p>
        ) : (
          <>
            <p role="status" aria-live="polite" className="review-progress">
              {finished ? `Tanda terminada: ${saved.size} guardados de ${total}` : `${idx + 1} de ${total}`} · {pending} pendientes en total · guardados {saved.size}
            </p>
            <div className="review-body">
              <nav aria-label="Juegos de la tanda" className="review-list">
                <ul>
                  {batch.map((it, i) => (
                    <li key={it.id}>
                      <button type="button" className={"side-item" + (i === idx ? " active" : "")} aria-current={i === idx ? "true" : undefined} disabled={saving} onClick={() => i !== idx && go(i)}>
                        <span className="side-label">
                          {it.codigo || "—"} · {it.titulo}
                        </span>
                        {saved.has(it.id) && (
                          <span className="side-count" aria-label="guardado">
                            ✓
                          </span>
                        )}
                      </button>
                    </li>
                  ))}
                </ul>
              </nav>
              <section className="review-edit" aria-label="Juego en revisión">
                {finished ? (
                  <p>Has llegado al final de la tanda. Pulsa «Nueva tanda» para otra, o elige un juego de la lista.</p>
                ) : !game || game.id !== currentId ? (
                  !error && <p role="status">Cargando juego…</p>
                ) : readOnly ? (
                  <>
                    <div className="cover-box">
                      <CoverImage id={game.id} hasCover={game.hasCover} alt={`Portada de ${game.titulo}`} />
                    </div>
                    <h3>{game.titulo}</h3>
                    <GameFields game={game} />
                    <p className="description">{game.descripcion || "Sin descripción."}</p>
                    <div className="dialog-actions">
                      <button type="button" onClick={forward}>
                        {nextLabel}
                      </button>
                    </div>
                  </>
                ) : (
                  <GameForm
                    key={`${game.id}-${formKey}`}
                    game={game}
                    vocab={vocab}
                    tagSuggestions={tagSuggestions}
                    saving={saving}
                    submitLabel="Guardar y siguiente"
                    cancelLabel={nextLabel}
                    escCancels={false}
                    ctrlEnterSubmits
                    onSubmit={(input) => void save(input)}
                    onCancel={forward}
                    onDirtyChange={(d) => {
                      dirtyRef.current = d;
                    }}
                    onCoverChanged={(g) => {
                      setGame(g);
                      onChanged(g.id);
                    }}
                    onCoverError={(e) => (e.kind === "Conflict" ? setConflict(true) : setError(e.message))}
                  />
                )}
              </section>
            </div>
          </>
        )}
        <div className="dialog-actions">
          <button type="button" disabled={saving} onClick={() => guardNav(onClose)}>
            Cerrar
          </button>
        </div>
        {dup && (
          <ConfirmDialog title="Código duplicado" confirmLabel="Guardar de todos modos" busy={saving} onCancel={() => setDup(null)} onConfirm={() => void save(dup.input, true)}>
            <p>{dup.message}</p>
            <p>¿Guardar de todos modos?</p>
          </ConfirmDialog>
        )}
        {confirm && (
          <ConfirmDialog
            title="¿Descartar los cambios?"
            confirmLabel="Descartar"
            danger
            onCancel={() => setConfirm(null)}
            onConfirm={() => {
              const a = confirm;
              setConfirm(null);
              dirtyRef.current = false;
              a();
            }}
          >
            <p>Tienes cambios sin guardar en el formulario. Si continúas, se perderán.</p>
          </ConfirmDialog>
        )}
      </div>
    </div>
  );
}
