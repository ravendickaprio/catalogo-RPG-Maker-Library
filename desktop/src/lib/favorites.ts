/** Reserved tag used as "favorite" (ADR-017). Python apps see it as a normal tag. */
export const FAVORITE_TAG = "Favorito";

const same = (a: string) => a.trim().toLowerCase() === FAVORITE_TAG.toLowerCase();

export function isFavorite(tags: readonly string[]): boolean {
  return tags.some(same);
}

/** Adds the tag when missing, removes every case-insensitive match otherwise; other tags are untouched. */
export function toggleFavoriteTags(tags: readonly string[]): { tags: string[]; favorite: boolean } {
  if (isFavorite(tags)) return { tags: tags.filter((t) => !same(t)), favorite: false };
  return { tags: [...tags, FAVORITE_TAG], favorite: true };
}

/** The spelling of the favorite tag as stored in the library (case may differ), or null if nobody has it. */
export function findFavoriteTag(tags: readonly { value: string }[] | undefined): string | null {
  return tags?.find((t) => same(t.value))?.value ?? null;
}
