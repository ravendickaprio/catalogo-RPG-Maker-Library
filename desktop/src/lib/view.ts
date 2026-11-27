import { PREF_SIDEBAR, PREF_VIEW, readPref } from "./storage";

export type ViewMode = "list" | "gallery";

export function loadView(): ViewMode {
  return readPref(PREF_VIEW) === "gallery" ? "gallery" : "list";
}

/** Stored "open"/"collapsed"; with no preference the sidebar starts closed on narrow windows. */
export function loadSidebarOpen(): boolean {
  const v = readPref(PREF_SIDEBAR);
  if (v === "open") return true;
  if (v === "collapsed") return false;
  return typeof window === "undefined" || window.innerWidth >= 900;
}
