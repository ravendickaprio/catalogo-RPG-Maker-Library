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
  const escRef = useRef(onEscape);
  escRef.current = onEscape;
  // When focus escapes to <body> (e.g. the focused button became disabled), the element handler never
  // sees the key. Only the topmost modal (last [aria-modal] in the DOM) reacts, and only to events
  // whose target is outside every dialog, so it never double-fires with onKeyDown.
  useEffect(() => {
    const isTop = () => {
      const all = document.querySelectorAll('[aria-modal="true"]');
      return !!ref.current && all.length > 0 && all[all.length - 1] === ref.current;
    };
    const onDocKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || !isTop()) return;
      const t = e.target as Element | null;
      if (t && t.closest?.('[aria-modal="true"]')) return;
      if (e.key === "Escape") {
        e.preventDefault();
        escRef.current();
      } else if (e.key === "Tab") {
        const first = ref.current?.querySelector<HTMLElement>(FOCUSABLE);
        e.preventDefault();
        (first ?? ref.current)?.focus();
      }
    };
    const onFocusOut = () => {
      window.setTimeout(() => {
        const a = document.activeElement;
        if (ref.current && isTop() && (!a || a === document.body)) ref.current.focus();
      }, 0);
    };
    document.addEventListener("keydown", onDocKey);
    document.addEventListener("focusout", onFocusOut);
    return () => {
      document.removeEventListener("keydown", onDocKey);
      document.removeEventListener("focusout", onFocusOut);
    };
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
