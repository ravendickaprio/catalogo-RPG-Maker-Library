import { useCallback, useEffect, useRef, useState } from "react";
import { checkDatabase, listBackups, normalizeError, restoreBackup, verifyBackup } from "../api/client";
import type { BackupCheck, BackupEntry, RestoreResult } from "../api/types";
import { formatDateTime, formatSize } from "../lib/format";
import ConfirmDialog from "./ConfirmDialog";

interface Props {
  headingId: string;
  readOnly: boolean;
  /** Changes to force reloading the list (e.g. after creating a backup). */
  reloadToken: number;
  /** A restore succeeded: caller refreshes the whole app. */
  onRestored: (r: RestoreResult) => void;
}

function CheckSummary({ c }: { c: BackupCheck }) {
  return (
    <p role="status" className={c.ok ? undefined : "field-error"}>
      {c.ok ? "Verificación correcta" : "Verificación con problemas"} · integridad: {c.integrity} · claves foráneas: {c.foreignKeys} · índice de búsqueda:{" "}
      {c.ftsOk == null ? "n/d" : c.ftsOk ? "ok" : "desincronizado"} · juegos: {c.games} · esquema: {c.schemaOk ? "ok" : "incompleto"}
    </p>
  );
}

export default function BackupsSection({ headingId, readOnly, reloadToken, onRestored }: Props) {
  const [items, setItems] = useState<BackupEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [checks, setChecks] = useState<Record<string, BackupCheck | string>>({});
  const [current, setCurrent] = useState<BackupCheck | string | null>(null);
  const [confirm, setConfirm] = useState<BackupEntry | null>(null);
  const [restoring, setRestoring] = useState(false);
  const [restoreError, setRestoreError] = useState<string | null>(null);
  const [restored, setRestored] = useState<RestoreResult | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const guard = useRef(false);

  const load = useCallback(() => {
    listBackups().then(
      (l) => {
        setItems(l);
        setError(null);
      },
      (e) => setError(normalizeError(e).message),
    );
  }, []);
  useEffect(load, [load, reloadToken]);

  const verify = async (name: string) => {
    setBusy(name);
    try {
      setChecks((c) => ({ ...c, [name]: "" }));
      const r = await verifyBackup(name);
      setChecks((c) => ({ ...c, [name]: r }));
    } catch (e) {
      setChecks((c) => ({ ...c, [name]: normalizeError(e).message || "Error" }));
    } finally {
      setBusy(null);
    }
  };
  const verifyCurrent = async () => {
    setBusy("*");
    setCurrent(null);
    try {
      setCurrent(await checkDatabase());
    } catch (e) {
      setCurrent(normalizeError(e).message || "Error");
    } finally {
      setBusy(null);
    }
  };
  const doRestore = async () => {
    if (!confirm || guard.current) return;
    guard.current = true;
    setRestoring(true);
    setRestoreError(null);
    try {
      const r = await restoreBackup(confirm.fileName);
      setRestored(r);
      setConfirm(null);
      onRestored(r);
      load();
    } catch (e) {
      setConfirm(null);
      setRestoreError(normalizeError(e).message);
    } finally {
      guard.current = false;
      setRestoring(false);
    }
  };

  return (
    <section aria-labelledby={headingId}>
      <h3 id={headingId}>Copias de seguridad</h3>
      {error && (
        <p className="field-error" role="alert">
          {error}
        </p>
      )}
      {!error && items == null && <p className="empty">Cargando copias…</p>}
      {items && items.length === 0 && <p className="empty">Todavía no hay copias de seguridad.</p>}
      {items && items.length > 0 && (
        <ul aria-label="Copias de seguridad" className="backup-list">
          {items.map((b) => {
            const c = checks[b.fileName];
            return (
              <li key={b.fileName}>
                <div className="inline-row">
                  <span className="path-text">{b.fileName}</span>
                  <span className="cell-muted">
                    {formatDateTime(b.modified)} · {formatSize(b.size)}
                  </span>
                  {b.preRestore && <span className="badge badge-ro">pre-restauración</span>}
                  <button type="button" disabled={busy != null || restoring} aria-label={`Verificar ${b.fileName}`} onClick={() => void verify(b.fileName)}>
                    Verificar
                  </button>
                  <button
                    type="button"
                    className="danger"
                    disabled={readOnly || restoring}
                    title={readOnly ? "Desactiva el modo solo lectura para restaurar" : undefined}
                    aria-label={`Restaurar ${b.fileName}`}
                    onClick={() => setConfirm(b)}
                  >
                    Restaurar
                  </button>
                </div>
                {typeof c === "object" && <CheckSummary c={c} />}
                {typeof c === "string" && c && (
                  <p className="field-error" role="alert">
                    {c}
                  </p>
                )}
              </li>
            );
          })}
        </ul>
      )}
      <div className="inline-row">
        <button type="button" disabled={busy != null || restoring} onClick={() => void verifyCurrent()}>
          Verificar base actual
        </button>
      </div>
      {current && typeof current === "object" && <CheckSummary c={current} />}
      {typeof current === "string" && (
        <p className="field-error" role="alert">
          {current}
        </p>
      )}
      {restored && (
        <p role="status">
          Restaurado desde {restored.restoredFrom} ({restored.games} juegos). Copia previa: <span className="path-text">{restored.preRestoreBackup}</span>
        </p>
      )}
      {restored?.warning && (
        <p className="field-error" role="alert">
          {restored.warning}
        </p>
      )}
      {restoreError && (
        <p className="field-error" role="alert">
          {restoreError}
        </p>
      )}
      {confirm && (
        <ConfirmDialog title="¿Restaurar esta copia?" confirmLabel="Restaurar" danger busy={restoring} onCancel={() => setConfirm(null)} onConfirm={() => void doRestore()}>
          <p>
            Se sustituirá el contenido actual por «{confirm.fileName}». Antes se creará un backup de la base actual.
          </p>
        </ConfirmDialog>
      )}
    </section>
  );
}
