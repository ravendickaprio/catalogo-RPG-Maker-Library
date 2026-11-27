import { useId, useState } from "react";
import { useDialog } from "../hooks/useDialog";
import { PREF_WELCOME, readPref, writePref } from "../lib/storage";
import portada from "../assets/portada.webp";

/** Whether the welcome screen should open at startup (default: yes). */
export const welcomeOnStartup = () => readPref(PREF_WELCOME) !== "off";

interface Props {
  /** Library size once loaded; `null` while the library is still loading. */
  total: number | null;
  error: string | null;
  onClose: () => void;
}

/** Startup cover (`imgs/portada.jpg`). Purely visual: never blocks loading nor touches data. */
export default function WelcomeScreen({ total, error, onClose }: Props) {
  const uid = useId();
  const [show, setShow] = useState(welcomeOnStartup);
  const { ref, onKeyDown } = useDialog(onClose, "button.primary");
  const toggle = (v: boolean) => {
    setShow(v);
    writePref(PREF_WELCOME, v ? "on" : "off");
  };
  return (
    <div className="backdrop welcome-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={ref} className="dialog welcome" role="dialog" aria-modal="true" aria-labelledby={`${uid}-t`} tabIndex={-1} onKeyDown={onKeyDown}>
        <img className="welcome-img" src={portada} alt="" draggable={false} />
        <h2 id={`${uid}-t`} className="sr-only">
          Bienvenida a RPG Maker Library
        </h2>
        <p className="welcome-status" aria-live="polite">
          {error ? `No se pudo abrir la biblioteca: ${error}` : total == null ? "Cargando biblioteca…" : `${total} juegos en la biblioteca`}
        </p>
        <div className="welcome-actions">
          <label>
            <input type="checkbox" checked={show} onChange={(e) => toggle(e.target.checked)} /> Mostrar al iniciar
          </label>
          <button type="button" className="primary" onClick={onClose}>
            Empezar
          </button>
        </div>
      </div>
    </div>
  );
}
