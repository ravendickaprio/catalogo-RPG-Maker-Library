import { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { getDashboard, normalizeError } from "../api/client";
import type { CountItem, Dashboard } from "../api/types";
import { formatMs } from "../lib/format";
import { useDialog } from "../hooks/useDialog";

export type DashboardFilter = { kind: "estado" | "idioma" | "tag"; value: string };

interface Props {
  onClose: () => void;
  /** Apply a filter in the main view (the host also closes the dialog). */
  onFilter: (f: DashboardFilter) => void;
}

const EMPTY = "(vacío)";
const pct = (n: number, total: number) => (total > 0 ? Math.round((n / total) * 1000) / 10 : 0);
const fmtPct = (n: number, total: number) => `${pct(n, total).toLocaleString("es")} %`;

/** One chart = one accessible table; the bar column is decorative (aria-hidden), the values are text. */
function BarTable({
  title,
  items,
  total,
  filterKind,
  onFilter,
  labelOf = (v: string) => v || EMPTY,
}: {
  title: string;
  items: CountItem[];
  total: number;
  filterKind?: DashboardFilter["kind"];
  onFilter: (f: DashboardFilter) => void;
  labelOf?: (v: string) => string;
}) {
  return (
    <section className="dash-card" aria-label={title}>
      <h3>{title}</h3>
      {items.length === 0 ? (
        <p className="cell-muted">Sin datos.</p>
      ) : (
        <table className="dash-table" aria-label={title}>
          <thead className="sr-only">
            <tr>
              <th scope="col">Valor</th>
              <th scope="col">Juegos</th>
              <th scope="col">Porcentaje</th>
              <th scope="col">Barra</th>
            </tr>
          </thead>
          <tbody>
            {items.map((it) => (
              <tr key={it.value}>
                <th scope="row" className="dash-label">
                  {filterKind && it.value ? (
                    <button type="button" className="link-btn" title={`Filtrar por ${labelOf(it.value)}`} onClick={() => onFilter({ kind: filterKind, value: it.value })}>
                      {labelOf(it.value)}
                    </button>
                  ) : (
                    labelOf(it.value)
                  )}
                </th>
                <td className="dash-num">{it.count}</td>
                <td className="dash-num dash-pct">{fmtPct(it.count, total)}</td>
                <td className="dash-bar-cell" aria-hidden="true">
                  {it.count > 0 && <div className="dash-bar" style={{ width: `max(2px, ${total > 0 ? (it.count / total) * 100 : 0}%)` }} />}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

export default function DashboardDialog({ onClose, onFilter }: Props) {
  const uid = useId();
  const [data, setData] = useState<Dashboard | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { ref, onKeyDown } = useDialog(onClose);

  const seq = useRef(0);
  const load = useCallback(() => {
    const mine = ++seq.current;
    setError(null);
    getDashboard().then(
      (d) => mine === seq.current && setData(d),
      (e) => mine === seq.current && setError(normalizeError(e).message),
    );
  }, []);
  useEffect(load, [load]);

  const pivot = useMemo(() => {
    if (!data) return null;
    const order = (list: CountItem[], extra: string[]) => {
      const known = list.map((i) => i.value);
      return [...known, ...extra.filter((v) => !known.includes(v))];
    };
    const estados = order(data.estados, data.estadoIdioma.map((r) => r.estado));
    const idiomas = order(data.idiomas, data.estadoIdioma.map((r) => r.idioma));
    const cell = new Map(data.estadoIdioma.map((r) => [`${r.estado}\u0000${r.idioma}`, r.count]));
    return { estados, idiomas, cell };
  }, [data]);

  const total = data?.total ?? 0;
  const tiles = data
    ? [
        { label: "Con portada", n: data.cobertura.conPortada },
        { label: "Con descripción", n: data.cobertura.conDescripcion },
        { label: "Con tags", n: data.cobertura.conTags },
        { label: "Con ruta", n: data.cobertura.conRuta },
      ]
    : [];

  return (
    <div className="backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={ref} className="dialog dash-dialog" role="dialog" aria-modal="true" aria-labelledby={`${uid}-t`} tabIndex={-1} onKeyDown={onKeyDown}>
        <h2 id={`${uid}-t`}>Estadísticas de la biblioteca</h2>
        {error && (
          <p className="error-text" role="alert">
            No se pudieron cargar las estadísticas: {error}{" "}
            <button type="button" onClick={load}>
              Reintentar
            </button>
          </p>
        )}
        {!data && !error && <p role="status">Calculando…</p>}
        {data && pivot && (
          <>
            <p className="dash-total" role="status">
              <strong>{data.total}</strong> juegos <span className="cell-muted dash-elapsed">· calculado en {formatMs(data.elapsedMs)}</span>
            </p>
            <ul className="dash-tiles" aria-label="Cobertura de datos">
              {tiles.map((t) => (
                <li key={t.label} className="dash-tile">
                  <span className="dash-tile-pct">{fmtPct(t.n, total)}</span>
                  <span className="dash-tile-label">{t.label}</span>
                  <span className="cell-muted">
                    {t.n} de {total}
                  </span>
                </li>
              ))}
            </ul>
            <div className="dash-grid">
              <BarTable title="Estados" items={data.estados} total={total} filterKind="estado" onFilter={onFilter} />
              <BarTable title="Idiomas" items={data.idiomas} total={total} filterKind="idioma" onFilter={onFilter} />
              <BarTable title="Tipos" items={data.tipos} total={total} onFilter={onFilter} />
              <BarTable title="Páginas" items={data.paginas} total={total} onFilter={onFilter} />
              <BarTable title="Etiquetas más usadas" items={data.topTags} total={total} filterKind="tag" onFilter={onFilter} />
              <BarTable title="Distribución de puntuaciones" items={data.puntuaciones} total={total} onFilter={onFilter} labelOf={(v) => v} />
            </div>
            <section className="dash-card" aria-label="Estado por idioma">
              <h3>Estado por idioma</h3>
              <div className="maint-scroll" role="region" aria-label="Tabla de estado por idioma (desplazable)" tabIndex={0}>
                <table className="maint-table dash-pivot" aria-label="Estado por idioma">
                  <thead>
                    <tr>
                      <th scope="col">Estado</th>
                      {pivot.idiomas.map((i) => (
                        <th key={i} scope="col" className="dash-num">
                          {i || EMPTY}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {pivot.estados.map((es) => (
                      <tr key={es}>
                        <th scope="row">{es || EMPTY}</th>
                        {pivot.idiomas.map((id) => (
                          <td key={id} className="dash-num">
                            {pivot.cell.get(`${es}\u0000${id}`) ?? 0}
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </section>
          </>
        )}
        <div className="dialog-actions">
          <button type="button" onClick={onClose}>
            Cerrar
          </button>
        </div>
      </div>
    </div>
  );
}
