import { useId, useRef, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { exportLibrary, normalizeError } from "../api/client";
import type { ExportFormat } from "../api/types";
import { useDialog } from "../hooks/useDialog";

interface Props {
  onClose: () => void;
  onDone: (message: string) => void;
}

const FILTERS: Record<ExportFormat, { name: string; extensions: string[] }[]> = {
  csv: [{ name: "CSV", extensions: ["csv"] }],
  json: [{ name: "JSON", extensions: ["json"] }],
};

function stamp(d = new Date()): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}`;
}
const baseName = (p: string) => p.split(/[\\/]/).pop() || p;

export default function ExportDialog({ onClose, onDone }: Props) {
  const uid = useId();
  const { ref, onKeyDown } = useDialog(onClose, "input");
  const [format, setFormat] = useState<ExportFormat>("csv");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const guard = useRef(false);

  const run = async () => {
    if (guard.current) return;
    guard.current = true;
    setBusy(true);
    setError(null);
    try {
      const path = await save({ defaultPath: `biblioteca-${stamp()}.${format}`, filters: FILTERS[format] });
      if (!path) return;
      const r = await exportLibrary(path, format);
      onDone(`Exportados ${r.rows} juegos a ${baseName(r.path)}`);
      onClose();
    } catch (e) {
      setError(normalizeError(e).message);
    } finally {
      guard.current = false;
      setBusy(false);
    }
  };

  return (
    <div className="backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={ref} className="dialog" role="dialog" aria-modal="true" aria-labelledby={`${uid}-t`} tabIndex={-1} onKeyDown={onKeyDown}>
        <h2 id={`${uid}-t`}>Exportar biblioteca</h2>
        <fieldset>
          <legend>Formato</legend>
          <label className="check">
            <input type="radio" name={`${uid}-f`} checked={format === "csv"} onChange={() => setFormat("csv")} /> CSV
          </label>
          <label className="check">
            <input type="radio" name={`${uid}-f`} checked={format === "json"} onChange={() => setFormat("json")} /> JSON
          </label>
        </fieldset>
        <p className="cell-muted">Se exportan los datos de los juegos (sin portadas). No modifica la base de datos.</p>
        {error && (
          <p className="field-error" role="alert">
            {error}
          </p>
        )}
        <div className="dialog-actions">
          <button type="button" onClick={onClose}>
            Cancelar
          </button>
          <button type="button" className="primary" disabled={busy} onClick={() => void run()}>
            Exportar…
          </button>
        </div>
      </div>
    </div>
  );
}
