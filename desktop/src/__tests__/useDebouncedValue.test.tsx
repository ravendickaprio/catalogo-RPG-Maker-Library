import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useDebouncedValue } from "../hooks/useDebouncedValue";

describe("useDebouncedValue", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("updates only after the delay and coalesces rapid changes", () => {
    const { result, rerender } = renderHook(({ v }) => useDebouncedValue(v, 150), { initialProps: { v: "a" } });
    expect(result.current).toBe("a");
    rerender({ v: "ab" });
    act(() => void vi.advanceTimersByTime(100));
    rerender({ v: "abc" });
    act(() => void vi.advanceTimersByTime(100));
    expect(result.current).toBe("a");
    act(() => void vi.advanceTimersByTime(60));
    expect(result.current).toBe("abc");
  });
});
