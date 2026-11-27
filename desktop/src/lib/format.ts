/** Shared date/number formatting (Spanish locale regardless of the OS/WebView locale). */
export function formatDateTime(iso: string | number | Date): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return String(iso);
  return d.toLocaleString("es-ES", { dateStyle: "short", timeStyle: "short" });
}

export function formatMs(ms: number): string {
  return `${ms.toLocaleString("es-ES", { minimumFractionDigits: 1, maximumFractionDigits: 1 })} ms`;
}
