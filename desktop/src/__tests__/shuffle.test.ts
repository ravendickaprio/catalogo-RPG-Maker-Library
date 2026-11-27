import { describe, it, expect } from "vitest";
import { shuffle, takeRandom } from "../lib/shuffle";
import { FAVORITE_TAG, findFavoriteTag, isFavorite, toggleFavoriteTags } from "../lib/favorites";

describe("shuffle", () => {
  it("is a permutation and does not mutate the input", () => {
    const src = Array.from({ length: 50 }, (_, i) => i);
    const copy = [...src];
    const out = shuffle(src);
    expect(src).toEqual(copy);
    expect(out).toHaveLength(50);
    expect([...out].sort((a, b) => a - b)).toEqual(src);
  });
  it("uses the injected rng deterministically", () => {
    expect(shuffle([1, 2, 3, 4, 5], () => 0)).toEqual([2, 3, 4, 5, 1]);
    expect(shuffle([1, 2, 3], () => 0.999)).toEqual([1, 2, 3]);
  });
  it("takeRandom takes N distinct items (or all when fewer)", () => {
    const src = Array.from({ length: 30 }, (_, i) => i);
    const out = takeRandom(src, 20);
    expect(out).toHaveLength(20);
    expect(new Set(out).size).toBe(20);
    expect(out.every((x) => src.includes(x))).toBe(true);
    expect(takeRandom([1, 2], 20)).toHaveLength(2);
    expect(takeRandom([1, 2], 0)).toEqual([]);
  });
});

describe("favorites helpers", () => {
  it("toggles case-insensitively and keeps other tags", () => {
    expect(toggleFavoriteTags(["a", "b"])).toEqual({ tags: ["a", "b", FAVORITE_TAG], favorite: true });
    expect(toggleFavoriteTags(["a", "FAVORITO", "b"])).toEqual({ tags: ["a", "b"], favorite: false });
    expect(isFavorite(["favorito"])).toBe(true);
    expect(isFavorite(["otro"])).toBe(false);
  });
  it("finds the stored spelling", () => {
    expect(findFavoriteTag([{ value: "x" }, { value: "favorito" }])).toBe("favorito");
    expect(findFavoriteTag([{ value: "x" }])).toBeNull();
    expect(findFavoriteTag(undefined)).toBeNull();
  });
});
