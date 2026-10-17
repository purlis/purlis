// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { declarations, isView, ratchet, SRC, sourceFiles, withoutComments } from "./guards.testkit";
import { GEOMETRY } from "./theme/geometry";
import { property, TOKENS } from "./theme/theme";

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
 * | `font-size`   | `var(--font-size-*)`, one of the core's seven                  |
 * | `font-weight` | `var(--font-weight-*)`, one of the core's three                 |
 * | `line-height` | `var(--line-height-tight)`, `var(--line-height-body)`, or `1`  |
 * | `font-family` | `var(--font-mono)`                                             |
 * | `color`       | `var(--<a theme's colour token>)`, or `currentColor`           |
 * | `font`        | `inherit` only: the shorthand carries a size and a family      |
 *
 * and `inherit` for each. A line height of exactly `1` is no leading at all, the line as
 * tall as its own text, as `1em` is no size to the geometry guard. Anything else is a literal:
 * `0.8rem`, `600`, `bold`, `1.4`, a stack of family names, a mixed or transparent colour.
 *
 * **A token is not yet a role.** `font-size: var(--font-size-sm)` passes here, and is still a
 * surface ranking its own words; the migrations of #1920 replace those with a role, surface by
 * surface. What this guard stops is the scale growing a 26th size while they do.
 *
 * **A font size is held twice**, here and by the geometry guard (`geometry.guard.test.ts`),
 * which lets the text's own unit through (`1em`) and reads no weight, family or colour. A
 * migration that gives a word its role deletes its line from both lists.
 *
 * **There is no allowance** (#2319). The migrations took every literal the window had onto a
 * token or a role, and the list they shrank is gone: a literal in what this guard reads, as the
 * table says, fails here.
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
  if (name === "line-height" && said === "1") return true;
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

const read = (file: string) => readFileSync(join(SRC, file), "utf8");

/** Every type literal in the window today, by file: its stylesheets (the core aside) and its
 *  components. */
function literalsByFile(): Map<string, string[]> {
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
    return "wear ui-text-meta (11px), or ui-text-label over a group: nothing is under 11px but --font-size-2xs, a tight secondary label's";
  }
  if (key === "font-weight") {
    if (/^(?:[6-9]00|bold|bolder)$/.test(value)) {
      return "wear ui-text-strong (600), or a role that is strong: title, heading, label";
    }
    if (/^500$/.test(value)) return "use var(--font-weight-medium) (500), a form's label's weight";
    return "wear a role, which is regular (400) unless it is a title, a heading or a label";
  }
  if (key === "line-height") {
    return "use var(--line-height-body) or var(--line-height-tight), or 1 for no leading";
  }
  if (key === "font-family") return "wear ui-text-code, the one monospace family (--font-mono)";
  return "colour follows the role: body is primary, secondary is secondary, meta is muted";
}

const ADDED = "new type literal";

/** Each type literal found, as a complaint with the role to wear instead: with no allowance and
 *  no ceiling, every one is new (`ratchet`). */
export function complaints(found: ReadonlyMap<string, readonly string[]>): string[] {
  return ratchet(found, {}, {}, { added: ADDED }).map((complaint) => {
    const at = complaint.indexOf(`: ${ADDED}: `);
    if (at < 0) return complaint;
    const entry = complaint.slice(at + ADDED.length + 4);
    return `${complaint} (${roleFor(entry)}; ${ROLES_SAID})`;
  });
}

describe("the type guard (#1920)", () => {
  it("finds every size, weight, line height, family and colour of text on a token", () => {
    expect(complaints(literalsByFile())).toEqual([]);
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
    expect(complaints(planted)).toEqual([
      `App.css: new type literal: .planted | font-size: 0.8rem (wear ui-text-meta (11px), or ui-text-label over a group: nothing is under 11px but --font-size-2xs, a tight secondary label's; ${ROLES_SAID})`,
      `App.css: new type literal: .planted | font-weight: 600 (wear ui-text-strong (600), or a role that is strong: title, heading, label; ${ROLES_SAID})`,
      `App.css: new type literal: .planted | color: transparent (colour follows the role: body is primary, secondary is secondary, meta is muted; ${ROLES_SAID})`,
      `App.css: new type literal: .planted-more | line-height: 1.3 (use var(--line-height-body) or var(--line-height-tight), or 1 for no leading; ${ROLES_SAID})`,
      `App.css: new type literal: .planted-more | font-family: Menlo, monospace (wear ui-text-code, the one monospace family (--font-mono); ${ROLES_SAID})`,
      `App.css: new type literal: .planted-more | font-size: 15px (wear ui-text-title (16px, strong), or keep it off the scale by ruling; ${ROLES_SAID})`,
    ]);
  });

  it("fails a hand-written font size planted in the window's own stylesheet", () => {
    const found = literalsByFile();
    const planted = `${read("App.css")}\n.planted { font-size: 13px; }\n`;
    const result = complaints(new Map([...found, ["App.css", cssLiterals(planted)]]));
    expect(result).toHaveLength(1);
    expect(result[0]).toContain("App.css: new type literal: .planted | font-size: 13px");
  });

  it("counts a declaration written twice as two", () => {
    const twice = [".a | font-weight: 600", ".a | font-weight: 600"];
    expect(complaints(new Map([["x.css", twice]]))).toHaveLength(2);
  });

  it("reads each type property, and the literals in it, and nothing else", () => {
    const css = `
      .a { font-size: 0.8rem; font-weight: bold; line-height: 1.3; font-family: monospace; }
      .b { color: color-mix(in srgb, currentcolor 85%, transparent); font: 600 12px/1.2 system-ui; }
      .c { font-size: 1em; font-weight: normal; color: transparent; font-size: var(--mine); }
      .d { background-color: var(--surface-base); border-color: red; letter-spacing: 0.06em; }
      .e { font-variant-numeric: tabular-nums; font-synthesis: none; padding: 7px; }
    `;
    expect(cssLiterals(css).map((entry) => entry.split(" | ")[1])).toEqual([
      "font-size: 0.8rem",
      "font-weight: bold",
      "line-height: 1.3",
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
      .e { line-height: 1; }
    `;
    expect(cssLiterals(css)).toEqual([]);
    expect(cssLiterals(".a { line-height: normal; line-height: 1.0; }")).toHaveLength(2);
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
    expect(roleFor(".a | font-weight: 500")).toContain("--font-weight-medium");
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

  it("holds the role sheet and the other primitives to no literal at all", () => {
    const found = literalsByFile();
    expect(found.get("primitives/text.css")).toBeUndefined();
    expect(existsSync(join(SRC, "type.guard.allowance.ts"))).toBe(false);
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
