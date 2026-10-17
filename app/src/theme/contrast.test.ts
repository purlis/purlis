// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * Whether the words can be read — **the stylesheets' half**.
 *
 * The contrast check is the core's theme engine's (`purlis_core::theme::check`): it holds every
 * pair of tokens that ends up as something drawn on something to a WCAG floor, the ANSI colours
 * to the terminal's, and the selected row apart, for any theme as it is and tinted by every
 * workspace colour — and the built-ins pass it (`purlis_core::theme`'s tests). Its pair list
 * (`PAIRS` in `crates/purlis-core/src/theme/check.rs`) is the one source of which pairs exist.
 *
 * What only the window can know is which pairs its stylesheets draw, so that half is here: every
 * rule that sets a token as its text colour and another as its background draws a pair, and
 * this fails until the pair is in the engine's list.
 */

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { property, TOKENS, type Token } from "./theme";

/** The engine's pair list, read out of its source: `front on back` for each `p("front", "back"`. */
const CHECK = join(process.cwd(), "..", "crates", "purlis-core", "src", "theme", "check.rs");
const HELD: ReadonlySet<string> = (() => {
  const source = readFileSync(CHECK, "utf8");
  const list = /pub const PAIRS: &\[Pair\] = &\[([\s\S]*?)\n\];/.exec(source)?.[1] ?? "";
  return new Set(
    [...list.matchAll(/\bp\(\s*"([^"]+)",\s*"([^"]+)"/g)].map(
      ([, front, back]) => `${front} on ${back}`,
    ),
  );
})();

describe("the engine's pair list", () => {
  it("is read out of the core", () => {
    expect(HELD.size).toBeGreaterThan(100);
    expect(HELD.has("text.primary on surface.base")).toBe(true);
    expect(HELD.has("text.primary on danger.wash")).toBe(true);
    for (const pair of HELD)
      for (const token of pair.split(" on ")) expect(TOKENS, pair).toContain(token as Token);
  });
});

/**
 * **Every pair the stylesheets draw is held by the engine** (DS-6, #629). A rule that sets a token as
 * its text colour and another as its background draws the one on the other, so the pair is in
 * the engine's `PAIRS`, at the floor its use asks for, or this fails naming the rule.
 *
 * **A state rule is read over its base rule.** A `:hover`, `:focus-visible` or
 * `[aria-selected]` rule often sets only a background, and the words on it keep the colour the
 * same selector without the state gives them (`.row:hover` over `.row`), or the other way
 * round. So a state rule's backgrounds are paired with its own colours, or its base rule's when
 * it sets none, and its colours with its base rule's backgrounds when it sets none. A colour a
 * rule takes from a parent's background is still held in the engine's list by hand, not here.
 */
/** What a colour or a background may be without naming a token: nothing is drawn by it. */
const KEYWORDS = new Set(["inherit", "transparent", "none", "currentcolor", "unset", "initial"]);

/**
 * The colours the stylesheet check below cannot read, each as `rule | declaration`. None of
 * these rules sets a background, so none draws a pair of its own; the colour each one mixes
 * from is held by the engine's `PAIRS` (`text.muted`, the change marks' state colours).
 */
const READ_BY_HAND: readonly string[] = [
  ".explorer [data-mark] > .file-node > .spot-name | color: var(--change-colour)",
  ".explorer .change-mark | color: var(--change-colour)",
];

/** The parts of a selector that say a state, not what is drawn: the pointer on it, the
 *  keyboard on it, pressed, or chosen. */
const STATE =
  /:(?:hover|focus-visible|focus-within|focus|active)(?![\w-])|\[aria-(?:selected|current|pressed|checked|expanded)(?:="[^"]*")?\]/g;

/**
 * The selector `selector` is a state of, or `undefined` when it says no state: each state part
 * taken out, and each `:not(…)`, which only narrows which of the base rule's elements are meant.
 */
function baseOf(selector: string): string | undefined {
  if (selector.match(STATE) === null) return undefined;
  let base = selector.replace(STATE, "");
  for (let was = ""; was !== base;) {
    was = base;
    base = base.replace(/:not\([^()]*(?:\([^()]*\)[^()]*)*\)/g, "");
  }
  base = base.replace(/\s+/g, " ").trim();
  return base === "" || base === selector ? undefined : base;
}

describe("a state rule's base", () => {
  it("is the selector without its state and its narrowing", () => {
    expect(baseOf(".row:hover")).toBe(".row");
    expect(baseOf('.tabs [role="tab"]:hover:not([aria-selected="true"])')).toBe(
      '.tabs [role="tab"]',
    );
    expect(baseOf('.tabs [role="tab"][aria-selected="true"]')).toBe('.tabs [role="tab"]');
    expect(baseOf("button.bare:focus-visible:not(:disabled)")).toBe("button.bare");
    expect(baseOf(".row:not(.quiet)")).toBeUndefined();
    expect(baseOf(".row")).toBeUndefined();
  });
});

describe("every pair a stylesheet draws", () => {
  const byProperty = new Map(TOKENS.map((token) => [property(token), token]));
  const held = HELD;
  const read = (file: string) =>
    readFileSync(join(process.cwd(), "src", file), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
  /** `styles.css` and every sheet of the window's own it imports, outside `theme/` (ADR 0094):
   *  App.css, the control baseline and each primitive's sheet. A new sheet is read here by
   *  being imported, so none can draw a pair this test never sees. */
  const sheetNames = [
    "styles.css",
    ...[...read("styles.css").matchAll(/@import "\.\/([^"]+\.css)"/g)]
      .map((hit) => hit[1])
      .filter((file) => !file.startsWith("theme/")),
  ];
  const sheets = sheetNames.map(read);

  it("reads every sheet of the window's own", () => {
    expect(sheetNames).toEqual(expect.arrayContaining(["styles.css", "App.css", "base.css"]));
  });
  const drawn = new Map<string, string>();
  /** Each selector's colours and backgrounds, over every rule that names it. */
  const colours = new Map<string, { fronts: Set<Token>; backs: Set<Token> }>();
  /** Each colour or background a rule sets that is not one token, as `rule | declaration`. */
  const unread: string[] = [];
  for (const sheet of sheets)
    for (const [, selector, body] of sheet.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      const rule = selector
        .trim()
        .split(/\s*,\s*/)[0]
        .replace(/\s+/g, " ");
      for (const [, name, value] of body.matchAll(
        /(?<![\w-])(color|background|background-color)\s*:\s*([^;]+)/g,
      )) {
        const said = value.trim().replace(/\s*!important$/, "");
        const one = /^var\((--[\w-]+)\)$/.exec(said);
        if (one !== null && byProperty.has(one[1])) continue;
        if (KEYWORDS.has(said.toLowerCase())) continue;
        unread.push(`${rule} | ${name}: ${said}`);
      }
      const read = (pattern: RegExp) =>
        [...body.matchAll(pattern)]
          .map((hit) => byProperty.get(hit[1]))
          .filter((token) => token !== undefined);
      const fronts = read(/(?<![\w-])color:\s*var\((--[\w-]+)\)/g);
      const backs = read(/(?<![\w-])background(?:-color)?:\s*var\((--[\w-]+)\)/g);
      for (const front of fronts)
        for (const back of backs)
          if (!drawn.has(`${front} on ${back}`))
            drawn.set(`${front} on ${back}`, selector.trim().split(/\s*,\s*/)[0]);
      for (const one of selector.trim().split(/\s*,\s*/)) {
        const key = one.replace(/\s+/g, " ");
        const sets = colours.get(key) ?? { fronts: new Set<Token>(), backs: new Set<Token>() };
        fronts.forEach((token) => sets.fronts.add(token));
        backs.forEach((token) => sets.backs.add(token));
        colours.set(key, sets);
      }
    }
  /** The pairs a state rule draws over its base rule, each with `state over base`. */
  const derived = new Map<string, string>();
  /** Every `state over base` that draws each pair, not only the first. */
  const derivedBy = new Map<string, string[]>();
  for (const [selector, own] of colours) {
    const base = baseOf(selector);
    const under = base === undefined ? undefined : colours.get(base);
    if (under === undefined) continue;
    const fronts = own.fronts.size > 0 ? own.fronts : under.fronts;
    const backs = own.backs.size > 0 ? own.backs : under.backs;
    for (const front of fronts)
      for (const back of backs) {
        const pair = `${front} on ${back}`;
        if (!derived.has(pair)) derived.set(pair, `${selector} over ${base}`);
        derivedBy.set(pair, [...(derivedBy.get(pair) ?? []), `${selector} over ${base}`]);
      }
  }

  it("reads every colour and background a rule sets, or names the ones read by hand", () => {
    // A value this check cannot resolve to one token (a `color-mix`, a property of the rule's
    // own, a literal) would let its rule's pair through unmeasured, so each is listed here,
    // and a new one fails until it is held by hand in the engine's PAIRS and added below.
    expect(unread.sort()).toEqual([...READ_BY_HAND].sort());
  });

  it("is found: the stylesheets draw text on a background", () => {
    expect(drawn.get("danger.text on danger.surface")).toBeDefined();
  });

  it("is found where a state rule sets a background on its own, under its base rule's colour", () => {
    // A menu row under the pointer (`primitives/controls.css`) sets only the row's fill; the words on it
    // are the colour `.ui-menu-item` gives them.
    expect(derivedBy.get("text.primary on tab.active")).toContain(
      ".ui-menu-item:hover:not([data-disabled]) over .ui-menu-item",
    );
  });

  it("is in the engine's list, so it is held to a floor in every theme and tint", () => {
    const loose = [...drawn, ...derived]
      .filter(([pair]) => !held.has(pair))
      .map(([pair, rule]) => `${pair} (${rule})`);
    expect(loose).toEqual([]);
  });
});
