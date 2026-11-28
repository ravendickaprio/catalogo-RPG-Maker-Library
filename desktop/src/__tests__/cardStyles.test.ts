import { describe, it, expect } from "vitest";
import css from "../styles.css?raw";
import { CARD_MIN_W } from "../lib/gridLayout";

// jsdom does not apply stylesheets, so the rules are checked as text; the real look is verified in the window.
const rule = (sel: string) => {
  const m = css.match(new RegExp(`(^|\\n)${sel.replace(/[.*+?^${}()|[\]\\:]/g, "\\$&")}\\s*\\{([^}]*)\\}`));
  if (!m) throw new Error(`missing rule ${sel}`);
  return m[2];
};

describe("game card styles", () => {
  it("halo is a radial gradient anchored at the top-left corner that fades out before the card's other corners", () => {
    const before = rule(".card::before");
    expect(before).toMatch(/radial-gradient\(circle at 0 0,/);
    const end = Number(before.match(/transparent (\d+)px\)/)?.[1]);
    expect(end).toBeGreaterThan(0);
    expect(end).toBeLessThan(CARD_MIN_W); // never reaches the top-right corner of the narrowest card
    expect(before).toMatch(/pointer-events: none/);
    expect(before).toMatch(/border-radius: inherit/);
  });

  it("card clips to rounded corners and hover lifts + intensifies the halo with a short transition", () => {
    const card = rule(".card");
    expect(card).toMatch(/overflow: hidden/);
    expect(card).toMatch(/border-radius: var\(--radius\)/);
    expect(card).toMatch(/transform \.2s ease/); // 200 ms, inside the 150–250 ms target
    expect(rule(".card:hover")).toMatch(/transform: translateY\(-[1-4]px\)/);
    expect(rule(".card:hover::before")).toMatch(/opacity: 1/);
  });

  it("reduced motion disables transitions and the hover lift", () => {
    expect(css).toMatch(/prefers-reduced-motion: reduce\) \{[^\n]*\.card:hover \{ transform: none; \}/);
  });

  it("panels, menus and buttons carry the blue top-left halo, and button states keep it", () => {
    expect(css).toMatch(/--halo-panel: radial-gradient\(circle at 0 0, var\(--halo\) 0, transparent \d+px\)/);
    expect(rule(".sidebar, .list-pane, .detail-pane, .dialog, .tools-list")).toMatch(/background-image: var\(--halo-panel\)/);
    expect(css).toMatch(/\nbutton \{ background-image: var\(--halo-btn\); \}/); // later than `button { cursor }`, wins
    expect(rule("button:hover:not(:disabled)")).toMatch(/background-image: var\(--halo-btn-hover\)/);
    // The `background` shorthand would reset background-image and drop the halo on these states.
    for (const sel of ["button.primary, .btn-suggest", 'button[aria-pressed="true"]', "button.danger:hover:not(:disabled)"])
      expect(rule(sel)).not.toMatch(/(^|[\s;])background:/);
  });
});
