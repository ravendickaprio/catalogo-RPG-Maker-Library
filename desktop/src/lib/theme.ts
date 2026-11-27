import { PREF_THEME, readPref, writePref } from "./storage";

export type Theme = "system" | "light" | "dark" | "night";

export const THEMES: { value: Theme; label: string }[] = [
  { value: "system", label: "Sistema" },
  { value: "light", label: "Claro" },
  { value: "dark", label: "Oscuro" },
  { value: "night", label: "Noche" },
];

export function isTheme(v: unknown): v is Theme {
  return THEMES.some((t) => t.value === v);
}

export function loadTheme(): Theme {
  const v = readPref(PREF_THEME);
  return isTheme(v) ? v : "system";
}

export function saveTheme(t: Theme): void {
  writePref(PREF_THEME, t);
}

/** "system" removes the attribute so the prefers-color-scheme media query decides. */
export function applyTheme(t: Theme): void {
  const root = document.documentElement;
  if (t === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", t);
}
