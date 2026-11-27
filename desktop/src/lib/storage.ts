/** localStorage wrappers that never throw (private mode, blocked storage, tests). */
export function readPref(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

export function writePref(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    /* ignore */
  }
}

export const PREF_THEME = "rpglib.theme";
export const PREF_VIEW = "rpglib.view";
export const PREF_SIDEBAR = "rpglib.sidebar";
