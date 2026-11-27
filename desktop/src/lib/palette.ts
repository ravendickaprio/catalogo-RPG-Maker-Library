export interface PaletteAction {
  id: string;
  label: string;
  run: () => void;
}

const fold = (s: string) => s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase().trim();

/** Case/accent-insensitive; every whitespace-separated term must appear in the label. */
export function filterActions(actions: PaletteAction[], query: string): PaletteAction[] {
  const terms = fold(query).split(/\s+/).filter(Boolean);
  if (!terms.length) return actions;
  return actions.filter((a) => {
    const l = fold(a.label);
    return terms.every((t) => l.includes(t));
  });
}
