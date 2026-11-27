import { useCallback, useEffect, useId, useState } from "react";
import { createBackup, getMigrationStatus, getSessionInfo, normalizeError, runMigrations, setDbPath, setReadOnly } from "../api/client";
import type { LibraryInfo, MigrationResult, MigrationStatus, SessionInfo } from "../api/types";
import { useDialog } from "../hooks/useDialog";
import ConfirmDialog from "./ConfirmDialog";
import BackupsSection from "./BackupsSection";

interface Props {
  session: SessionInfo | null;
  onClose: () => void;
  /** The database was reopened (new path): reload everything. */
  onDbChanged: (info: LibraryInfo) => void;
  /** The session info changed (read-only toggle, backup). */
  onSessionChanged: (s: SessionInfo) => void;
  /** Migrations were applied: data and counts may have changed. */
  onMigrated: () => void;
  /** A backup was restored: refresh everything like a DB switch. */
  onRestored: () => void;
}

export default function SettingsDialog({ session, onClose, onDbChanged, onSessionChanged, onMigrated, onRestored }: Props) {
  const { ref, onKeyDown } = useDialog(onClose, "input");
  const uid = useId();
  const [path, setPath] = useState("");
  const [pathError, setPathError] = useState<string | null>(null);
  const [pathOk, setPathOk] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [backupsTick, setBackupsTick] = useState(0);
  const [modeError, setModeError] = useState<string | null>(null);
  const [backup, setBackup] = useState<{ path?: string; error?: string } | null>(null);
  const [status, setStatus] = useState<MigrationStatus | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [confirmMig, setConfirmMig] = useState(false);
  const [migResult, setMigResult] = useState<MigrationResult | null>(null);
  const [migError, setMigError] = useState<string | null>(null);

  const loadStatus = useCallback(() => {
    getMigrationStatus().then(
      (s) => {
        setStatus(s);
        setStatusError(null);
      },
      (e) => setStatusError(normalizeError(e).message),
    );
  }, []);
  useEffect(loadStatus, [loadStatus]);

  const readOnly = session?.readOnly ?? true;

  const openPath = async () => {
    const p = path.trim();
    if (!p) return;
    setBusy(true);
    setPathError(null);
    setPathOk(null);
    try {
      const info = await setDbPath(p);
      setPathOk(`Base abierta: ${info.dbFileName}`);
      onDbChanged(info);
      getSessionInfo().then(onSessionChanged, () => undefined);
      loadStatus();
    } catch (e) {
      setPathError(normalizeError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const toggleReadOnly = async (value: boolean) => {
    setModeError(null);
    try {
      onSessionChanged(await setReadOnly(value));
    } catch (e) {
      setModeError(normalizeError(e).message);
    }
  };

  const doBackup = async () => {
    setBusy(true);
    setBackup(null);
    try {
      const r = await createBackup();
      setBackup({ path: r.path });
      setBackupsTick((n) => n + 1);
      getSessionInfo().then(onSessionChanged, () => undefined);
    } catch (e) {
      setBackup({ error: normalizeError(e).message });
    } finally {
      setBusy(false);
    }
  };

  const doMigrate = async () => {
    setBusy(true);
    setMigError(null);
    setMigResult(null);
    try {
      const r = await runMigrations();
      setMigResult(r);
      setConfirmMig(false);
      loadStatus();
      onMigrated();
    } catch (e) {
      setConfirmMig(false);
      setMigError(normalizeError(e).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <div
        className="backdrop"
        onMouseDown={(e) => {
          if (e.target === e.currentTarget) onClose();
        }}
      >
        <div ref={ref} className="dialog settings" role="dialog" aria-modal="true" aria-labelledby={`${uid}-t`} tabIndex={-1} onKeyDown={onKeyDown}>
          <h2 id={`${uid}-t`}>Ajustes</h2>

          <section aria-labelledby={`${uid}-s1`}>
            <h3 id={`${uid}-s1`}>Sesión</h3>
            {session ? (
              <dl className="fields">
                <dt>Base de datos</dt>
                <dd>{session.dbFileName}</dd>
                <dt>Carpeta</dt>
                <dd className="path-text">{session.dbDir}</dd>
                <dt>Modo</dt>
                <dd>{session.readOnly ? "solo lectura" : "edición"}</dd>
                <dt>Último backup</dt>
                <dd className="path-text">{session.lastBackup ?? "—"}</dd>
              </dl>
            ) : (
              <p className="empty">Información de sesión no disponible.</p>
            )}
            <div className="form-row">
              <label htmlFor={`${uid}-path`}>Ruta de la base de datos</label>
              <div className="inline-row">
                <input
                  id={`${uid}-path`}
                  type="text"
                  value={path}
                  placeholder="Ruta absoluta a un archivo .db existente"
                  aria-invalid={pathError ? true : undefined}
                  aria-describedby={pathError ? `${uid}-path-err` : undefined}
                  onChange={(e) => setPath(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      void openPath();
                    }
                  }}
                />
                <button type="button" disabled={busy || !path.trim()} onClick={() => void openPath()}>
                  Abrir
                </button>
              </div>
              {pathError && (
                <p className="field-error" id={`${uid}-path-err`} role="alert">
                  {pathError}
                </p>
              )}
              {pathOk && <p role="status">{pathOk}</p>}
            </div>
            <label className="check">
              <input type="checkbox" checked={readOnly} disabled={!session} onChange={(e) => void toggleReadOnly(e.target.checked)} /> Modo solo lectura
            </label>
            {modeError && (
              <p className="field-error" role="alert">
                {modeError}
              </p>
            )}
          </section>

          <section aria-labelledby={`${uid}-s2`}>
            <h3 id={`${uid}-s2`}>Backup</h3>
            <button type="button" disabled={busy} onClick={() => void doBackup()}>
              Crear backup ahora
            </button>
            {backup?.path && <p role="status">Backup creado: <span className="path-text">{backup.path}</span></p>}
            {backup?.error && (
              <p className="field-error" role="alert">
                {backup.error}
              </p>
            )}
          </section>

          <BackupsSection
            headingId={`${uid}-s4`}
            readOnly={readOnly}
            reloadToken={backupsTick}
            onRestored={() => {
              onRestored();
              getSessionInfo().then(onSessionChanged, () => undefined);
              loadStatus();
            }}
          />

          <section aria-labelledby={`${uid}-s3`}>
            <h3 id={`${uid}-s3`}>Migraciones</h3>
            {statusError && (
              <p className="field-error" role="alert">
                {statusError}
              </p>
            )}
            {status && (
              <>
                <p>
                  Versión actual: {status.current} · última disponible: {status.latest}
                </p>
                {status.pending.length > 0 ? (
                  <ul aria-label="Migraciones pendientes">
                    {status.pending.map((m) => (
                      <li key={m.version}>
                        v{m.version}: {m.description}
                      </li>
                    ))}
                  </ul>
                ) : (
                  <p>No hay migraciones pendientes.</p>
                )}
                <button
                  type="button"
                  disabled={busy || status.pending.length === 0 || readOnly}
                  title={readOnly ? "Desactiva el modo solo lectura para aplicar migraciones" : undefined}
                  onClick={() => setConfirmMig(true)}
                >
                  Aplicar migraciones
                </button>
              </>
            )}
            {migResult && (
              <p role="status">
                Migraciones aplicadas: {migResult.applied.join(", ") || "ninguna"}. Backup: <span className="path-text">{migResult.backupPath}</span>
              </p>
            )}
            {migError && (
              <p className="field-error" role="alert">
                {migError}
              </p>
            )}
          </section>

          <div className="dialog-actions">
            <button type="button" className="primary" onClick={onClose}>
              Cerrar
            </button>
          </div>
        </div>
      </div>
      {confirmMig && status && (
        <ConfirmDialog title="¿Aplicar migraciones?" confirmLabel="Aplicar migraciones" busy={busy} onCancel={() => setConfirmMig(false)} onConfirm={() => void doMigrate()}>
          <p>Se creará primero un backup de la base de datos y después se aplicarán {status.pending.length} migración(es) en una transacción.</p>
        </ConfirmDialog>
      )}
    </>
  );
}
