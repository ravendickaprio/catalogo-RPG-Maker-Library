import { describe, it, expect, beforeEach } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { applyTheme, loadTheme, saveTheme } from "../lib/theme";
import { useTheme } from "../hooks/useTheme";

describe("theme", () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute("data-theme");
  });

  it("defaults to system and ignores invalid stored values", () => {
    expect(loadTheme()).toBe("system");
    localStorage.setItem("rpglib.theme", "neon");
    expect(loadTheme()).toBe("system");
  });

  it("saves and applies the theme; system removes the attribute", () => {
    saveTheme("night");
    expect(localStorage.getItem("rpglib.theme")).toBe("night");
    expect(loadTheme()).toBe("night");
    applyTheme("night");
    expect(document.documentElement.getAttribute("data-theme")).toBe("night");
    applyTheme("system");
    expect(document.documentElement.hasAttribute("data-theme")).toBe(false);
  });

  it("useTheme persists changes and restores them", () => {
    const { result } = renderHook(() => useTheme());
    act(() => result.current[1]("dark"));
    expect(localStorage.getItem("rpglib.theme")).toBe("dark");
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
    const again = renderHook(() => useTheme());
    expect(again.result.current[0]).toBe("dark");
  });
});
