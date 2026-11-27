import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { useExternalRefresh } from "../hooks/useExternalRefresh";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mockInvoke = vi.mocked(invoke);

describe("useExternalRefresh", () => {
  beforeEach(() => mockInvoke.mockReset());

  it("calls onChanged only when check_data_version reports a change", async () => {
    mockInvoke.mockResolvedValueOnce({ version: 2, changed: false }).mockResolvedValueOnce({ version: 3, changed: true });
    const onChanged = vi.fn();
    renderHook(() => useExternalRefresh(onChanged));
    window.dispatchEvent(new Event("focus"));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledTimes(1));
    await Promise.resolve();
    expect(onChanged).not.toHaveBeenCalled();
    window.dispatchEvent(new Event("focus"));
    await waitFor(() => expect(onChanged).toHaveBeenCalledTimes(1));
    expect(mockInvoke).toHaveBeenCalledWith("check_data_version", undefined);
  });

  it("polls on the interval", async () => {
    mockInvoke.mockResolvedValue({ version: 1, changed: true });
    const onChanged = vi.fn();
    renderHook(() => useExternalRefresh(onChanged, 20));
    await waitFor(() => expect(onChanged).toHaveBeenCalled());
  });
});
