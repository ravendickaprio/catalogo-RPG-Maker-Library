import { describe, it, expect } from "vitest";
import { computeGridLayout, moveInGrid } from "../lib/gridLayout";

describe("gridLayout", () => {
  it("computes at least one column and grows with width", () => {
    expect(computeGridLayout(0).columns).toBe(1);
    expect(computeGridLayout(100).columns).toBe(1);
    expect(computeGridLayout(800).columns).toBe(4);
    expect(computeGridLayout(1200).columns).toBe(6);
    const l = computeGridLayout(800);
    expect(l.cardWidth).toBeGreaterThanOrEqual(170);
    expect(l.columns * l.cardWidth + 11 * (l.columns - 1) / 11 * 12).toBeLessThanOrEqual(800);
    expect(l.rowHeight).toBeGreaterThan(l.cardWidth * (4 / 3));
  });

  it("moves across the grid with clamping", () => {
    expect(moveInGrid("ArrowRight", 0, 4, 10)).toBe(1);
    expect(moveInGrid("ArrowLeft", 0, 4, 10)).toBe(0);
    expect(moveInGrid("ArrowDown", 1, 4, 10)).toBe(5);
    expect(moveInGrid("ArrowDown", 8, 4, 10)).toBe(9);
    expect(moveInGrid("ArrowUp", 2, 4, 10)).toBe(0);
    expect(moveInGrid("ArrowRight", -1, 4, 10)).toBe(0);
    expect(moveInGrid("a", 0, 4, 10)).toBeNull();
    expect(moveInGrid("ArrowDown", 0, 4, 0)).toBeNull();
  });
});
