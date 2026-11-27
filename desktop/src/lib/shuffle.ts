/** Fisher–Yates shuffle; returns a new array. `rng` returns [0,1) (injectable for tests). */
export function shuffle<T>(items: readonly T[], rng: () => number = Math.random): T[] {
  const a = items.slice();
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(rng() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

/** A random sample of up to `n` items (a prefix of a full shuffle). */
export function takeRandom<T>(items: readonly T[], n: number, rng: () => number = Math.random): T[] {
  return shuffle(items, rng).slice(0, Math.max(0, n));
}
