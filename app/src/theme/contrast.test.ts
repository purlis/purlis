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
import { builtStyles } from "../ui/stylesheets";
import { property, TOKENS, type Token } from "./theme";

/**
 * The engine's pair list, read out of its source: each `p("front", "back", floor)` with the
 * layers its `.over("layer")` calls lay it over, the top one first (#2724), as
 * `front on back over layer over layer`.
 */
const CHECK = join(process.cwd(), "..", "crates", "purlis-core", "src", "theme", "check.rs");
const LIST =
  /pub const PAIRS: &\[Pair\] = &\[([\s\S]*?)\n\];/.exec(readFileSync(CHECK, "utf8"))?.[1] ?? "";
const LAID: readonly string[] = [
  ...LIST.matchAll(
    /\bp\(\s*"([^"]+)",\s*"([^"]+)",[^)]*\)((?:\s*\.(?:over|faded|selected)\([^)]*\))*)/g,
  ),
].map(([, front, back, calls]) =>
  [
    `${front} on ${back}`,
    ...[...(calls ?? "").matchAll(/\.over\(\s*"([^"]+)"\s*\)/g)].map(([, layer]) => layer),
  ].join(" over "),
);
/** `front on back` for each pair, whatever it is laid over. */
const HELD: ReadonlySet<string> = new Set(LAID.map((pair) => pair.split(" over ")[0] ?? pair));

describe("the engine's pair list", () => {
  it("is read out of the core", () => {
    expect(HELD.size).toBeGreaterThan(100);
    expect(HELD.has("text.primary on surface.base")).toBe(true);
    expect(HELD.has("text.primary on danger.wash")).toBe(true);
    for (const pair of LAID)
      for (const token of pair.split(/ on | over /)) expect(TOKENS, pair).toContain(token as Token);
  });

  it("reads each layer a pair is laid over, two at most", () => {
    expect(LAID).toContain("text.primary on danger.wash over surface.overlay");
    expect(LAID).toContain("text.primary on surface.overlay");
    // The command a profile will run as you, selected on the danger wash in its dialog.
    expect(LAID).toContain(
      "selection.text on selection.background over danger.wash over surface.overlay",
    );
    for (const pair of LAID) expect(pair.split(" over ").length, pair).toBeLessThanOrEqual(3);
    // Every pair is read, and every layer of each, none skipped by a form this does not know.
    expect(LAID).toHaveLength([...LIST.matchAll(/\bp\(/g)].length);
    const layers = LAID.reduce((sum, pair) => sum + pair.split(" over ").length - 1, 0);
    expect(layers).toBe([...LIST.matchAll(/\.over\(/g)].length);
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
 * The colours the stylesheet check below cannot read, each as `rule | declaration`. Every other
 * colour and background a rule sets is one token, so each is read below (the explorer's change
 * marks name their state's colour in each rule, #2318). Reading is not pairing: a rule that sets
 * a colour and no background draws no pair of its own, so the change marks' colours are held on
 * their backgrounds by hand, in the engine's `PAIRS`, and the test below fails when one is not.
 *
 * - The Chats view's sticky head (#3331) is filled with its panel's fill, `--panel-fill`: the
 *   family default's `surface.base`, or the token a theme's `panel.root.fill` names. Its words
 *   (primary, secondary, muted) are held on `surface.base` and `surface.raised`, the two fills
 *   the built-in themes give a panel, in `PAIRS`.
 * - A view's top and its groups' names (`ui/panel/panel.css`, #3340) are drawn on the same
 *   fill. What stands on them is a row's words and a group's name, held on both grounds in the
 *   engine's `PAIRS` (`text.secondary`, `text.quiet` on `surface.base` and `surface.raised`).
 */
const READ_BY_HAND: readonly string[] = [
  ".chats-head | background: var(--panel-fill, var(--surface-base))",
  '[data-scope="panel-section"][data-part="top"] | background-color: var(--panel-fill)',
  '[data-scope="panel-section"][data-frame] [data-scope="list"][data-part="header"] | background-color: var(--panel-fill)',
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
    ...[
      ...builtStyles(join(process.cwd(), "src"))
        .replace(/\/\*[\s\S]*?\*\//g, "")
        .matchAll(/@import "\.\/([^"]+\.css)"/g),
    ]
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
    // A row under the pointer (`primitives/collections.css`) sets only the row's fill; the words
    // on it are the colour `.ui-row` gives them.
    expect(derivedBy.get("text.primary on surface.hover")).toContain(".ui-row:hover over .ui-row");
  });

  it("holds each change mark's colour on every row the explorer draws it on", () => {
    // A change mark (FM-4), its letter and the file's name, sets its colour and no background:
    // it is drawn on the row it is on, which is the window, the row under the pointer or the
    // selected row (#2392). A name rule is read as well as the letter's, so one split out with a
    // colour of its own is held too.
    const markRule =
      /^\.explorer (?:\.change-mark|\[data-mark(?:="[^"]*")?\] > \.file-node > \.spot-name)/;
    const marks = [...colours]
      .filter(([selector]) => markRule.test(selector))
      .flatMap(([, sets]) => [...sets.fronts]);
    expect(marks).toEqual(expect.arrayContaining(["state.success", "danger.base"]));
    const loose = [...new Set(marks)].flatMap((mark) =>
      ["surface.base", "surface.hover", "list.selected"]
        .map((ground) => `${mark} on ${ground}`)
        .filter((pair) => !held.has(pair)),
    );
    expect(loose).toEqual([]);
  });

  it("is in the engine's list, so it is held to a floor in every theme and tint", () => {
    const loose = [...drawn, ...derived]
      .filter(([pair]) => !held.has(pair))
      .map(([pair, rule]) => `${pair} (${rule})`);
    expect(loose).toEqual([]);
  });
});
