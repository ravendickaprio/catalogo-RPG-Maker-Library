import { useEffect, useId, useState } from "react";
import { deletePagePassword, normalizeError, pagePasswordsStatus, setPagePassword } from "../api/client";
import { useDialog } from "../hooks/useDialog";
import ConfirmDialog from "./ConfirmDialog";
import { PASSWORDS_CHANGED, PasswordActions } from "./PagePassword";

interface Props {
  /** Pages present in the library (vocabulary from the `pagina` column). */
  paginas: string[];
  onClose: () => void;
  onToast: (msg: string) => void;
}

/** Contraseñas de descompresión por página (ADR-019). Stored in the OS keyring, never in SQLite. */
export default function PasswordsDialog({ paginas, onClose, onToast }: Props) {
  const uid = useId();
  const { ref, onKeyDown } = useDialog(onClose);
  const pages = paginas.filter((p) => p.trim());
  const [has, setHas] = useState<Set<string> | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [value, setValue] = useState("");
  const [busy, setBusy] = useState(false);
  const [confirmDel, setConfirmDel] = useState<string | null>(null);

  const reload = () =>
    pagePasswordsStatus(pages).then(
      (r) => {
        setHas(new Set(r));
        setError(null);
      },
      (e) => setError(normalizeError(e).message),
    );
  useEffect(() => {
    void reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [paginas]);

  const act = async (fn: () => Promise<unknown>, msg: string) => {
    setBusy(true);
    try {
      await fn();
      window.dispatchEvent(new Event(PASSWORDS_CHANGED));
      onToast(msg);
      await reload();
      return true;
    } catch (e) {
      setError(normalizeError(e).message);
      return false;
    } finally {
      setBusy(false);
    }
  };

  const save = async (p: string) => {
    if (!value) return setError("Escribe la contraseña.");
    if (await act(() => setPagePassword(p, value), `Contraseña de «${p}» guardada`)) {
      setEditing(null);
      setValue("");
    }
  };

  return (
    <div className="backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={ref} className="dialog" role="dialog" aria-modal="true" aria-labelledby={`${uid}-t`} tabIndex={-1} onKeyDown={onKeyDown}>
        <h2 id={`${uid}-t`}>Contraseñas por página</h2>
        <p className="cell-muted">Se guardan en el almacén seguro del sistema (no en la base de datos) y se ocultan por defecto.</p>
        {error && (
          <p className="field-error" role="alert">
            {error}
          </p>
        )}
        {pages.length === 0 && <p className="empty">Ningún juego tiene página asignada.</p>}
        <ul className="pw-list" aria-busy={has === null}>
          {pages.map((p) => {
            const saved = has?.has(p) ?? false;
            return (
              <li key={p}>
                <strong>{p}</strong>
                {has !== null && <span className={saved ? "pw-badge saved" : "pw-badge"}>{saved ? "Guardada" : "Sin contraseña"}</span>}
                {editing === p ? (
                  <form
                    className="pw-form"
                    onSubmit={(e) => {
                      e.preventDefault();
                      void save(p);
                    }}
                  >
                    <input
                      type="password"
                      autoComplete="off"
                      autoFocus
                      aria-label={`Contraseña de ${p}`}
                      value={value}
                      maxLength={1024}
                      onChange={(e) => setValue(e.target.value)}
                    />
                    <button type="submit" className="primary" disabled={busy}>
                      Guardar
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        setEditing(null);
                        setValue("");
                      }}
                    >
                      Cancelar
                    </button>
                  </form>
                ) : (
                  <span className="pw-row-actions">
                    {saved && <PasswordActions pagina={p} onToast={onToast} />}
                    <button
                      type="button"
                      disabled={busy || has === null}
                      onClick={() => {
                        setEditing(p);
                        setValue("");
                      }}
                    >
                      {saved ? "Cambiar" : "Añadir"}
                    </button>
                    {saved && (
                      <button type="button" className="danger" disabled={busy} onClick={() => setConfirmDel(p)}>
                        Eliminar
                      </button>
                    )}
                  </span>
                )}
              </li>
            );
          })}
        </ul>
        <div className="dialog-actions">
          <button type="button" onClick={onClose}>
            Cerrar
          </button>
        </div>
      </div>
      {confirmDel && (
        <ConfirmDialog
          title={`¿Eliminar la contraseña de «${confirmDel}»?`}
          confirmLabel="Eliminar"
          danger
          busy={busy}
          onCancel={() => setConfirmDel(null)}
          onConfirm={() => void act(() => deletePagePassword(confirmDel), `Contraseña de «${confirmDel}» eliminada`).then(() => setConfirmDel(null))}
        >
          <p>Se borra del almacén del sistema. Los juegos no se modifican.</p>
        </ConfirmDialog>
      )}
    </div>
  );
}
