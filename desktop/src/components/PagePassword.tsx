import { useEffect, useState } from "react";
import { copyPagePassword, getPagePassword, normalizeError, pagePasswordsStatus } from "../api/client";

/** Fired by the Contraseñas dialog after a change, so open game details re-check their page. */
export const PASSWORDS_CHANGED = "rpglib:passwords-changed";

interface Props {
  pagina: string;
  onToast?: (msg: string) => void;
}

/** «Mostrar» / «Copiar» for a page password. Hidden by default; copying is done by the backend. */
export function PasswordActions({ pagina, onToast }: Props) {
  const [shown, setShown] = useState<string | null>(null);
  useEffect(() => setShown(null), [pagina]);
  const fail = (e: unknown) => onToast?.(normalizeError(e).message);
  const toggle = async () => {
    if (shown !== null) return setShown(null);
    try {
      const pw = await getPagePassword(pagina);
      if (pw === null) onToast?.(`«${pagina}» ya no tiene contraseña guardada`);
      else setShown(pw);
    } catch (e) {
      fail(e);
    }
  };
  const copy = async () => {
    try {
      await copyPagePassword(pagina);
      onToast?.(`Contraseña de «${pagina}» copiada`);
    } catch (e) {
      fail(e);
    }
  };
  return (
    <span className="pw-actions">
      <code className="pw-value">
        {shown ?? "••••••••"}
      </code>
      <button type="button" onClick={() => void toggle()} aria-pressed={shown !== null} aria-label={`${shown !== null ? "Ocultar" : "Mostrar"} contraseña de ${pagina}`}>
        {shown !== null ? "Ocultar" : "Mostrar"}
      </button>
      <button type="button" onClick={() => void copy()} aria-label={`Copiar contraseña de ${pagina}`}>
        Copiar
      </button>
    </span>
  );
}

/** Game detail row: appears only when the game's page has a saved password. */
export default function PagePassword({ pagina, onToast }: Props) {
  const [has, setHas] = useState(false);
  useEffect(() => {
    let live = true;
    const check = () => {
      if (pagina.trim())
        pagePasswordsStatus([pagina]).then(
          (r) => live && setHas(r.length > 0),
          () => undefined, // storage unavailable: the Contraseñas dialog explains it
        );
    };
    setHas(false);
    check();
    window.addEventListener(PASSWORDS_CHANGED, check);
    return () => {
      live = false;
      window.removeEventListener(PASSWORDS_CHANGED, check);
    };
  }, [pagina]);
  if (!has) return null;
  return (
    <div className="page-password">
      <span className="cell-muted">Contraseña de «{pagina}»</span>
      <PasswordActions pagina={pagina} onToast={onToast} />
    </div>
  );
}
