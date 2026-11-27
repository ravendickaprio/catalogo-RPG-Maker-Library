export const CARD_MIN_W = 170;
export const GRID_GAP = 12;
export const GRID_PAD = 12;
export const CARD_INFO_H = 76;

export interface GridLayout {
  columns: number;
  cardWidth: number;
  rowHeight: number; // card height + vertical gap
}

/** Columns that fit `containerWidth` with cards of at least CARD_MIN_W; cards stretch to fill. */
export function computeGridLayout(containerWidth: number): GridLayout {
  const inner = Math.max(0, containerWidth - GRID_PAD * 2);
  const columns = Math.max(1, Math.floor((inner + GRID_GAP) / (CARD_MIN_W + GRID_GAP)));
  const cardWidth = Math.max(CARD_MIN_W, (inner - GRID_GAP * (columns - 1)) / columns);
  const cardHeight = Math.round((cardWidth * 4) / 3) + CARD_INFO_H;
  return { columns, cardWidth, rowHeight: cardHeight + GRID_GAP };
}

/** Next index after an arrow key in a grid, clamped; null if the key is not a grid move. */
export function moveInGrid(key: string, index: number, columns: number, count: number): number | null {
  if (count <= 0) return null;
  let next: number;
  switch (key) {
    case "ArrowRight": next = index + 1; break;
    case "ArrowLeft": next = index - 1; break;
    case "ArrowDown": next = index + columns; break;
    case "ArrowUp": next = index - columns; break;
    case "Home": next = 0; break;
    case "End": next = count - 1; break;
    default: return null;
  }
  if (index < 0) return key === "ArrowLeft" || key === "ArrowUp" || key === "End" ? count - 1 : 0;
  return Math.min(count - 1, Math.max(0, next));
}
