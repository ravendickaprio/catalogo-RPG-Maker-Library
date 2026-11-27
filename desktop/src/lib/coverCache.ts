import { getCover } from "../api/client";
import type { Cover } from "../api/types";

export interface CoverCache {
  /** Cached data URL (or null = known to have no cover); undefined = not cached. */
  peek(id: number): string | null | undefined;
  load(id: number): Promise<string | null>;
  /** Forgets a single id (e.g. after it was edited or deleted). */
  invalidate(id: number): void;
  clear(): void;
  size(): number;
}

export function toDataUrl(c: Cover | null): string | null {
  return c ? `data:${c.mime};base64,${c.dataBase64}` : null;
}

/** LRU of data URLs keyed by game id with in-flight de-duplication. Failures are not cached. */
export function createCoverCache(fetcher: (id: number) => Promise<Cover | null>, max = 300): CoverCache {
  const lru = new Map<number, string | null>();
  const inflight = new Map<number, Promise<string | null>>();
  let epoch = 0;

  const touch = (id: number, v: string | null) => {
    lru.delete(id);
    lru.set(id, v);
    while (lru.size > max) {
      const oldest = lru.keys().next().value as number;
      lru.delete(oldest);
    }
  };

  return {
    peek(id) {
      if (!lru.has(id)) return undefined;
      const v = lru.get(id) as string | null;
      touch(id, v);
      return v;
    },
    load(id) {
      if (lru.has(id)) return Promise.resolve(this.peek(id) as string | null);
      const pending = inflight.get(id);
      if (pending) return pending;
      const myEpoch = epoch;
      const p = fetcher(id).then(
        (c) => {
          const url = toDataUrl(c);
          if (myEpoch === epoch) {
            inflight.delete(id);
            touch(id, url);
          }
          return url;
        },
        (e) => {
          if (myEpoch === epoch) inflight.delete(id);
          throw e;
        },
      );
      inflight.set(id, p);
      return p;
    },
    invalidate(id) {
      lru.delete(id);
      inflight.delete(id);
    },
    clear() {
      epoch++;
      lru.clear();
      inflight.clear();
    },
    size: () => lru.size,
  };
}

export const coverCache = createCoverCache(getCover, 300);
export const invalidateCover = (id: number) => coverCache.invalidate(id);
export const clearCoverCache = () => coverCache.clear();
