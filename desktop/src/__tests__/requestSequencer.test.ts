import { describe, it, expect } from "vitest";
import { RequestSequencer } from "../lib/requestSequencer";

describe("RequestSequencer", () => {
  it("only the latest token is valid", () => {
    const s = new RequestSequencer();
    const a = s.next();
    const b = s.next();
    expect(s.isLatest(a)).toBe(false);
    expect(s.isLatest(b)).toBe(true);
  });

  it("discards a slow stale response arriving after a newer one", async () => {
    const s = new RequestSequencer();
    const applied: string[] = [];
    const run = (label: string, delay: number) => {
      const t = s.next();
      return new Promise<void>((r) =>
        setTimeout(() => {
          if (s.isLatest(t)) applied.push(label);
          r();
        }, delay),
      );
    };
    await Promise.all([run("old", 30), run("new", 5)]);
    expect(applied).toEqual(["new"]);
  });

  it("invalidate drops outstanding tokens", () => {
    const s = new RequestSequencer();
    const t = s.next();
    s.invalidate();
    expect(s.isLatest(t)).toBe(false);
  });
});
