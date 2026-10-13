// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { declarations, isView, ratchet, SRC, sourceFiles, withoutComments } from "./guards.testkit";
import { GEOMETRY } from "./theme/geometry";
import { property, TOKENS } from "./theme/theme";
import { TYPE_ALLOWANCE } from "./type.guard.allowance";

/**
 * **No size, weight, line height, family or colour of text is written outside the tokens**
 * (#1920; `docs/design-system.md` "Text roles").
 *
 * The window's words were ranked surface by surface: 152 font sizes in 25 values, weights as
 * bare numbers, two monospace stacks, and the section label written five ways. A word now
 * wears a **text role** (`primitives/text.css`: `ui-text-title`, `-heading`, `-body`,
 * `-secondary`, `-meta`, `-label`, `-code`, and `ui-text-strong`), and this guard refuses the
 * other way of ranking it. In a stylesheet, or in a component's inline `style`:
 *
 * | Property      | May be                                                        |
 * | ------------- | ------------------------------------------------------------- |
 * | `font-size`   | `var(--font-size-*)`, one of the core's six                    |
 * | `font-weight` | `var(--font-weight-regular)` or `var(--font-weight-strong)`    |
 * | `line-height` | `var(--line-height-tight)` or `var(--line-height-body)`        |
 * | `font-family` | `var(--font-mono)`                                             |
 * | `color`       | `var(--<a theme's colour token>)`, or `currentColor`           |
 * | `font`        | `inherit` only: the shorthand carries a size and a family      |
 *
 * and `inherit` for each. Anything else is a literal: `0.8rem`, `600`, `bold`, `1.4`, a stack
 * of family names, a mixed or transparent colour.
 *
 * **A token is not yet a role.** `font-size: var(--font-size-sm)` passes here, and is still a
 * surface ranking its own words; the migrations of #1920 replace those with a role, surface by
 * surface. What this guard stops is the scale growing a 26th size while they do.
 *
 * **A font size is held twice**, here and by the geometry guard (`geometry.guard.test.ts`),
 * which lets the text's own unit through (`1em`) and reads no weight, family or colour. A
 * migration that gives a word its role deletes its line from both lists.
 *
 * **Today's offenders are allowed, one per line, and the list can only shrink**
 * (`type.guard.allowance.ts`), the same ratchet as the geometry guard: an entry names a
 * declaration by its file, its rule and what it sets, never by line; the list must match the
 * source exactly; and no file's list may grow past its frozen `CEILING`.
 */

/** The one stylesheet that gives the core tokens their values. */
const THE_CORE = "theme/geometry.css";

/** The tokens each property may take its value from. */
const COLOURS: ReadonlySet<string> = new Set(TOKENS.map(property));
const MAY_BE: Readonly<Record<string, ReadonlySet<string>>> = {
  "font-size": new Set(GEOMETRY.type),
  "font-weight": new Set(GEOMETRY.weight),
  "line-height": new Set(GEOMETRY.leading),
  "font-family": new Set(GEOMETRY.family),
  color: COLOURS,
  font: new Set(),
};

/** Whether a declaration's value is one of its property's tokens, or inherited. */
export function isOnTheScale(name: string, value: string): boolean {
  const said = value.replace(/\s*!important$/i, "").trim();
  if (/^inherit$/i.test(said)) return true;
  if (name === "color" && /^currentcolor$/i.test(said)) return true;
  const token = /^var\((--[\w-]+)\)$/.exec(said)?.[1];
  return token !== undefined && MAY_BE[name].has(token);
}

/** Each type declaration in a stylesheet that is off the scale, as its allowance entry:
 *  `<rule> | <property>: <value>`, the rule being its selectors and at-rules, outermost first. */
export function cssLiterals(css: string): string[] {
  return declarations(css)
    .filter(({ property: name }) => name in MAY_BE)
    .filter(({ property: name, value }) => !isOnTheScale(name, value))
    .map(({ context, property: name, value }) => `${context.join(" › ")} | ${name}: ${value}`);
}

/** A React style key that ranks a word, and the property it sets. */
const STYLE_KEY: Readonly<Record<string, string>> = {
  fontSize: "font-size",
  fontWeight: "font-weight",
  lineHeight: "line-height",
  fontFamily: "font-family",
  color: "color",
  font: "font",
};

/** Each type literal in a component's inline `style`, as `style <key>: <value>`. A value that
 *  is computed (a variable, a call) is not read: only what is written out. */
export function componentLiterals(source: string): string[] {
  const found: string[] = [];
  const text = withoutComments(source, "ts");
  for (const style of text.matchAll(/\bstyle=\{\{([\s\S]*?)\}\}/g)) {
    for (const [, key, value] of style[1].matchAll(
      /(?<![\w$.])(\w+)\s*:\s*(-?\d*\.?\d+(?![\w.])|"[^"]*"|'[^']*'|`[^`$]*`)/g,
    )) {
      const name = STYLE_KEY[key];
      if (name === undefined) continue;
      const written = /^-?[\d.]/.test(value) ? value : value.slice(1, -1);
      if (!isOnTheScale(name, written)) found.push(`style ${key}: ${value}`);
    }
  }
  return found;
}

/** Every type literal in the window today, by file: its stylesheets (the core aside) and its
 *  components. */
function literalsByFile(): Map<string, string[]> {
  const read = (file: string) => readFileSync(join(SRC, file), "utf8");
  const byFile = new Map<string, string[]>();
  for (const file of sourceFiles((name) => name.endsWith(".css"))) {
    if (file !== THE_CORE) byFile.set(file, cssLiterals(read(file)));
  }
  for (const file of sourceFiles(isView)) byFile.set(file, componentLiterals(read(file)));
  return new Map([...byFile].filter(([, found]) => found.length > 0));
}

/** The first length a value writes, in px at the window's default text size (a bare number is
 *  React's px), or `undefined` when it is the text's own (`em`, `%`) or no length at all. */
function px(value: string): number | undefined {
  if (/^\d*\.?\d+$/.test(value.trim())) return Number(value);
  const [, number, unit] = /(?<![\w.-])(\d*\.?\d+)(px|rem)\b/.exec(value) ?? [];
  if (number === undefined) return undefined;
  return unit === "rem" ? Number(number) * 14 : Number(number);
}

const ROLES_SAID = 'docs/ui-primitives.md "Text roles"';

/**
 * **Which role to wear instead of an offender**, said with the complaint: the role whose size is
 * nearest for a font size, the strong modifier or a role's own weight for a weight, and the
 * rule a colour, a line height or a family broke.
 */
export function roleFor(entry: string): string {
  const [, name = "", value = ""] =
    /(?:\| |^style )([\w-]+): ?(.*)$/.exec(entry.replace(/["'`]/g, "")) ?? [];
  const key = STYLE_KEY[name] ?? name;
  if (key === "font-size" || key === "font") {
    const size = px(value);
    if (size === undefined) {
      return "wear the role of what it says; a size in em is the scale times something";
    }
    if (size >= 15) return "wear ui-text-title (16px, strong), or keep it off the scale by ruling";
    if (size >= 13.5) return "wear ui-text-heading (14px, strong)";
    if (size >= 12.5) return "wear ui-text-body (13px), with ui-text-strong for the row's one line";
    if (size >= 11.5) return "wear ui-text-secondary (12px)";
    return "wear ui-text-meta (11px), or ui-text-label over a group: nothing is under 11px";
  }
  if (key === "font-weight") {
    return /^(?:[6-9]00|bold|bolder)$/.test(value)
      ? "wear ui-text-strong (600), or a role that is strong: title, heading, label"
      : "wear a role, which is regular (400) unless it is a title, a heading or a label";
  }
  if (key === "line-height") return "use var(--line-height-body) or var(--line-height-tight)";
  if (key === "font-family") return "wear ui-text-code, the one monospace family (--font-mono)";
  return "colour follows the role: body is primary, secondary is secondary, meta is muted";
}

/**
 * How long each file's list was when the guard landed: a list may only get shorter than this.
 * This is a frozen record, not a count to keep current. Lowering a ceiling is never needed;
 * raising one, or naming a new file, is adding a literal the guard exists to refuse.
 */
export const CEILING: Readonly<Record<string, number>> = {
  "App.css": 138,
  "primitives/collections.css": 2,
  "primitives/controls.css": 2,
};

const ADDED = "new type literal";

/** What differs between the type literals in the window and the list (`ratchet`), each new
 *  literal with the role to wear instead. */
export function offTheList(
  found: ReadonlyMap<string, readonly string[]>,
  allowed: Readonly<Record<string, readonly string[]>>,
  ceiling: Readonly<Record<string, number>> = CEILING,
): string[] {
  return ratchet(found, allowed, ceiling, { added: ADDED }).map((complaint) => {
    const at = complaint.indexOf(`: ${ADDED}: `);
    if (at < 0) return complaint;
    const entry = complaint.slice(at + ADDED.length + 4);
    return `${complaint} (${roleFor(entry)}; ${ROLES_SAID})`;
  });
}

describe("the type guard (#1920)", () => {
  it("finds every size, weight, line height, family and colour of text on a token, or allowed", () => {
    expect(offTheList(literalsByFile(), TYPE_ALLOWANCE)).toEqual([]);
  });

  it("fails a literal planted in a stylesheet, and says which role to wear", () => {
    const planted = new Map([
      [
        "App.css",
        cssLiterals(`
          .planted { font-size: 0.8rem; font-weight: 600; color: transparent; }
          .planted-more { line-height: 1.3; font-family: Menlo, monospace; font-size: 15px; }
        `),
      ],
    ]);
    expect(offTheList(planted, {}, { "App.css": 10 })).toEqual([
      `App.css: new type literal: .planted | font-size: 0.8rem (wear ui-text-meta (11px), or ui-text-label over a group: nothing is under 11px; ${ROLES_SAID})`,
      `App.css: new type literal: .planted | font-weight: 600 (wear ui-text-strong (600), or a role that is strong: title, heading, label; ${ROLES_SAID})`,
      `App.css: new type literal: .planted | color: transparent (colour follows the role: body is primary, secondary is secondary, meta is muted; ${ROLES_SAID})`,
      `App.css: new type literal: .planted-more | line-height: 1.3 (use var(--line-height-body) or var(--line-height-tight); ${ROLES_SAID})`,
      `App.css: new type literal: .planted-more | font-family: Menlo, monospace (wear ui-text-code, the one monospace family (--font-mono); ${ROLES_SAID})`,
      `App.css: new type literal: .planted-more | font-size: 15px (wear ui-text-title (16px, strong), or keep it off the scale by ruling; ${ROLES_SAID})`,
    ]);
  });

  it("fails a literal planted in the real stylesheet, on top of what is allowed", () => {
    const found = literalsByFile();
    const css = [...(found.get("App.css") ?? []), ".planted | font-size: 0.8rem"];
    const complaints = offTheList(new Map([...found, ["App.css", css]]), TYPE_ALLOWANCE);
    expect(complaints).toHaveLength(1);
    expect(complaints[0]).toContain("App.css: new type literal: .planted | font-size: 0.8rem");
  });

  it("fails when an allowance is removed and its literal is not fixed", () => {
    const [first, ...rest] = TYPE_ALLOWANCE["App.css"];
    const complaints = offTheList(literalsByFile(), { ...TYPE_ALLOWANCE, "App.css": rest });
    expect(complaints).toHaveLength(1);
    expect(complaints[0]).toContain(`App.css: new type literal: ${first}`);
  });

  it("fails an entry its declaration no longer bears out", () => {
    const allowed = { "App.css": [".a | font-size: 0.8rem", ".b | font-weight: 600"] };
    expect(
      offTheList(new Map([["App.css", [".a | font-size: 0.8rem"]]]), allowed, { "App.css": 2 }),
    ).toEqual(["App.css: stale entry, delete it from the allowance: .b | font-weight: 600"]);
  });

  it("fails a ceiling that was raised, and a file that was given one", () => {
    // The list and the source agree, and the list is one longer than the ceiling it had.
    const found = literalsByFile();
    const grown = [...TYPE_ALLOWANCE["App.css"], ".planted | font-size: 0.8rem"];
    expect(
      offTheList(new Map([...found, ["App.css", grown]]), { ...TYPE_ALLOWANCE, "App.css": grown }),
    ).toEqual([`App.css: the allowance grew to ${grown.length}, past ${CEILING["App.css"]}`]);
    const fresh = [".a | font-size: 0.8rem"];
    expect(offTheList(new Map([["Fresh.css", fresh]]), { "Fresh.css": fresh }, {})).toEqual([
      "Fresh.css: the allowance grew to 1, past 0",
    ]);
  });

  it("holds each ceiling to its list as it landed: a ceiling can only go down", () => {
    // What each file's list was when the guard landed (#1920). This is the frozen record the
    // ceilings are checked against, so raising a ceiling fails here even with the list grown
    // to match; a ceiling is lowered by deleting lines, and may be lowered here with them.
    const LANDED: Record<string, number> = {
      "App.css": 159,
      "primitives/collections.css": 2,
      "primitives/controls.css": 2,
    };
    expect(Object.keys(CEILING).sort()).toEqual(Object.keys(LANDED).sort());
    for (const [file, ceiling] of Object.entries(CEILING)) {
      expect(ceiling, `${file}: its ceiling was raised`).toBeLessThanOrEqual(LANDED[file]);
      expect(
        TYPE_ALLOWANCE[file]?.length ?? 0,
        `${file}: its list is longer than its ceiling`,
      ).toBeLessThanOrEqual(ceiling);
    }
    expect(Object.keys(TYPE_ALLOWANCE).filter((file) => !(file in CEILING))).toEqual([]);
  });

  it("counts a declaration written twice as two", () => {
    const twice = [".a | font-weight: 600", ".a | font-weight: 600"];
    expect(
      offTheList(new Map([["x.css", twice]]), { "x.css": [twice[0]] }, { "x.css": 2 }),
    ).toHaveLength(1);
  });

  it("reads each type property, and the literals in it, and nothing else", () => {
    const css = `
      .a { font-size: 0.8rem; font-weight: bold; line-height: 1; font-family: monospace; }
      .b { color: color-mix(in srgb, currentcolor 85%, transparent); font: 600 12px/1.2 system-ui; }
      .c { font-size: 1em; font-weight: normal; color: transparent; font-size: var(--mine); }
      .d { background-color: var(--surface-base); border-color: red; letter-spacing: 0.06em; }
      .e { font-variant-numeric: tabular-nums; font-synthesis: none; padding: 7px; }
    `;
    expect(cssLiterals(css).map((entry) => entry.split(" | ")[1])).toEqual([
      "font-size: 0.8rem",
      "font-weight: bold",
      "line-height: 1",
      "font-family: monospace",
      "color: color-mix(in srgb, currentcolor 85%, transparent)",
      "font: 600 12px/1.2 system-ui",
      "font-size: 1em",
      "font-weight: normal",
      "color: transparent",
      "font-size: var(--mine)",
    ]);
  });

  it("passes the tokens, each for its own property only, and what is inherited", () => {
    const css = `
      .a { font-size: var(--font-size-sm); font-weight: var(--font-weight-strong); }
      .b { line-height: var(--line-height-tight); font-family: var(--font-mono); }
      .c { color: var(--text-muted); font: inherit; font-size: inherit; color: inherit; }
      .d { color: currentColor; color: var(--state-failed) !important; line-height: inherit; }
    `;
    expect(cssLiterals(css)).toEqual([]);
    // A token of another kind is not this property's.
    const crossed = `
      .a { font-size: var(--space-4); font-weight: var(--font-size-sm); }
      .b { color: var(--font-size-sm); line-height: var(--font-weight-strong); }
      .c { font-family: var(--text-primary); font-size: currentColor; }
    `;
    expect(cssLiterals(crossed)).toHaveLength(6);
  });

  it("names a declaration by its rule, inside media queries and nesting, comments aside", () => {
    const css = `
      /* .commented { font-size: 9px; } */
      @media (max-width: 900px) { .a, .b { font-weight: 700 } }
      .c { &:hover { color: transparent; } }
    `;
    expect(cssLiterals(css)).toEqual([
      "@media (max-width: 900px) › .a, .b | font-weight: 700",
      ".c › &:hover | color: transparent",
    ]);
  });

  it("reads an inline style's type literals, and leaves its tokens and what is computed", () => {
    const tsx = `
      <p style={{ fontSize: 13, fontWeight: "600", color: 'transparent', lineHeight: 1.2 }} />
      <p style={{ fontFamily: \`Menlo\`, padding: 7, width: "12px" }} />
      <p style={{ fontSize: size, color: "var(--text-muted)", fontWeight: "inherit" }} />
    `;
    expect(componentLiterals(tsx)).toEqual([
      "style fontSize: 13",
      'style fontWeight: "600"',
      "style color: 'transparent'",
      "style lineHeight: 1.2",
      "style fontFamily: `Menlo`",
    ]);
    expect(roleFor('style fontWeight: "600"')).toContain("ui-text-strong");
    expect(roleFor("style fontSize: 13")).toContain("ui-text-body");
    expect(roleFor(".a | font-size: 0.9em")).toContain("a size in em");
    expect(roleFor(".a | font: 600 12px/1.2 system-ui")).toContain("ui-text-secondary");
  });

  it("names the nearest role for each size the window has written", () => {
    const role = (value: string) => /ui-text-\w+/.exec(roleFor(`.a | font-size: ${value}`))?.[0];
    expect(
      ["1.5rem", "1rem", "0.95rem", "0.9rem", "0.85rem", "0.8rem", "0.72rem"].map(role),
    ).toEqual([
      "ui-text-title",
      "ui-text-heading",
      "ui-text-body",
      "ui-text-body",
      "ui-text-secondary",
      "ui-text-meta",
      "ui-text-meta",
    ]);
  });

  it("lists by file and in order, so a removal is one line in one place", () => {
    for (const [file, entries] of Object.entries(TYPE_ALLOWANCE)) {
      expect(entries.length, `${file}: an empty list is deleted, not kept`).toBeGreaterThan(0);
    }
    expect(Object.keys(TYPE_ALLOWANCE)).toEqual(Object.keys(TYPE_ALLOWANCE).sort());
    const source = readFileSync(join(SRC, "type.guard.allowance.ts"), "utf8");
    const entryLines = source.split("\n").filter((line) => /^\s+"(?:[^"\\]|\\.)*",$/.test(line));
    const total = Object.values(TYPE_ALLOWANCE).flat().length;
    expect(entryLines.length, "one entry per line").toBe(total);
  });

  it("holds the role sheet and the other primitives to no literal at all", () => {
    const found = literalsByFile();
    expect(found.get("primitives/text.css")).toBeUndefined();
    expect(
      Object.keys(TYPE_ALLOWANCE).filter((file) => file.startsWith("primitives/text")),
    ).toEqual([]);
  });

  it("covers the window's stylesheets and components, so a reader that found nothing fails", () => {
    const sheets = sourceFiles((name) => name.endsWith(".css"));
    expect(sheets).toEqual(expect.arrayContaining(["App.css", "base.css", "primitives/text.css"]));
    expect(sourceFiles(isView)).toContain("Inbox.tsx");
    const typed = declarations(readFileSync(join(SRC, "App.css"), "utf8")).filter(
      ({ property: name }) => name in MAY_BE,
    );
    expect(typed.length).toBeGreaterThan(400);
    expect(COLOURS.has("--text-muted")).toBe(true);
  });
});
