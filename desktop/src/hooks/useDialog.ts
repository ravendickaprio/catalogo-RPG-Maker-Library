import { useEffect, useRef } from "react";

const FOCUSABLE = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * Focus management for modal dialogs: focuses `initial` (or the container) on mount, traps Tab,
 * calls onEscape on Esc and returns focus to the element that opened the dialog.
 */
export function useDialog(onEscape: () => void, initialSelector?: string) {
  const ref = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLElement | null>(typeof document !== "undefined" ? (document.activeElement as HTMLElement | null) : null);
  useEffect(() => {
    const el = ref.current;
    const target = initialSelector ? el?.querySelector<HTMLElement>(initialSelector) : null;
    (target ?? el)?.focus();
    const t = trigger.current;
    return () => {
      if (t && document.contains(t)) t.focus?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onEscape();
    } else if (e.key === "Tab" && ref.current) {
      const els = Array.from(ref.current.querySelectorAll<HTMLElement>(FOCUSABLE));
      if (!els.length) return;
      const first = els[0];
      const last = els[els.length - 1];
      const active = document.activeElement;
      if (e.shiftKey && (active === first || active === ref.current)) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && active === last) {
        e.preventDefault();
        first.focus();
      }
    }
  };
  return { ref, onKeyDown };
}
