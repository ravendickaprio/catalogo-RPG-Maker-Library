import { memo, useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useVirtualizer } from "@tanstack/react-virtual";
import { cancelScan, getImportHistory, normalizeError, runImport, scanImport, undoLastImport } from "../api/client";
import type { HistoryEntry, ImportItem, ImportPreview, ImportResult, ItemStatus, ScanProgress, UndoResult, Vocabulary } from "../api/types";
import { useDialog } from "../hooks/useDialog";
import { withCurrent } from "../lib/gameInput";
import ConfirmDialog from "./ConfirmDialog";

interface Props {
  vocab: Vocabulary | null;
  readOnly: boolean;
  onClose: () => void;
  /** Data changed (import or undo): refresh counts, list and vocabulary. */
  onChanged: () => void;
}

interface Edit {
  checked: boolean;
  titulo: string;
  estado: string;
  idioma: string;
}

const STATUS_LABEL: Record<ItemStatus, string> = {
  nuevo: "Nuevo",
  duplicado_codigo: "Duplicado (código)",
  duplicado_ruta: "Duplicado (ruta)",
  duplicado_lote: "Duplicado (en este análisis)",
  sin_codigo: "Sin código",
};
const VIRTUAL_THRESHOLD = 300;
const ROW_H = 44;

function reasonOf(it: ImportItem): string {
  if (it.status === "sin_codigo") return "No se detectó un código RY-… en el nombre";
  if (it.status === "nuevo") return "";
  const of = it.duplicateOf ? `: «${it.duplicateOf.titulo}»` : "";
  return `${STATUS_LABEL[it.status]}${of}`;
}

/** Solo los elementos nuevos son importables: tienen entrada de edición. */
function initialEdits(items: ImportItem[]): Record<string, Edit> {
  const m: Record<string, Edit> = {};
  for (const it of items) if (it.status === "nuevo") m[it.key] = { checked: true, titulo: it.parsed.titulo, estado: it.parsed.estado, idioma: it.parsed.idioma };
  return m;
}

function Options({ list, current }: { list: string[]; current: string }) {
  return (
    <>
      {withCurrent(list, current).map((v) => (
        <option key={v} value={v}>
          {v}
        </option>
      ))}
    </>
  );
}

interface RowProps {
  item: ImportItem;
  /** Ausente si el elemento no es importable. */
  edit?: Edit;
  estados: string[];
  idiomas: string[];
  style?: React.CSSProperties;
  onEdit: (key: string, patch: Partial<Edit>) => void;
}

const ImportRow = memo(function ImportRow({ item, edit, estados, idiomas, style, onEdit }: RowProps) {
  const importable = !!edit;
  const titulo = edit?.titulo ?? item.parsed.titulo;
  const estado = edit?.estado ?? item.parsed.estado;
  const idioma = edit?.idioma ?? item.parsed.idioma;
  const reason = reasonOf(item);
  return (
    <div className="import-row" role="row" style={style}>
      <span role="cell">
        <input
          type="checkbox"
          aria-label={`Importar ${item.name}`}
          checked={!!edit?.checked}
          disabled={!importable}
          title={reason || undefined}
          onChange={(e) => onEdit(item.key, { checked: e.target.checked })}
        />
      </span>
      <span role="cell" className="import-title">
        <input type="text" aria-label={`Título de ${item.name}`} value={titulo} disabled={!importable} maxLength={300} onChange={(e) => onEdit(item.key, { titulo: e.target.value })} />
        {reason && <small className="cell-muted">{reason}</small>}
      </span>
      <span role="cell">
        <select aria-label={`Estado de ${item.name}`} value={estado} disabled={!importable} onChange={(e) => onEdit(item.key, { estado: e.target.value })}>
          <Options list={estados} current={estado} />
        </select>
      </span>
      <span role="cell">
        <select aria-label={`Idioma de ${item.name}`} value={idioma} disabled={!importable} onChange={(e) => onEdit(item.key, { idioma: e.target.value })}>
          <Options list={idiomas} current={idioma} />
        </select>
      </span>
      <span role="cell" className="cell-muted">
        {item.parsed.codigo || "—"}
      </span>
      <span role="cell" className="path-text import-path" title={item.path}>
        {item.path}
      </span>
    </div>
  );
});

function ItemTable({ items, edits, estados, idiomas, onEdit }: { items: ImportItem[]; edits: Record<string, Edit>; estados: string[]; idiomas: string[]; onEdit: RowProps["onEdit"] }) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const virtual = items.length > VIRTUAL_THRESHOLD;
  const renderRow = (it: ImportItem, style?: React.CSSProperties) => <ImportRow key={it.key} item={it} edit={edits[it.key]} estados={estados} idiomas={idiomas} onEdit={onEdit} style={style} />;
  const virtualizer = useVirtualizer({ count: virtual ? items.length : 0, getScrollElement: () => scrollRef.current, estimateSize: () => ROW_H, overscan: 8 });
  return (
    <div role="table" aria-label="Elementos detectados" aria-rowcount={items.length}>
      <div className="import-row import-head" role="row">
        <span role="columnheader">Importar</span>
        <span role="columnheader">Título</span>
        <span role="columnheader">Estado</span>
        <span role="columnheader">Idioma</span>
        <span role="columnheader">Código</span>
        <span role="columnheader">Ruta</span>
      </div>
      <div ref={scrollRef} className="import-scroll">
        {virtual ? (
          <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
            {virtualizer.getVirtualItems().map((v) => {
              return renderRow(items[v.index], { position: "absolute", top: 0, left: 0, right: 0, height: ROW_H, transform: `translateY(${v.start}px)` });
            })}
          </div>
        ) : (
          items.map((it) => renderRow(it))
        )}
      </div>
    </div>
  );
}

type Phase = "pick" | "scanning" | "preview" | "running" | "done";

export default function ImportDialog({ vocab, readOnly, onClose, onChanged }: Props) {
  const uid = useId();
  const [tab, setTab] = useState<"import" | "history">("import");
  const [phase, setPhase] = useState<Phase>("pick");
  const [root, setRoot] = useState("");
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [edits, setEdits] = useState<Record<string, Edit>>({});
  const [pagina, setPagina] = useState("");
  const [tipo, setTipo] = useState("");
  const [filter, setFilter] = useState<"" | ItemStatus>("");
  const [result, setResult] = useState<ImportResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmImport, setConfirmImport] = useState(false);

  const busy = phase === "scanning" || phase === "running";
  const guardedClose = () => {
    if (!busy) onClose();
  };
  const { ref, onKeyDown } = useDialog(guardedClose, "input");

  const estados = useMemo(() => vocab?.estados ?? [], [vocab]);
  const idiomas = useMemo(() => vocab?.idiomas ?? [], [vocab]);

  const pickFolder = async () => {
    setError(null);
    try {
      const sel = await open({ directory: true, multiple: false });
      if (typeof sel === "string") setRoot(sel);
    } catch (e) {
      setError(normalizeError(e).message);
    }
  };

  const analyze = async () => {
    const r = root.trim();
    if (!r) return;
    setError(null);
    setProgress(null);
    setPhase("scanning");
    try {
      const p = await scanImport(r, setProgress);
      if (p.cancelled) {
        setPhase("pick");
        setError("Análisis cancelado.");
        return;
      }
      setPreview(p);
      setEdits(initialEdits(p.items));
      setPagina(p.defaults.pagina);
      setTipo(p.defaults.tipo);
      setFilter("");
      setPhase("preview");
    } catch (e) {
      setPhase("pick");
      setError(normalizeError(e).message);
    }
  };

  const cancel = () => {
    cancelScan().catch((e) => setError(normalizeError(e).message));
  };

  const onEdit = useCallback((key: string, patch: Partial<Edit>) => setEdits((m) => ({ ...m, [key]: { ...m[key], ...patch } })), []);

  const visible = useMemo(() => (preview ? (filter ? preview.items.filter((i) => i.status === filter) : preview.items) : []), [preview, filter]);
  const selected = useMemo(() => (preview ? preview.items.filter((i) => edits[i.key]?.checked) : []), [preview, edits]);
  const allNewChecked = preview != null && preview.counts.nuevo > 0 && selected.length === preview.counts.nuevo;
  const toggleNew = () => {
    if (!preview) return;
    setEdits((m) => {
      const n: Record<string, Edit> = {};
      for (const k of Object.keys(m)) n[k] = { ...m[k], checked: !allNewChecked };
      return n;
    });
  };

  const doImport = async () => {
    if (!preview) return;
    setConfirmImport(false);
    setError(null);
    setPhase("running");
    try {
      const res = await runImport({
        root: preview.root,
        pagina: pagina.trim(),
        tipo: tipo.trim(),
        items: selected.map((i) => ({ key: i.key, path: i.path, titulo: edits[i.key].titulo, codigo: i.parsed.codigo, estado: edits[i.key].estado, idioma: edits[i.key].idioma })),
      });
      setResult(res);
      setPhase("done");
      onChanged();
    } catch (e) {
      setError(normalizeError(e).message);
      setPhase("preview");
    }
  };

  const reset = () => {
    setPhase("pick");
    setPreview(null);
    setResult(null);
    setError(null);
  };

  const names = useMemo(() => new Map((preview?.items ?? []).map((i) => [i.key, i.name])), [preview]);
  const titleOf = (key: string) => names.get(key) ?? key;
  const roNote = "Modo solo lectura: se puede analizar, pero no importar ni deshacer.";

  return (
    <>
      <div className="backdrop" onMouseDown={(e) => e.target === e.currentTarget && guardedClose()}>
        <div ref={ref} className="dialog import-dialog" role="dialog" aria-modal="true" aria-labelledby={`${uid}-t`} tabIndex={-1} onKeyDown={onKeyDown}>
          <h2 id={`${uid}-t`}>Importar juegos</h2>
          <div className="view-toggle" role="group" aria-label="Sección">
            <button type="button" aria-pressed={tab === "import"} disabled={busy} onClick={() => setTab("import")}>
              Importar
            </button>
            <button type="button" aria-pressed={tab === "history"} disabled={busy} onClick={() => setTab("history")}>
              Historial
            </button>
          </div>
          {readOnly && (
            <p className="note" role="note">
              {roNote}
            </p>
          )}
          {error && (
            <p className="field-error" role="alert">
              {error}
            </p>
          )}

          {tab === "import" && (
            <>
              {(phase === "pick" || phase === "scanning") && (
                <section aria-label="Carpeta">
                  <div className="form-row">
                    <label htmlFor={`${uid}-root`}>Carpeta a analizar</label>
                    <div className="inline-row">
                      <input id={`${uid}-root`} type="text" value={root} disabled={phase === "scanning"} placeholder="Ruta absoluta de la carpeta" onChange={(e) => setRoot(e.target.value)} onKeyDown={(e) => e.key === "Enter" && (e.preventDefault(), void analyze())} />
                      <button type="button" disabled={phase === "scanning"} onClick={() => void pickFolder()}>
                        Elegir carpeta…
                      </button>
                      <button type="button" className="primary" disabled={phase === "scanning" || !root.trim()} onClick={() => void analyze()}>
                        Analizar
                      </button>
                    </div>
                  </div>
                  {phase === "scanning" && (
                    <div role="status" aria-live="polite" className="import-progress">
                      <p>
                        Analizando… {progress?.scanned ?? 0} entradas, {progress?.candidates ?? 0} candidatos
                      </p>
                      <p className="path-text">{progress?.currentDir ?? ""}</p>
                      <button type="button" onClick={cancel}>
                        Cancelar
                      </button>
                    </div>
                  )}
                </section>
              )}

              {phase === "preview" && preview && (
                <section aria-label="Vista previa">
                  <p role="status">
                    {preview.counts.total} elementos: {preview.counts.nuevo} nuevos, {preview.counts.duplicadoCodigo + preview.counts.duplicadoRuta + preview.counts.duplicadoLote} duplicados, {preview.counts.sinCodigo} sin código. Analizadas {preview.scanned} entradas.
                  </p>
                  {preview.truncated && (
                    <p className="banner error" role="alert">
                      Resultado truncado: hay más candidatos que el límite de análisis.
                    </p>
                  )}
                  {preview.unreadable > 0 && <p className="note">{preview.unreadable} carpeta(s) no se pudieron leer.</p>}
                  <div className="inline-row import-controls">
                    <label>
                      Estado{" "}
                      <select aria-label="Filtrar por estado de importación" value={filter} onChange={(e) => setFilter(e.target.value as "" | ItemStatus)}>
                        <option value="">Todos</option>
                        {(Object.keys(STATUS_LABEL) as ItemStatus[]).map((s) => (
                          <option key={s} value={s}>
                            {STATUS_LABEL[s]}
                          </option>
                        ))}
                      </select>
                    </label>
                    <button type="button" onClick={toggleNew} disabled={preview.counts.nuevo === 0}>
                      {allNewChecked ? "Desmarcar todos los nuevos" : "Marcar todos los nuevos"}
                    </button>
                    <label>
                      Página{" "}
                      <input type="text" list={`${uid}-paginas`} value={pagina} maxLength={100} onChange={(e) => setPagina(e.target.value)} />
                    </label>
                    <label>
                      Tipo{" "}
                      <input type="text" list={`${uid}-tipos`} value={tipo} maxLength={50} onChange={(e) => setTipo(e.target.value)} />
                    </label>
                    <datalist id={`${uid}-paginas`}>
                      {(vocab?.paginas ?? []).map((p) => (
                        <option key={p} value={p} />
                      ))}
                    </datalist>
                    <datalist id={`${uid}-tipos`}>
                      {(vocab?.tipos ?? []).map((p) => (
                        <option key={p} value={p} />
                      ))}
                    </datalist>
                  </div>
                  {visible.length ? <ItemTable items={visible} edits={edits} estados={estados} idiomas={idiomas} onEdit={onEdit} /> : <p className="empty">No hay elementos para mostrar.</p>}
                  {readOnly && <p className="note">{roNote}</p>}
                  <div className="dialog-actions">
                    <button type="button" onClick={reset}>
                      Otra carpeta
                    </button>
                    <button type="button" className="primary" disabled={readOnly || selected.length === 0} onClick={() => setConfirmImport(true)}>
                      Importar {selected.length} juegos
                    </button>
                  </div>
                </section>
              )}

              {phase === "running" && (
                <p role="status" aria-live="polite">
                  Importando…
                </p>
              )}

              {phase === "done" && result && (
                <section aria-label="Resumen">
                  <p role="status">
                    Importados: {result.inserted.length} · omitidos: {result.skipped.length}
                  </p>
                  {result.warning && (
                    <p className="banner error" role="alert">
                      {result.warning}
                    </p>
                  )}
                  {result.backupPath && (
                    <p>
                      Backup: <span className="path-text">{result.backupPath}</span>
                    </p>
                  )}
                  {result.skipped.length > 0 && (
                    <ul aria-label="Omitidos">
                      {result.skipped.map((s) => (
                        <li key={s.key}>
                          {titleOf(s.key)}: {s.reason}
                        </li>
                      ))}
                    </ul>
                  )}
                  <div className="dialog-actions">
                    <button type="button" onClick={reset}>
                      Importar otra carpeta
                    </button>
                  </div>
                </section>
              )}
            </>
          )}

          {tab === "history" && <HistoryPanel readOnly={readOnly} onChanged={onChanged} />}

          <div className="dialog-actions">
            <button type="button" disabled={busy} onClick={onClose}>
              Cerrar
            </button>
          </div>
        </div>
      </div>
      {confirmImport && (
        <ConfirmDialog title="¿Importar juegos?" confirmLabel={`Importar ${selected.length} juegos`} onCancel={() => setConfirmImport(false)} onConfirm={() => void doImport()}>
          <p>Se creará un backup y se añadirán {selected.length} juegos a la biblioteca (página «{pagina}», tipo «{tipo || "Otro"}»). Podrás deshacer la última importación desde el historial.</p>
        </ConfirmDialog>
      )}
    </>
  );
}

function HistoryPanel({ readOnly, onChanged }: { readOnly: boolean; onChanged: () => void }) {
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [undo, setUndo] = useState<UndoResult | null>(null);
  const load = useCallback(() => {
    getImportHistory().then(
      (h) => setEntries(h),
      (e) => setError(normalizeError(e).message),
    );
  }, []);
  useEffect(load, [load]);

  const last = entries?.[0];
  const canUndo = !!last && !last.undone && !readOnly;

  const running = useRef(false);
  const doUndo = async () => {
    if (running.current) return;
    running.current = true;
    setBusy(true);
    setUndo(null);
    setError(null);
    try {
      setUndo(await undoLastImport());
      onChanged();
      load();
    } catch (e) {
      setError(normalizeError(e).message);
    } finally {
      running.current = false;
      setBusy(false);
      setConfirm(false);
    }
  };

  return (
    <section aria-label="Historial">
      {error && (
        <p className="field-error" role="alert">
          {error}
        </p>
      )}
      {entries && entries.length === 0 && <p className="empty">Aún no hay importaciones.</p>}
      {entries && entries.length > 0 && (
        <ul aria-label="Importaciones anteriores">
          {entries.map((h) => (
            <li key={h.id}>
              {new Date(h.timestamp).toLocaleString()} · <span className="path-text">{h.root}</span> · {h.insertedCount} importados, {h.skippedCount} omitidos{h.undone ? " · deshecha" : ""}
            </li>
          ))}
        </ul>
      )}
      {readOnly && <p className="note">Modo solo lectura: no se puede deshacer.</p>}
      <button type="button" disabled={!canUndo || busy} onClick={() => setConfirm(true)}>
        Deshacer última importación
      </button>
      {undo && (
        <div role="status">
          <p>
            Eliminados: {undo.deleted} · conservados: {undo.kept.length}
          </p>
          {undo.kept.length > 0 && (
            <ul aria-label="Conservados">
              {undo.kept.map((k) => (
                <li key={k.id}>
                  {k.titulo}: {k.reason}
                </li>
              ))}
            </ul>
          )}
          {undo.warning && (
            <p className="banner error" role="alert">
              {undo.warning}
            </p>
          )}
          {undo.backupPath && (
            <p>
              Backup: <span className="path-text">{undo.backupPath}</span>
            </p>
          )}
        </div>
      )}
      {confirm && (
        <ConfirmDialog title="¿Deshacer la última importación?" confirmLabel="Deshacer importación" danger busy={busy} onCancel={() => setConfirm(false)} onConfirm={() => void doUndo()}>
          <p>Se eliminarán los juegos de la última importación que no hayan sido modificados desde entonces. Los modificados se conservarán.</p>
        </ConfirmDialog>
      )}
    </section>
  );
}
