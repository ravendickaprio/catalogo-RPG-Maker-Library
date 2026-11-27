import { useEffect, useRef } from "react";
import { checkDataVersion } from "../api/client";

export const REFRESH_INTERVAL_MS = 30_000;

/**
 * ADR-010: asks the backend whether another process committed to the DB (PRAGMA data_version)
 * on window focus and every 30 s while the window is visible. Calls `onChanged` when it did.
 */
export function useExternalRefresh(onChanged: () => void, intervalMs = REFRESH_INTERVAL_MS): void {
  const cb = useRef(onChanged);
  cb.current = onChanged;
  useEffect(() => {
    let busy = false;
    let alive = true;
    const check = () => {
      if (busy || document.visibilityState === "hidden") return;
      busy = true;
      checkDataVersion().then(
        (r) => {
          busy = false;
          if (alive && r.changed) cb.current();
        },
        () => {
          busy = false; // transient failure: try again on the next trigger
        },
      );
    };
    const onVisible = () => {
      if (document.visibilityState === "visible") check();
    };
    window.addEventListener("focus", check);
    document.addEventListener("visibilitychange", onVisible);
    const timer = window.setInterval(check, intervalMs);
    return () => {
      alive = false;
      window.removeEventListener("focus", check);
      document.removeEventListener("visibilitychange", onVisible);
      window.clearInterval(timer);
    };
  }, [intervalMs]);
}
