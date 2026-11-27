import { describe, it, expect } from "vitest";
import { formatDateTime, formatMs } from "../lib/format";

describe("format", () => {
  it("formats dates in es-ES (day first, 24h)", () => {
    const s = formatDateTime("2026-11-27T14:17:05");
    expect(s).toMatch(/^27\/11\/(26|2026)/);
    expect(s).not.toMatch(/PM|AM/);
  });
  it("formats ms with one decimal and a comma", () => expect(formatMs(6.8)).toMatch(/^6,8\sms$/));
});
