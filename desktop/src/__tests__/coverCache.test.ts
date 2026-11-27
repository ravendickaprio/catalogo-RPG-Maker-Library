import { describe, it, expect, vi } from "vitest";
import { createCoverCache } from "../lib/coverCache";

const cover = (id: number) => ({ mime: "image/webp", dataBase64: `b${id}` });

describe("coverCache", () => {
  it("evicts the least recently used entry beyond the max", async () => {
    const fetcher = vi.fn(async (id: number) => cover(id));
    const c = createCoverCache(fetcher, 3);
    for (const id of [1, 2, 3]) await c.load(id);
    c.peek(1); // 1 becomes most recent; 2 is now oldest
    await c.load(4);
    expect(c.size()).toBe(3);
    expect(c.peek(2)).toBeUndefined();
    expect(c.peek(1)).toBe("data:image/webp;base64,b1");
    expect(c.peek(4)).toBeDefined();
  });

  it("de-duplicates in-flight requests and serves hits without refetching", async () => {
    let resolve!: (v: ReturnType<typeof cover>) => void;
    const fetcher = vi.fn(() => new Promise<ReturnType<typeof cover>>((r) => (resolve = r)));
    const c = createCoverCache(fetcher, 10);
    const p1 = c.load(7);
    const p2 = c.load(7);
    expect(fetcher).toHaveBeenCalledTimes(1);
    resolve(cover(7));
    expect(await p1).toBe(await p2);
    await c.load(7);
    expect(fetcher).toHaveBeenCalledTimes(1);
  });

  it("does not cache failures and clear() empties the cache", async () => {
    const fetcher = vi.fn().mockRejectedValueOnce(new Error("x")).mockResolvedValue(cover(1));
    const c = createCoverCache(fetcher, 10);
    await expect(c.load(1)).rejects.toThrow();
    expect(c.peek(1)).toBeUndefined();
    await c.load(1);
    expect(c.size()).toBe(1);
    c.clear();
    expect(c.size()).toBe(0);
  });
});
