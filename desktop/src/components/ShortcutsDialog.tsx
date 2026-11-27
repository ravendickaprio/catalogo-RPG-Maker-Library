import { useId } from "react";
import { useDialog } from "../hooks/useDialog";
import { SHORTCUTS, formatShortcut } from "../lib/shortcuts";

/** Keyboard help generated from the same SHORTCUTS table the handlers use. */
export default function ShortcutsDialog({ onClose }: { onClose: () => void }) {
  const uid = useId();
  const { ref, onKeyDown } = useDialog(onClose, "button");
  return (
    <div className="backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={ref} className="dialog" role="dialog" aria-modal="true" aria-labelledby={`${uid}-t`} tabIndex={-1} onKeyDown={onKeyDown}>
        <h2 id={`${uid}-t`}>Atajos de teclado</h2>
        <table className="maint-table" aria-label="Atajos de teclado">
          <thead>
            <tr>
              <th scope="col">Atajo</th>
              <th scope="col">Acción</th>
              <th scope="col">Dónde</th>
            </tr>
          </thead>
          <tbody>
            {SHORTCUTS.map((s) => (
              <tr key={s.id}>
                <td>
                  <kbd>{formatShortcut(s)}</kbd>
                </td>
                <td>{s.description}</td>
                <td className="cell-muted">{s.scope}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <div className="dialog-actions">
          <button type="button" onClick={onClose}>
            Cerrar
          </button>
        </div>
      </div>
    </div>
  );
}
