const STRATEGY_HELP =
  "Estrategia de búsqueda. FTS: índice de texto completo (rápido). LIKE: búsqueda por coincidencia parcial. —: sin texto de búsqueda.";

export function strategyLabel(s: string | null): string {
  if (s === "fts") return "FTS";
  if (s === "like") return "LIKE";
  return "—";
}

interface Props {
  loading: boolean;
  total: number;
  count: number;
  elapsedMs: number | null;
  strategy: string | null;
}

export default function StatusLine({ loading, total, count, elapsedMs, strategy }: Props) {
  return (
    <div className="statusline" role="status" aria-live="polite">
      {loading ? (
        "Cargando…"
      ) : (
        <>
          <span>
            {total} {total === 1 ? "resultado" : "resultados"}
          </span>
          {count < total && <span className="cell-muted"> · {count} cargados</span>}
        </>
      )}
      {elapsedMs != null && !loading && (
        <span className="cell-muted" title={STRATEGY_HELP}>
          {" "}
          · {elapsedMs.toFixed(1)} ms · {strategyLabel(strategy)}
        </span>
      )}
    </div>
  );
}
