import { useId, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { normalizeError, setDbPath } from "../api/client";
import type { LibraryInfo } from "../api/types";
import logo from "../assets/logo.webp";

interface Props {
  /** Why the library could not be opened (e.g. "no database configured"). */
  message: string | null;
  /** The database was opened: reload everything. */
  onOpened: (info: LibraryInfo) => void;
}

/** Shown instead of the library when no (valid) database is configured. */
export default function FirstRunScreen({ message, onOpened }: Props) {
  const uid = useId();
  const [path, setPath] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const pick = async () => {
    setError(null);
    try {
      const p = await open({ multiple: false, directory: false, filters: [{ name: "SQLite", extensions: ["db", "sqlite", "sqlite3"] }] });
      if (typeof p === "string" && p) setPath(p);
    } catch (e) {
      setError(normalizeError(e).message);
    }
  };
  const submit = async () => {
    const p = path.trim();
    if (!p || busy) return;
    setBusy(true);
    setError(null);
    try {
      onOpened(await setDbPath(p));
    } catch (e) {
      setError(normalizeError(e).message);
      setBusy(false);
    }
  };

  return (
    <main className="first-run" aria-labelledby={`${uid}-t`}>
      <img className="app-logo" src={logo} alt="" width={64} height={64} />
      <h1 id={`${uid}-t`}>Elegir base de datos</h1>
      <p>RPG Maker Library trabaja con un archivo de base de datos SQLite local (por ejemplo <code>juegos.db</code>). Elige el archivo para empezar; nada se envía fuera de tu equipo.</p>
      {message && <p className="cell-muted">{message}</p>}
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <div className="form-row">
          <label htmlFor={`${uid}-path`}>Ruta de la base de datos</label>
          <div className="inline-row">
            <input
              id={`${uid}-path`}
              type="text"
              value={path}
              autoFocus
              placeholder="Ruta absoluta a un archivo .db existente"
              aria-invalid={error ? true : undefined}
              aria-describedby={error ? `${uid}-err` : undefined}
              onChange={(e) => setPath(e.target.value)}
            />
            <button type="button" onClick={() => void pick()} disabled={busy}>
              Elegir archivo…
            </button>
            <button type="submit" className="primary" disabled={busy || !path.trim()}>
              Abrir
            </button>
          </div>
        </div>
        {error && (
          <p className="field-error" id={`${uid}-err`} role="alert">
            {error}
          </p>
        )}
      </form>
    </main>
  );
}
