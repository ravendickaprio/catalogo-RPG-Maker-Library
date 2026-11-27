import { useId, type ReactNode } from "react";
import { useDialog } from "../hooks/useDialog";

interface Props {
  title: string;
  children: ReactNode;
  confirmLabel: string;
  cancelLabel?: string;
  /** Styles the confirm button as dangerous and focuses Cancel first (default). */
  danger?: boolean;
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/** Reusable accessible confirmation (role=alertdialog). Focus starts on the safe option. */
export default function ConfirmDialog({ title, children, confirmLabel, cancelLabel = "Cancelar", danger, busy, onConfirm, onCancel }: Props) {
  const titleId = useId();
  const descId = useId();
  const { ref, onKeyDown } = useDialog(onCancel, "[data-autofocus]");
  return (
    <div className="backdrop confirm-backdrop">
      <div
        ref={ref}
        className="dialog confirm"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descId}
        tabIndex={-1}
        onKeyDown={onKeyDown}
      >
        <h2 id={titleId}>{title}</h2>
        <div id={descId}>{children}</div>
        <div className="dialog-actions">
          <button type="button" data-autofocus onClick={onCancel}>
            {cancelLabel}
          </button>
          <button type="button" className={danger ? "danger" : "primary"} disabled={busy} onClick={onConfirm}>
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
