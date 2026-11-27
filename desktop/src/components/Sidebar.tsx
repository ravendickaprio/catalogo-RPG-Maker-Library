import type { LibraryInfo } from "../api/types";

interface Props {
  info: LibraryInfo | null;
  estado: string;
  idioma: string;
  tag: string;
  open: boolean;
  onEstado: (v: string) => void;
  onIdioma: (v: string) => void;
  onTag: (v: string) => void;
  onAll: () => void;
  onClose: () => void;
}

interface ItemProps {
  label: string;
  count: number;
  active: boolean;
  disabled?: boolean;
  onClick: () => void;
}

function Item({ label, count, active, disabled, onClick }: ItemProps) {
  return (
    <li>
      <button
        type="button"
        className={"side-item" + (active ? " active" : "")}
        aria-pressed={active}
        aria-disabled={disabled || undefined}
        disabled={disabled}
        title={disabled ? "Los juegos sin valor no se pueden filtrar" : undefined}
        onClick={onClick}
      >
        <span className="side-label">{label}</span>
        <span className="side-count">{count}</span>
      </button>
    </li>
  );
}

export default function Sidebar({ info, estado, idioma, tag, open, onEstado, onIdioma, onTag, onAll, onClose }: Props) {
  if (!open) return null;
  const noFilter = !estado && !idioma && !tag;
  // Clicking the active value again clears that filter.
  const toggle = (cur: string, set: (v: string) => void) => (v: string) => set(cur === v ? "" : v);
  return (
    <>
      <div className="side-scrim" onClick={onClose} aria-hidden="true" />
      <nav className="sidebar" aria-label="Filtros de la biblioteca">
        <section>
          <h2 className="side-h">Biblioteca</h2>
          <ul>
            <Item label="Todos" count={info?.total ?? 0} active={noFilter} onClick={onAll} />
          </ul>
        </section>
        <section aria-label="Estado">
          <h2 className="side-h">Estado</h2>
          <ul>
            {info?.estados.map((s) => (
              <Item
                key={s.value}
                label={s.value || "(sin estado)"}
                count={s.count}
                active={!!s.value && estado === s.value}
                disabled={!s.value}
                onClick={() => toggle(estado, onEstado)(s.value)}
              />
            ))}
          </ul>
        </section>
        <section aria-label="Idioma">
          <h2 className="side-h">Idioma</h2>
          <ul>
            {info?.idiomas.map((s) => (
              <Item
                key={s.value}
                label={s.value || "(sin idioma)"}
                count={s.count}
                active={!!s.value && idioma === s.value}
                disabled={!s.value}
                onClick={() => toggle(idioma, onIdioma)(s.value)}
              />
            ))}
          </ul>
        </section>
        {!!info?.tags.length && (
          <section aria-label="Etiquetas">
            <h2 className="side-h">Etiquetas</h2>
            <ul>
              {info.tags.map((s) => (
                <Item key={s.value} label={s.value} count={s.count} active={tag === s.value} onClick={() => toggle(tag, onTag)(s.value)} />
              ))}
            </ul>
          </section>
        )}
      </nav>
    </>
  );
}
