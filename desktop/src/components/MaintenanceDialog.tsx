import { memo, useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  applyBatch,
  cancelMaintenanceScan,
  deleteOrphanTags,
  listBatches,
  listOrphanTags,
  maintenanceList,
  maintenanceSummary,
  normalizeError,
  previewBatch,
  rebuildSearchIndex,
  scanMissingPaths,
  searchIndexStatus,
  undoLastBatch,
} from "../api/client";
import type {
  BatchAction,
  BatchEntry,
  BatchField,
  BatchPreview,
  BatchRequest,
  BatchResult,
  CheckCount,
  CheckId,
  IndexStatus,
  Issue,
  MissingScanProgress,
  OrphanTag,
  UndoBatchResult,
  Vocabulary,
} from "../api/types";
import { useDialog } from "../hooks/useDialog";
import { withCurrent } from "../lib/gameInput";
import ConfirmDialog from "./ConfirmDialog";

export interface OpenGameRef {
  id: number;
  codigo: string;
  titulo: string;
}

interface Props {
  vocab: Vocabulary | null;
  readOnly: boolean;
  onClose: () => void;
  /** Data changed (batch, undo, review, tag delete, index rebuild): refresh counts, list and vocabulary. */
  onChanged: () => void;
  /** Shows a game in the main window (the host closes this dialog). */
  onOpenGame?: (game: OpenGameRef) => void;
  /** Opens the batch review preset to a criterion (the host closes this dialog). */
  onOpenReview: (criterion: "sin_descripcion") => void;
}

export const CHECK_LABEL: Record<CheckId, string> = {
  sin_descripcion: "Sin descripción",
  sin_portada: "Sin portada",
  sin_tags: "Sin tags",
  codigo_duplicado: "Código duplicado",
  codigo_vacio: "Código vacío",
  titulo_ry: "Título con prefijo RY",
  estado_inconsistente: "Estado inconsistente",
  ruta_inexistente: "Ruta inexistente",
};
const CHECK_HELP: Record<CheckId, string> = {
  sin_descripcion: "Juegos con la descripción vacía.",
  sin_portada: "Juegos sin imagen de portada.",
  sin_tags: "Juegos sin ninguna etiqueta.",
  codigo_duplicado: "Códigos compartidos por dos o más juegos.",
  codigo_vacio: "Juegos sin código.",
  titulo_ry: "Títulos que empiezan por RY (probable código pegado al título).",
  estado_inconsistente: "El nombre del archivo indica un estado distinto al guardado (informativo).",
  ruta_inexistente: "Rutas que ya no existen en disco. Requiere comprobarlas.",
};
const CHECK_ORDER: CheckId[] = ["sin_descripcion", "sin_portada", "sin_tags", "codigo_duplicado", "codigo_vacio", "titulo_ry", "estado_inconsistente", "ruta_inexistente"];
const PAGE = 200;
const VIRTUAL_THRESHOLD = 300;
const ROW_H = 40;
const PREVIEW_CAP = 500;
const MAX_IDS = 5000;
const RO_NOTE = "Modo solo lectura: se pueden consultar los problemas y comprobar rutas, pero no aplicar lotes, deshacer, reconstruir ni eliminar.";

type Tab = "overview" | "batches";

export default function MaintenanceDialog({ vocab, readOnly, onClose, onChanged, onOpenGame, onOpenReview }: Props) {
  const uid = useId();
  const [tab, setTab] = useState<Tab>("overview");
  const [busy, setBusy] = useState(false);
  const guardedClose = () => {
    if (!busy) onClose();
  };
  const { ref, onKeyDown } = useDialog(guardedClose, "button");
  const tabs: [Tab, string][] = [
    ["overview", "Resumen"],
    ["batches", "Historial de lotes"],
  ];
  return (
    <div className="backdrop" onMouseDown={(e) => e.target === e.currentTarget && guardedClose()}>
      <div ref={ref} className="dialog maint-dialog" role="dialog" aria-modal="true" aria-labelledby={`${uid}-t`} tabIndex={-1} onKeyDown={onKeyDown}>
        <h2 id={`${uid}-t`}>Centro de mantenimiento</h2>
        <div className="view-toggle" role="group" aria-label="Sección">
          {tabs.map(([k, label]) => (
            <button key={k} type="button" aria-pressed={tab === k} disabled={busy} onClick={() => setTab(k)}>
              {label}
            </button>
          ))}
        </div>
        {readOnly && (
          <p className="note" role="note">
            {RO_NOTE}
          </p>
        )}
        {tab === "overview" && <Overview vocab={vocab} readOnly={readOnly} onChanged={onChanged} onOpenGame={onOpenGame} onBusy={setBusy} />}
        {tab === "batches" && <BatchesPanel readOnly={readOnly} onChanged={onChanged} onBusy={setBusy} />}
        <div className="dialog-actions">
          <button type="button" disabled={busy} onClick={() => onOpenReview("sin_descripcion")}>
            Revisión por tandas (sin descripción)
          </button>
          <button type="button" disabled={busy} onClick={onClose}>
            Cerrar
          </button>
        </div>
      </div>
    </div>
  );
}

function Err({ message }: { message: string | null }) {
  return message ? (
    <p className="field-error" role="alert">
      {message}
    </p>
  ) : null;
}

function Warn({ message }: { message: string | null }) {
  return message ? (
    <p className="banner error" role="alert">
      {message}
    </p>
  ) : null;
}

/* ───────────────────────── Issue list ───────────────────────── */

interface RowProps {
  issue: Issue;
  checked: boolean;
  style?: React.CSSProperties;
  onToggle: (id: number) => void;
  onOpen?: (i: Issue) => void;
}

const IssueRow = memo(function IssueRow({ issue, checked, style, onToggle, onOpen }: RowProps) {
  return (
    <div className="maint-row" role="row" style={style}>
      <span role="cell">
        <input type="checkbox" aria-label={`Seleccionar ${issue.titulo}`} checked={checked} onChange={() => onToggle(issue.id)} />
      </span>
      <span role="cell" className="maint-title">
        {onOpen ? (
          <button type="button" className="link-btn" title="Mostrar en la biblioteca" onClick={() => onOpen(issue)}>
            {issue.titulo}
          </button>
        ) : (
          issue.titulo
        )}
        {issue.detail && <small className="cell-muted">{issue.detail}</small>}
      </span>
      <span role="cell" className="cell-muted">
        {issue.codigo || "—"}
      </span>
      <span role="cell">{issue.estado}</span>
    </div>
  );
});

function IssueTable({ items, selected, onToggle, onOpen }: { items: Issue[]; selected: Set<number>; onToggle: (id: number) => void; onOpen?: (i: Issue) => void }) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const virtual = items.length > VIRTUAL_THRESHOLD;
  const virtualizer = useVirtualizer({ count: virtual ? items.length : 0, getScrollElement: () => scrollRef.current, estimateSize: () => ROW_H, overscan: 8 });
  const row = (i: Issue, style?: React.CSSProperties) => <IssueRow key={i.id} issue={i} checked={selected.has(i.id)} onToggle={onToggle} onOpen={onOpen} style={style} />;
  return (
    <div role="table" aria-label="Problemas detectados" aria-rowcount={items.length}>
      <div className="maint-row maint-head" role="row">
        <span role="columnheader">Sel.</span>
        <span role="columnheader">Título</span>
        <span role="columnheader">Código</span>
        <span role="columnheader">Estado</span>
      </div>
      <div ref={scrollRef} className="maint-scroll">
        {virtual ? (
          <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
            {virtualizer.getVirtualItems().map((v) => row(items[v.index], { position: "absolute", top: 0, left: 0, right: 0, height: ROW_H, transform: `translateY(${v.start}px)` }))}
          </div>
        ) : (
          items.map((i) => row(i))
        )}
      </div>
    </div>
  );
}

/* ───────────────────────── Overview ───────────────────────── */

type FormKind = BatchField | "add_tag" | "remove_tag";
const FORM_LABEL: Record<FormKind, string> = {
  estado: "Establecer estado",
  idioma: "Establecer idioma",
  tipo: "Establecer tipo",
  pagina: "Establecer página",
  add_tag: "Añadir tag",
  remove_tag: "Quitar tag",
};

function describeAction(a: BatchAction): string {
  switch (a.kind) {
    case "set_field":
      return `${a.field} → «${a.value}»`;
    case "add_tag":
      return `añadir tag «${a.tag}»`;
    case "remove_tag":
      return `quitar tag «${a.tag}»`;
    case "fix_ry_titles":
      return "corregir títulos RY";
  }
}

interface OverviewProps {
  vocab: Vocabulary | null;
  readOnly: boolean;
  onChanged: () => void;
  onOpenGame?: (g: OpenGameRef) => void;
  onBusy: (b: boolean) => void;
}

function Overview({ vocab, readOnly, onChanged, onOpenGame, onBusy }: OverviewProps) {
  const [error, setError] = useState<string | null>(null);
  const [counts, setCounts] = useState<CheckCount[] | null>(null);
  const [index, setIndex] = useState<IndexStatus | null>(null);
  const [orphans, setOrphans] = useState<OrphanTag[] | null>(null);
  const [active, setActive] = useState<CheckId | null>(null);
  const [items, setItems] = useState<Issue[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  // Missing paths scan
  const [scanning, setScanning] = useState(false);
  const [scanProgress, setScanProgress] = useState<MissingScanProgress | null>(null);
  const [scanned, setScanned] = useState<Issue[] | null>(null);
  const [scanNote, setScanNote] = useState<string | null>(null);
  // Maintenance operations
  const [confirm, setConfirm] = useState<"index" | "orphans" | "apply" | null>(null);
  const [working, setWorking] = useState(false);
  const [opMessage, setOpMessage] = useState<string | null>(null);
  const [opWarning, setOpWarning] = useState<string | null>(null);
  // Batch
  const [formKind, setFormKind] = useState<FormKind>("estado");
  const [formValue, setFormValue] = useState("");
  const [previewing, setPreviewing] = useState(false);
  const [preview, setPreview] = useState<{ req: BatchRequest; data: BatchPreview } | null>(null);
  const [result, setResult] = useState<BatchResult | null>(null);
  const names = useRef(new Map<number, string>());
  const [excluded, setExcluded] = useState<Set<number>>(new Set());
  const seq = useRef(0);
  const previewSeq = useRef(0);
  const applying = useRef(false);
  const opRunning = useRef(false);

  const busy = scanning || working || previewing;
  useEffect(() => {
    onBusy(busy);
  }, [busy, onBusy]);
  useEffect(() => () => onBusy(false), [onBusy]);

  const fail = (e: unknown) => setError(normalizeError(e).message);
  const loadSummary = useCallback(() => {
    const f = (e: unknown) => setError(normalizeError(e).message);
    maintenanceSummary().then(setCounts, f);
    searchIndexStatus().then(setIndex, f);
    listOrphanTags().then(setOrphans, f);
  }, []);
  useEffect(loadSummary, [loadSummary]);

  const loadIssues = useCallback((check: CheckId, offset: number) => {
    const my = ++seq.current;
    setLoading(true);
    maintenanceList(check, offset, PAGE).then(
      (p) => {
        if (my !== seq.current) return;
        for (const i of p.items) names.current.set(i.id, i.titulo);
        setItems((cur) => (offset === 0 ? p.items : [...cur, ...p.items]));
        setTotal(p.total);
        setLoading(false);
      },
      (e) => {
        if (my !== seq.current) return;
        setLoading(false);
        setError(normalizeError(e).message);
      },
    );
  }, []);

  // Descarta la vista previa y cualquier cálculo en curso (su respuesta llegaría obsoleta).
  const dropPreview = useCallback(() => {
    previewSeq.current++;
    setPreviewing(false);
    setPreview(null);
  }, []);

  const pick = (check: CheckId) => {
    setActive(check);
    setError(null);
    setSelected(new Set());
    dropPreview();
    setResult(null);
    setOpMessage(null);
    if (check === "ruta_inexistente") {
      seq.current++;
      setItems(scanned ?? []);
      setTotal(scanned?.length ?? 0);
      setLoading(false);
    } else {
      setItems([]);
      setTotal(0);
      loadIssues(check, 0);
    }
  };

  const refreshAfterWrite = () => {
    onChanged();
    loadSummary();
    if (active && active !== "ruta_inexistente") loadIssues(active, 0);
  };

  const runScan = async () => {
    seq.current++;
    setLoading(false);
    setError(null);
    setScanNote(null);
    setScanProgress(null);
    setScanning(true);
    try {
      const r = await scanMissingPaths(setScanProgress);
      seq.current++; // una carga de detector lanzada durante el análisis ya es obsoleta
      setLoading(false);
      setScanned(r.items);
      setScanNote(r.cancelled ? "Comprobación cancelada: los resultados son parciales." : `Comprobación terminada: ${r.items.length} rutas inexistentes.`);
      for (const i of r.items) names.current.set(i.id, i.titulo);
      setActive("ruta_inexistente");
      setItems(r.items);
      setTotal(r.items.length);
      setSelected(new Set());
      dropPreview();
      setResult(null);
    } catch (e) {
      fail(e);
    } finally {
      setScanning(false);
    }
  };

  const cancelScan = () => {
    cancelMaintenanceScan().catch(fail);
  };

  const toggle = useCallback((id: number) => {
    dropPreview();
    setSelected((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  }, [dropPreview]);
  const allLoaded = items.length > 0 && items.every((i) => selected.has(i.id));
  const toggleAll = () => {
    dropPreview();
    setSelected(allLoaded ? new Set() : new Set(items.map((i) => i.id)));
  };

  const doPreview = async (action: BatchAction) => {
    const ids = [...selected];
    if (!ids.length) return;
    setError(null);
    setResult(null);
    setPreview(null);
    const my = ++previewSeq.current;
    setPreviewing(true);
    const req: BatchRequest = { ids, action };
    try {
      const data = await previewBatch(req);
      if (my !== previewSeq.current) return;
      setExcluded(new Set());
      setPreview({ req, data });
    } catch (e) {
      if (my === previewSeq.current) fail(e);
    } finally {
      if (my === previewSeq.current) setPreviewing(false);
    }
  };
  const formAction = (): BatchAction | null => {
    const v = formValue.trim();
    if (!v) return null;
    if (formKind === "add_tag") return { kind: "add_tag", tag: v };
    if (formKind === "remove_tag") return { kind: "remove_tag", tag: v };
    return { kind: "set_field", field: formKind, value: v };
  };

  const doApply = async () => {
    if (!preview || applying.current) return;
    applying.current = true;
    setWorking(true);
    setError(null);
    try {
      const revisions: Record<number, string> = {};
      const chosen = preview.data.changes.filter((c) => !excluded.has(c.id));
      if (!chosen.length) return;
      for (const c of chosen) revisions[c.id] = c.revision;
      const req: BatchRequest = { ids: chosen.map((c) => c.id), action: preview.req.action };
      const r = await applyBatch(req, revisions);
      setResult(r);
      setPreview(null);
      setSelected(new Set());
      refreshAfterWrite();
    } catch (e) {
      fail(e);
    } finally {
      applying.current = false;
      setWorking(false);
      setConfirm(null);
    }
  };

  const doRebuild = async () => {
    if (opRunning.current) return;
    opRunning.current = true;
    setWorking(true);
    setError(null);
    setOpMessage(null);
    try {
      const s = await rebuildSearchIndex();
      setIndex(s);
      setOpMessage("Índice de búsqueda reconstruido.");
      onChanged();
    } catch (e) {
      fail(e);
    } finally {
      opRunning.current = false;
      setWorking(false);
      setConfirm(null);
    }
  };

  const doDeleteOrphans = async () => {
    if (opRunning.current) return;
    opRunning.current = true;
    setWorking(true);
    setError(null);
    setOpMessage(null);
    setOpWarning(null);
    try {
      const r = await deleteOrphanTags();
      setOpMessage(`Tags huérfanas eliminadas: ${r.changed}.${r.backupPath ? ` Backup: ${r.backupPath}` : ""}`);
      setOpWarning(r.warning);
      listOrphanTags().then(setOrphans, fail);
      onChanged();
    } catch (e) {
      fail(e);
    } finally {
      opRunning.current = false;
      setWorking(false);
      setConfirm(null);
    }
  };

  const countOf = (c: CheckId): string => {
    if (c === "ruta_inexistente") return scanned ? String(scanned.length) : "—";
    const n = counts?.find((x) => x.check === c)?.count;
    return n == null ? "…" : String(n);
  };
  const titleOf = (id: number) => names.current.get(id) ?? `#${id}`;
  const open = onOpenGame ? (i: Issue) => onOpenGame({ id: i.id, codigo: i.codigo, titulo: i.titulo }) : undefined;
  const nSel = selected.size;
  const nChosen = preview ? preview.data.changes.filter((c) => !excluded.has(c.id)).length : 0;
  const toggleChange = (id: number) =>
    setExcluded((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  const toggleAllChanges = () => {
    if (!preview) return;
    setExcluded(nChosen === preview.data.changes.length ? new Set(preview.data.changes.map((c) => c.id)) : new Set());
  };
  const tooMany = nSel > MAX_IDS;
  const batchOff = readOnly || nSel === 0 || tooMany || previewing || working;

  const valueList = useMemo(() => {
    switch (formKind) {
      case "estado":
        return vocab?.estados ?? [];
      case "idioma":
        return vocab?.idiomas ?? [];
      case "tipo":
        return vocab?.tipos ?? [];
      case "pagina":
        return vocab?.paginas ?? [];
      default:
        return [];
    }
  }, [formKind, vocab]);
  const isSelectKind = formKind !== "add_tag" && formKind !== "remove_tag";
  const action = formAction();
  const mismatch = index != null && (index.mismatched > 0 || !index.ftsOk);

  return (
    <>
      <Err message={error} />
      <section aria-label="Detectores">
        <div className="maint-cards">
          {CHECK_ORDER.map((c) => (
            <button key={c} type="button" className="maint-card" aria-pressed={active === c} title={CHECK_HELP[c]} onClick={() => pick(c)}>
              <span className="maint-card-label">{CHECK_LABEL[c]}</span>
              <span className="maint-card-count">{countOf(c)}</span>
            </button>
          ))}
        </div>
        <div className="inline-row maint-tools">
          <button type="button" disabled={scanning || working} onClick={() => void runScan()}>
            Comprobar rutas
          </button>
          {scanning && (
            <>
              <span role="status" aria-live="polite">
                Comprobando rutas… {scanProgress?.checked ?? 0} de {scanProgress?.total ?? "?"}
              </span>
              <button type="button" onClick={cancelScan}>
                Cancelar
              </button>
            </>
          )}
          {!scanning && scanNote && <span role="status">{scanNote}</span>}
        </div>
      </section>

      <section aria-label="Índice de búsqueda" className="maint-box">
        <h3>Índice de búsqueda</h3>
        {index ? (
          <p role="status">
            {index.juegos} juegos · {index.busqueda} en el índice · {index.mismatched} desajustados · integridad FTS: {index.ftsOk ? "correcta" : "con errores"}
            {mismatch ? " — conviene reconstruirlo." : "."}
          </p>
        ) : (
          <p className="cell-muted">Cargando…</p>
        )}
        <button type="button" disabled={readOnly || working || scanning} onClick={() => setConfirm("index")}>
          Reconstruir índice
        </button>
      </section>

      <section aria-label="Tags huérfanas" className="maint-box">
        <h3>Tags huérfanas</h3>
        {orphans ? (
          <p role="status">
            {orphans.length === 0 ? "No hay tags huérfanas." : `${orphans.length} tags sin ningún juego: ${orphans.slice(0, 15).map((t) => t.nombre).join(", ")}${orphans.length > 15 ? "…" : ""}`}
          </p>
        ) : (
          <p className="cell-muted">Cargando…</p>
        )}
        <button type="button" disabled={readOnly || working || scanning || !orphans?.length} onClick={() => setConfirm("orphans")}>
          Eliminar tags huérfanas
        </button>
        {opMessage && <p role="status">{opMessage}</p>}
        <Warn message={opWarning} />
      </section>

      {active && (
        <section aria-label={`Problemas: ${CHECK_LABEL[active]}`} className="maint-box">
          <h3>{CHECK_LABEL[active]}</h3>
          <p className="note">{CHECK_HELP[active]}</p>
          {active === "ruta_inexistente" && !scanned && <p className="empty">Pulsa «Comprobar rutas» para buscar rutas inexistentes.</p>}
          {items.length > 0 && (
            <>
              <div className="inline-row maint-controls">
                <label>
                  <input type="checkbox" checked={allLoaded} onChange={toggleAll} /> Seleccionar todos los cargados
                </label>
                <span role="status" aria-live="polite">
                  {nSel} seleccionados · {items.length} de {total} cargados
                </span>
                {items.length < total && active !== "ruta_inexistente" && (
                  <button type="button" disabled={loading} onClick={() => loadIssues(active, items.length)}>
                    Cargar más
                  </button>
                )}
              </div>
              <IssueTable items={items} selected={selected} onToggle={toggle} onOpen={open} />
            </>
          )}
          {loading && <p role="status">Cargando…</p>}
          {!loading && items.length === 0 && (active !== "ruta_inexistente" || scanned) && <p className="empty">Sin problemas de este tipo.</p>}

          {items.length > 0 && (
            <fieldset className="maint-batch" aria-label="Acción en lote" disabled={readOnly}>
              <legend>Acción en lote sobre la selección</legend>
              <div className="inline-row maint-controls">
                <label>
                  Acción{" "}
                  <select
                    aria-label="Tipo de acción"
                    value={formKind}
                    onChange={(e) => {
                      setFormKind(e.target.value as FormKind);
                      setFormValue("");
                      dropPreview();
                    }}
                  >
                    {(Object.keys(FORM_LABEL) as FormKind[]).map((k) => (
                      <option key={k} value={k}>
                        {FORM_LABEL[k]}
                      </option>
                    ))}
                  </select>
                </label>
                {isSelectKind ? (
                  <label>
                    Valor{" "}
                    <select
                      aria-label="Valor"
                      value={formValue}
                      onChange={(e) => {
                        setFormValue(e.target.value);
                        dropPreview();
                      }}
                    >
                      <option value="">Elegir…</option>
                      {withCurrent(valueList, formValue).map((v) => (
                        <option key={v} value={v}>
                          {v}
                        </option>
                      ))}
                    </select>
                  </label>
                ) : (
                  <label>
                    Tag{" "}
                    <input
                      type="text"
                      aria-label="Tag"
                      value={formValue}
                      maxLength={100}
                      onChange={(e) => {
                        setFormValue(e.target.value);
                        dropPreview();
                      }}
                    />
                  </label>
                )}
                <button type="button" disabled={batchOff || !action} onClick={() => action && void doPreview(action)}>
                  Vista previa
                </button>
                {active === "titulo_ry" && (
                  <button type="button" disabled={batchOff} onClick={() => void doPreview({ kind: "fix_ry_titles" })}>
                    Proponer títulos
                  </button>
                )}
              </div>
              {tooMany && <p className="field-error">Selecciona como máximo {MAX_IDS} juegos por lote.</p>}
            </fieldset>
          )}
          {readOnly && items.length > 0 && <p className="note">{RO_NOTE}</p>}
        </section>
      )}

      {previewing && (
        <p role="status" aria-live="polite">
          Calculando vista previa…
        </p>
      )}
      {preview && (
        <section aria-label="Vista previa del lote" className="maint-box">
          <h3>Vista previa: {describeAction(preview.req.action)}</h3>
          <p role="status">
            {preview.data.changes.length} cambios · {preview.data.skipped.length} omitidos
          </p>
          {preview.data.changes.length > 0 && (
            <div className="maint-scroll">
              <table aria-label="Cambios propuestos" className="maint-table">
                <thead>
                  <tr>
                    <th>
                      <input type="checkbox" aria-label="Marcar o desmarcar todos los cambios" checked={nChosen === preview.data.changes.length} onChange={toggleAllChanges} />
                    </th>
                    <th>Título</th>
                    <th>Antes</th>
                    <th>Después</th>
                  </tr>
                </thead>
                <tbody>
                  {preview.data.changes.slice(0, PREVIEW_CAP).map((c) => (
                    <tr key={c.id}>
                      <td>
                        <input type="checkbox" aria-label={`Aplicar a ${c.titulo}`} checked={!excluded.has(c.id)} onChange={() => toggleChange(c.id)} />
                      </td>
                      <td>{c.titulo}</td>
                      <td>{c.before || "—"}</td>
                      <td>{c.after || "—"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          {preview.data.changes.length > PREVIEW_CAP && <p className="note">Se muestran los primeros {PREVIEW_CAP} cambios; se aplicarán todos los marcados.</p>}
          {preview.data.skipped.length > 0 && (
            <ul aria-label="Omitidos en la vista previa">
              {preview.data.skipped.map((s) => (
                <li key={s.id}>
                  {titleOf(s.id)}: {s.reason}
                </li>
              ))}
            </ul>
          )}
          <div className="dialog-actions">
            <button type="button" onClick={dropPreview}>
              Descartar vista previa
            </button>
            <button type="button" className="primary" disabled={readOnly || working || nChosen === 0} onClick={() => setConfirm("apply")}>
              Aplicar {nChosen} cambios
            </button>
          </div>
        </section>
      )}
      {result && (
        <section aria-label="Resumen del lote" className="maint-box">
          <p role="status">
            Cambiados: {result.changed} · omitidos: {result.skipped.length}
          </p>
          {result.skipped.length > 0 && (
            <ul aria-label="Omitidos">
              {result.skipped.map((s) => (
                <li key={s.id}>
                  {titleOf(s.id)}: {s.reason}
                </li>
              ))}
            </ul>
          )}
          <Warn message={result.warning} />
          {result.backupPath && (
            <p>
              Backup: <span className="path-text">{result.backupPath}</span>
            </p>
          )}
        </section>
      )}

      {confirm === "apply" && preview && (
        <ConfirmDialog title="¿Aplicar el lote?" confirmLabel={`Aplicar ${nChosen} cambios`} busy={working} onCancel={() => setConfirm(null)} onConfirm={() => void doApply()}>
          <p>
            Se creará un backup y se modificarán {nChosen} juegos ({describeAction(preview.req.action)}). Podrás deshacer el último lote desde el historial.
          </p>
        </ConfirmDialog>
      )}
      {confirm === "index" && (
        <ConfirmDialog title="¿Reconstruir el índice de búsqueda?" confirmLabel="Reconstruir índice" busy={working} onCancel={() => setConfirm(null)} onConfirm={() => void doRebuild()}>
          <p>Se creará un backup y se regenerará el índice de búsqueda a partir de los juegos. No se modifican los datos de los juegos.</p>
        </ConfirmDialog>
      )}
      {confirm === "orphans" && (
        <ConfirmDialog title="¿Eliminar las tags huérfanas?" confirmLabel="Eliminar tags huérfanas" danger busy={working} onCancel={() => setConfirm(null)} onConfirm={() => void doDeleteOrphans()}>
          <p>Se eliminarán {orphans?.length ?? 0} tags que no están asignadas a ningún juego. Puedes deshacerlo desde el historial de lotes.</p>
        </ConfirmDialog>
      )}
    </>
  );
}

/* ───────────────────────── Batch history ───────────────────────── */

function BatchesPanel({ readOnly, onChanged, onBusy }: { readOnly: boolean; onChanged: () => void; onBusy: (b: boolean) => void }) {
  const [entries, setEntries] = useState<BatchEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [undo, setUndo] = useState<UndoBatchResult | null>(null);
  const running = useRef(false);
  const load = useCallback(() => {
    listBatches().then(setEntries, (e) => setError(normalizeError(e).message));
  }, []);
  useEffect(load, [load]);
  useEffect(() => {
    onBusy(busy);
  }, [busy, onBusy]);
  useEffect(() => () => onBusy(false), [onBusy]);

  const last = entries?.[0];
  const canUndo = !!last && !last.undone && !readOnly;

  const doUndo = async () => {
    if (running.current) return;
    running.current = true;
    setBusy(true);
    setUndo(null);
    setError(null);
    try {
      setUndo(await undoLastBatch());
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
    <section aria-label="Historial de lotes">
      <Err message={error} />
      {entries && entries.length === 0 && <p className="empty">Aún no hay lotes.</p>}
      {entries && entries.length > 0 && (
        <ul aria-label="Lotes anteriores">
          {entries.map((b) => (
            <li key={b.id}>
              {new Date(b.timestamp).toLocaleString()} · {b.description} · {b.changed} cambios{b.undone ? " · deshecho" : ""}
            </li>
          ))}
        </ul>
      )}
      {readOnly && <p className="note">Modo solo lectura: no se puede deshacer.</p>}
      <button type="button" disabled={!canUndo || busy} onClick={() => setConfirm(true)}>
        Deshacer último lote
      </button>
      {undo && (
        <div role="status">
          <p>
            Restaurados: {undo.restored} · conservados: {undo.kept.length}
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
          <Warn message={undo.warning} />
          {undo.backupPath && (
            <p>
              Backup: <span className="path-text">{undo.backupPath}</span>
            </p>
          )}
        </div>
      )}
      {confirm && (
        <ConfirmDialog title="¿Deshacer el último lote?" confirmLabel="Deshacer lote" danger busy={busy} onCancel={() => setConfirm(false)} onConfirm={() => void doUndo()}>
          <p>Se restaurarán los valores anteriores de los juegos del último lote que no hayan sido modificados desde entonces. Los modificados se conservarán.</p>
        </ConfirmDialog>
      )}
    </section>
  );
}
