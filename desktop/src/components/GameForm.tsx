import { useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { GameDetail, GameInput, Vocabulary } from "../api/types";
import {
  SCORE_STEPS,
  defaultValues,
  toInput,
  validate,
  valuesFromGame,
  withCurrent,
  type FieldKey,
  type FormValues,
} from "../lib/gameInput";
import type { AppError } from "../api/types";
import ConfirmDialog from "./ConfirmDialog";
import CoverEditor from "./CoverEditor";
import TagEditor from "./TagEditor";

interface Props {
  /** null = new game. */
  game: GameDetail | null;
  vocab: Vocabulary | null;
  tagSuggestions: string[];
  saving: boolean;
  onSubmit: (input: GameInput) => void;
  onCancel: () => void;
  onDirtyChange?: (dirty: boolean) => void;
  /** Cover changes are saved immediately; the parent receives the updated record (new revision). */
  onCoverChanged?: (g: GameDetail) => void;
  onCoverError?: (e: AppError) => void;
}

export default function GameForm({ game, vocab, tagSuggestions, saving, onSubmit, onCancel, onDirtyChange, onCoverChanged, onCoverError }: Props) {
  const initial = useMemo(() => (game ? valuesFromGame(game) : defaultValues(vocab)), [game]); // eslint-disable-line react-hooks/exhaustive-deps
  const [values, setValues] = useState<FormValues>(initial);
  const [submitted, setSubmitted] = useState(false);
  const [confirmDiscard, setConfirmDiscard] = useState(false);
  const uid = useId();
  const titleRef = useRef<HTMLInputElement>(null);
  const descRef = useRef<HTMLTextAreaElement>(null);
  const initialJson = useMemo(() => JSON.stringify(toInput(initial)), [initial]);
  const dirty = JSON.stringify(toInput(values)) !== initialJson;
  const errors = validate(values);
  const shown = submitted ? errors : {};

  useEffect(() => {
    onDirtyChange?.(dirty);
  }, [dirty]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => () => onDirtyChange?.(false), []); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    titleRef.current?.focus();
  }, []);
  useLayoutEffect(() => {
    const t = descRef.current;
    if (t) {
      t.style.height = "auto";
      t.style.height = `${t.scrollHeight}px`;
    }
  }, [values.descripcion]);

  const set = <K extends keyof FormValues>(k: K, v: FormValues[K]) => setValues((p) => ({ ...p, [k]: v }));

  const submit = () => {
    if (saving) return;
    setSubmitted(true);
    const errs = validate(values);
    const first = (Object.keys(errs) as FieldKey[])[0];
    if (first) {
      document.getElementById(`${uid}-${first}`)?.focus();
      return;
    }
    onSubmit(toInput(values));
  };
  const cancel = () => (dirty ? setConfirmDiscard(true) : onCancel());

  const err = (k: FieldKey) => shown[k];
  const fieldProps = (k: FieldKey) => ({
    id: `${uid}-${k}`,
    "aria-invalid": err(k) ? (true as const) : undefined,
    "aria-describedby": err(k) ? `${uid}-${k}-err` : undefined,
  });
  const msg = (k: FieldKey) =>
    err(k) ? (
      <p className="field-error" id={`${uid}-${k}-err`} role="alert">
        {err(k)}
      </p>
    ) : null;

  const select = (k: "estado" | "idioma" | "tipo" | "pagina", label: string, list: string[], allowEmpty: boolean, required = false) => (
    <div className="form-row">
      <label htmlFor={`${uid}-${k}`}>
        {label}
        {required && " *"}
      </label>
      <select {...fieldProps(k)} value={values[k]} onChange={(e) => set(k, e.target.value)}>
        {allowEmpty && <option value="">(ninguno)</option>}
        {!allowEmpty && values[k] === "" && <option value="">Selecciona…</option>}
        {withCurrent(list, values[k]).map((o) => (
          <option key={o} value={o}>
            {o}
          </option>
        ))}
      </select>
      {msg(k)}
    </div>
  );

  const scores = SCORE_STEPS.includes(values.puntuacion) ? SCORE_STEPS : [...SCORE_STEPS, values.puntuacion];

  return (
    <form
      className="game-form"
      aria-label={game ? "Editar juego" : "Nuevo juego"}
      noValidate
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
      onKeyDown={(e) => {
        if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
          e.preventDefault();
          submit();
        } else if (e.key === "Escape" && !confirmDiscard) {
          e.preventDefault();
          e.stopPropagation();
          cancel();
        }
      }}
    >
      <h2>{game ? "Editar juego" : "Nuevo juego"}</h2>
      <div className="form-row">
        <label htmlFor={`${uid}-titulo`}>Título *</label>
        <input ref={titleRef} type="text" {...fieldProps("titulo")} value={values.titulo} onChange={(e) => set("titulo", e.target.value)} />
        {msg("titulo")}
      </div>
      <div className="form-row">
        <label htmlFor={`${uid}-codigo`}>Código</label>
        <input type="text" {...fieldProps("codigo")} value={values.codigo} onChange={(e) => set("codigo", e.target.value)} />
        {msg("codigo")}
      </div>
      {select("estado", "Estado", vocab?.estados ?? [], false, true)}
      {select("idioma", "Idioma", vocab?.idiomas ?? [], false, true)}
      {select("tipo", "Tipo", vocab?.tipos ?? [], true)}
      {select("pagina", "Página", vocab?.paginas ?? [], true)}
      <div className="form-row">
        <label htmlFor={`${uid}-puntuacion`}>Puntuación</label>
        <select {...fieldProps("puntuacion")} value={String(values.puntuacion)} onChange={(e) => set("puntuacion", Number(e.target.value))}>
          {scores.map((s) => (
            <option key={s} value={String(s)}>
              {s}
            </option>
          ))}
        </select>
        {msg("puntuacion")}
      </div>
      <div className="form-row">
        <label htmlFor={`${uid}-ruta`}>Ruta</label>
        <input type="text" {...fieldProps("ruta")} value={values.ruta} onChange={(e) => set("ruta", e.target.value)} />
        {msg("ruta")}
      </div>
      <div className="form-row">
        <label htmlFor={`${uid}-descripcion`}>Descripción</label>
        <textarea ref={descRef} rows={3} {...fieldProps("descripcion")} value={values.descripcion} onChange={(e) => set("descripcion", e.target.value)} />
        {msg("descripcion")}
      </div>
      <div className="form-row">
        <span id={`${uid}-tags-label`}>Etiquetas</span>
        <TagEditor
          tags={values.tags}
          draft={values.tagDraft}
          suggestions={tagSuggestions}
          invalid={!!err("tags")}
          describedBy={err("tags") ? `${uid}-tags-err` : undefined}
          onChange={(tags, tagDraft) => setValues((p) => ({ ...p, tags, tagDraft }))}
        />
        {msg("tags")}
      </div>
      {game ? (
        <CoverEditor game={game} onChanged={(g) => onCoverChanged?.(g)} onError={(e) => onCoverError?.(e)} />
      ) : (
        <p className="note">Podrás añadir la portada después de crear el juego.</p>
      )}
      <div className="dialog-actions">
        <button type="button" onClick={cancel} disabled={saving} title="Cancelar (Esc)">
          Cancelar
        </button>
        <button type="submit" className="primary" disabled={saving} title="Guardar (Ctrl+S)">
          {saving ? "Guardando…" : "Guardar"}
        </button>
      </div>
      {confirmDiscard && (
        <ConfirmDialog
          title="¿Descartar los cambios?"
          confirmLabel="Descartar"
          danger
          onCancel={() => setConfirmDiscard(false)}
          onConfirm={() => {
            setConfirmDiscard(false);
            onCancel();
          }}
        >
          <p>Tienes cambios sin guardar que se perderán.</p>
        </ConfirmDialog>
      )}
    </form>
  );
}
