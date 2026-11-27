/** Single source of truth for keyboard shortcuts: drives the key handlers and the help dialog. */

export type ShortcutId =
  | "palette"
  | "focus-search"
  | "new-game"
  | "view-list"
  | "view-gallery"
  | "help"
  | "form-save"
  | "form-cancel"
  | "review-save-next"
  | "list-nav"
  | "gallery-nav";

export interface Combo {
  /** `KeyboardEvent.key`, lowercase for letters. */
  key: string;
  /** Requires Ctrl (or Cmd). Without it, Ctrl/Cmd/Alt must NOT be pressed. */
  mod?: boolean;
  /** true = require Shift, false = require no Shift, undefined = ignore Shift. */
  shift?: boolean;
  /** Ignored while typing in a text field. */
  notTyping?: boolean;
}

export interface Shortcut {
  id: ShortcutId;
  description: string;
  scope: string;
  combos: Combo[];
  /** Display text when it can't be derived from `combos` (navigation keys handled by the lists). */
  display?: string;
}

export const SHORTCUTS: Shortcut[] = [
  { id: "palette", scope: "Global", description: "Abrir la paleta de comandos", combos: [{ key: "k", mod: true }, { key: "p", mod: true, shift: true }] },
  { id: "focus-search", scope: "Global", description: "Ir al buscador", combos: [{ key: "/", notTyping: true }] },
  { id: "new-game", scope: "Global", description: "Nuevo juego", combos: [{ key: "n", mod: true, shift: false }] },
  { id: "view-list", scope: "Global", description: "Vista lista", combos: [{ key: "1", mod: true }] },
  { id: "view-gallery", scope: "Global", description: "Vista galería", combos: [{ key: "2", mod: true }] },
  { id: "help", scope: "Global", description: "Mostrar los atajos de teclado", combos: [{ key: "?", notTyping: true }, { key: "F1" }] },
  { id: "form-save", scope: "Formulario", description: "Guardar el formulario", combos: [{ key: "s", mod: true }] },
  { id: "form-cancel", scope: "Formulario", description: "Cancelar la edición", combos: [{ key: "Escape" }] },
  { id: "review-save-next", scope: "Revisión por tandas", description: "Guardar y pasar al siguiente juego", combos: [{ key: "Enter", mod: true }] },
  { id: "list-nav", scope: "Lista", description: "Moverse por la lista; Enter abre el detalle", combos: [], display: "↑ ↓ Inicio Fin Enter" },
  { id: "gallery-nav", scope: "Galería", description: "Moverse por la galería; Enter abre el detalle", combos: [], display: "← → ↑ ↓ Enter" },
];

export function isTypingTarget(t: EventTarget | null): boolean {
  const el = t as HTMLElement | null;
  if (!el || !el.tagName) return false;
  return el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable === true;
}

type KeyLike = Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey"> & { target?: EventTarget | null };

export function comboMatches(c: Combo, e: KeyLike): boolean {
  const mod = e.ctrlKey || e.metaKey;
  if (e.altKey) return false;
  if (!!c.mod !== mod) return false;
  if (c.shift !== undefined && c.shift !== e.shiftKey) return false;
  if (c.notTyping && isTypingTarget(e.target ?? null)) return false;
  return (e.key.length === 1 ? e.key.toLowerCase() : e.key) === c.key;
}

/** The shortcut triggered by `e`, or null. */
export function matchShortcut(e: KeyLike): Shortcut | null {
  return SHORTCUTS.find((s) => s.combos.some((c) => comboMatches(c, e))) ?? null;
}

export function matches(id: ShortcutId, e: KeyLike): boolean {
  const s = SHORTCUTS.find((x) => x.id === id);
  return !!s && s.combos.some((c) => comboMatches(c, e));
}

const KEY_LABEL: Record<string, string> = { Escape: "Esc", Enter: "Enter" };
export function formatCombo(c: Combo): string {
  const parts: string[] = [];
  if (c.mod) parts.push("Ctrl");
  if (c.shift) parts.push("Shift");
  parts.push(KEY_LABEL[c.key] ?? (c.key.length === 1 ? c.key.toUpperCase() : c.key));
  return parts.join("+");
}
export function formatShortcut(s: Shortcut): string {
  return s.display ?? s.combos.map(formatCombo).join(" / ");
}

/** True while an aria-modal dialog is open: global shortcuts must not fire behind it. */
export function isModalOpen(): boolean {
  return typeof document !== "undefined" && document.querySelector('[aria-modal="true"]') !== null;
}
