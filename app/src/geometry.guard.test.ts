// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { GEOMETRY_ALLOWANCE } from "./geometry.guard.allowance";
import {
  classNameOf,
  classWords,
  declarations,
  isView,
  openingTags,
  ratchet,
  SRC,
  sourceFiles,
  withoutComments,
} from "./guards.testkit";

/**
 * **No size, gap, corner or type size is written outside the core tokens** (DS-4, #1837;
 * ADR 0094).
 *
 * Colour and motion stayed consistent because a guard refused a literal the day it was typed.
 * Geometry had no guard, and drifted to 132 paddings, 20 font sizes and 11 radii. This is that
 * guard for geometry: a stylesheet, an inline style or a Tailwind class that sets a **size**
 * (width, height and their min, max, block and inline forms), a **padding**, a **margin**, a
 * **gap**, a **corner** or a **font size** takes it from `theme/geometry.css`, as
 * `var(--space-4)`, `var(--size-control)`, `var(--radius-md)` or `var(--font-size-sm)`.
 *
 * **What is not a literal.** Nothing (`0`, in any unit); a hairline (`1px`); one of the
 * element's own text (`1em`, `1lh`, `1ch`: an icon as big as the words beside it); a share of
 * the box or the window (`%`, `vh`, `vw`, `fr`); a keyword; and a token, or a token's multiple
 * or share inside `calc()`. A custom property that holds a length is held to the same rule, so
 * `--pad: 7px; padding: var(--pad)` is still a literal; a shadow's lengths are its elevation's
 * (`--elevation-*`), which this guard does not cover.
 *
 * **Today's offenders are allowed, one per line, and the list can only shrink**
 * (`geometry.guard.allowance.ts`). Each entry names a declaration by the file, the rule it is
 * in and what it sets, never by line, so editing above it does not move it. The list must match
 * the source exactly: a new literal fails here, and so does an entry whose declaration was
 * tokenised or deleted, until the entry is deleted too. No file's list may grow past what it
 * was when the guard landed (`CEILING`), and a file not on it may never get one. The surface
 * tickets (DS-8, DS-12, DS-13 and on) delete entries as they migrate; a line each, so their
 * removals merge cleanly.
 */

/** The one stylesheet that gives the core tokens their values. */
const THE_CORE = "theme/geometry.css";

/** A property that sets geometry: a size, padding, margin, gap, corner, or font size (with the
 *  `font` shorthand, which carries one). A custom property is checked as one too. */
const GEOMETRY_PROPERTY =
  /^(?:--[\w-]+|(?:min-|max-)?(?:width|height|block-size|inline-size)|padding(?:-[\w-]+)?|margin(?:-[\w-]+)?|(?:row-|column-)?gap|border(?:-[\w-]+)?-radius|font-size|font)$/;

/** A length written as a number: `7px`, `.35rem`, `-0.25rem`, `1.5em`. Not inside a name, so
 *  `--space-2` or `h2em` is not one. */
const LENGTH = /(?<![\w.#-])-?(\d*\.?\d+)(px|rem|em|ch|ex|lh|pt|pc|cm|mm|in|q)\b/gi;

/** The lengths that are not a literal size: nothing, a hairline, or one of the text's own. */
function isLiteral(length: string): boolean {
  const [, number, unit] = /^-?(\d*\.?\d+)([a-z]+)$/i.exec(length) ?? [];
  const value = Number(number);
  if (value === 0) return false;
  if (value === 1 && ["px", "em", "lh", "ch"].includes(unit.toLowerCase())) return false;
  return true;
}

/** Whether a value writes a literal size. */
export function writesALiteral(value: string): boolean {
  return [...value.matchAll(LENGTH)].some(([length]) => isLiteral(length));
}

/** Each literal geometry declaration in a stylesheet, as its allowance entry:
 *  `<rule> | <property>: <value>`, the rule being its selectors and at-rules, outermost first. */
export function cssLiterals(css: string): string[] {
  return declarations(css)
    .filter(({ property }) => GEOMETRY_PROPERTY.test(property) && !/shadow/.test(property))
    .filter(({ value }) => writesALiteral(value))
    .map(({ context, property, value }) => `${context.join(" › ")} | ${property}: ${value}`);
}

/** A React style key that sets geometry. */
const STYLE_KEY =
  /^(?:(?:min|max)?(?:Width|Height)|width|height|(?:min|max)?(?:BlockSize|InlineSize)|blockSize|inlineSize|padding\w*|margin\w*|gap|rowGap|columnGap|border\w*Radius|fontSize)$/;

/**
 * Tailwind utilities that set geometry off the core scale. Its `--spacing` is the core's 4px
 * (`styles.css`), so a spacing step is core where it lands on the scale (0, 2, 4, 6, 8, 12, 16,
 * 24 or 32px); its text sizes are the core's and nothing else, so another one draws nothing;
 * its corners are the core's but for the four Tailwind keeps of its own.
 */
const ON_THE_SCALE = new Set(["0", "px", "0.5", "1", "1.5", "2", "3", "4", "6", "8"]);
const SPACING_UTILITY =
  /^-?(?:p[xytrblse]?|m[xytrblse]?|gap(?:-[xy])?|space-(?:x|y)|w|h|size|(?:min|max)-(?:w|h)|inset(?:-[xy])?|top|right|bottom|left)-(\d+(?:\.\d+)?|px)$/;
const TEXT_UTILITY = /^text-(\d?xs|sm|md|base|lg|xl|[2-9]xl)$/;
const CORE_TEXT = new Set(["xs", "sm", "md", "base", "lg", "xl"]);
const RADIUS_UTILITY = /^rounded(?:-[trblse]{1,2})?(?:-(xs|sm|md|lg|xl|2xl|3xl|4xl|none|full))?$/;
const CORE_RADIUS = new Set(["sm", "md", "lg", "xl", "none", "full"]);

/** Whether one Tailwind class sets geometry off the core. `md:p-5` is read as `p-5`. */
export function offTheScale(word: string): boolean {
  const utility = word.split(":").pop() ?? word;
  const spacing = SPACING_UTILITY.exec(utility);
  if (spacing) return !ON_THE_SCALE.has(spacing[1]);
  const text = TEXT_UTILITY.exec(utility);
  if (text) return !CORE_TEXT.has(text[1]);
  const radius = RADIUS_UTILITY.exec(utility);
  if (radius) return !CORE_RADIUS.has(radius[1] ?? "");
  return false;
}

/** Each literal geometry a component writes: in an inline `style`, as `style <key>: <value>`,
 *  and in a `className`, as `class <utility>`. */
export function componentLiterals(source: string): string[] {
  const found: string[] = [];
  const text = withoutComments(source, "ts");
  for (const style of text.matchAll(/\bstyle=\{\{([\s\S]*?)\}\}/g)) {
    for (const [, key, value] of style[1].matchAll(
      /(?<![\w$.])(\w+)\s*:\s*(-?\d*\.?\d+(?![\w.])|"[^"]*"|'[^']*'|`[^`$]*`)/g,
    )) {
      if (!STYLE_KEY.test(key)) continue;
      const literal = /^-?\d/.test(value)
        ? Number(value) !== 0
        : writesALiteral(value.slice(1, -1));
      if (literal) found.push(`style ${key}: ${value}`);
    }
  }
  for (const tag of openingTags(source, /^[A-Za-z]/)) {
    for (const word of classWords(classNameOf(tag.text))) {
      if (offTheScale(word)) found.push(`class ${word}`);
    }
  }
  return found;
}

/** Every literal in the window today, by file: its stylesheets (the core aside) and its
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

/**
 * How long each file's list was when the guard landed: a list may only get shorter than this.
 * This is a frozen record, not a count to keep current. Lowering a ceiling is never needed;
 * raising one, or naming a new file, is adding a literal the guard exists to refuse.
 */
const CEILING: Record<string, number> = {
  "App.css": 595,
};

/** What differs between the literals in the window and the list (`ratchet`). */
export function offTheList(
  found: ReadonlyMap<string, readonly string[]>,
  allowed: Readonly<Record<string, readonly string[]>>,
  ceiling: Readonly<Record<string, number>> = CEILING,
): string[] {
  return ratchet(found, allowed, ceiling, {
    added: "new literal, take it from a core token",
  });
}

describe("the geometry guard (#1837)", () => {
  it("finds every size, gap, corner and type size in the window on the core tokens, or allowed", () => {
    expect(offTheList(literalsByFile(), GEOMETRY_ALLOWANCE)).toEqual([]);
  });

  it("fails a raw padding planted in a stylesheet", () => {
    const planted = new Map([["App.css", cssLiterals(".planted { padding: 7px; }")]]);
    expect(offTheList(planted, {}, { "App.css": 10 })).toEqual([
      "App.css: new literal, take it from a core token: .planted | padding: 7px",
    ]);
  });

  it("fails an entry its declaration no longer bears out, and a list that grew", () => {
    const allowed = { "App.css": [".a | padding: 7px", ".b | margin: 3px"] };
    expect(
      offTheList(new Map([["App.css", [".a | padding: 7px"]]]), allowed, { "App.css": 2 }),
    ).toEqual(["App.css: stale entry, delete it from the allowance: .b | margin: 3px"]);
    const both = [".a | padding: 7px", ".b | margin: 3px"];
    expect(offTheList(new Map([["App.css", both]]), allowed, { "App.css": 1 })).toEqual([
      "App.css: the allowance grew to 2, past 1",
    ]);
    expect(offTheList(new Map([["Fresh.css", both]]), { "Fresh.css": both }, {})).toEqual([
      "Fresh.css: the allowance grew to 2, past 0",
    ]);
  });

  it("counts a declaration written twice as two", () => {
    const twice = [".a | gap: 3px", ".a | gap: 3px"];
    expect(
      offTheList(new Map([["x.css", twice]]), { "x.css": [twice[0]] }, { "x.css": 2 }),
    ).toEqual(["x.css: new literal, take it from a core token: .a | gap: 3px"]);
  });

  it("reads each geometry property, and the literals in it, and nothing else", () => {
    const css = `
      .a { padding: 7px; margin-inline: 0 .35rem; gap: 2px; row-gap: 1rem; }
      .b { width: 12rem; min-height: 1.5em; max-inline-size: 60ch; block-size: 3lh; }
      .c { border-radius: 5px; border-top-left-radius: 2px; font-size: 13px; }
      .d { font: 600 12px/1.2 system-ui; --indent: .85rem; margin-top: -3px; }
      .e { line-height: 1.4; border: 2px solid; outline-offset: 3px; top: 4px; }
    `;
    expect(cssLiterals(css).map((entry) => entry.split(" | ")[1].split(":")[0])).toEqual([
      "padding",
      "margin-inline",
      "gap",
      "row-gap",
      "width",
      "min-height",
      "max-inline-size",
      "block-size",
      "border-radius",
      "border-top-left-radius",
      "font-size",
      "font",
      "--indent",
      "margin-top",
    ]);
  });

  it("passes the tokens, nothing, a hairline, the text's own unit, and shares", () => {
    const css = `
      .a { padding: var(--space-3) var(--space-4); gap: 0; margin: 0px auto; }
      .b { height: 1px; width: 1em; min-height: 1lh; max-width: 100%; max-height: 80vh; }
      .c { padding-block: calc((var(--size-control) - 1lh) / 2 - 1px); }
      .d { inline-size: calc(var(--space-5) * 3); border-radius: var(--radius-md); }
      .e { font-size: var(--font-size-sm); font: inherit; --row: var(--size-chrome); }
      .f { --shadow-md: 0 4px 6px var(--overlay-shadow); width: calc(100% - var(--space-4)); }
    `;
    expect(cssLiterals(css)).toEqual([]);
  });

  it("names a declaration by its rule, inside media queries and nesting, comments aside", () => {
    const css = `
      /* .commented { padding: 9px; } */
      @media (max-width: 900px) { .a, .b { padding: 3px } }
      .c { &:hover { margin: 5px; } }
      .d { background: url("data:image/svg+xml;utf8,<svg width='9px'/>"); width: 9px; }
    `;
    expect(cssLiterals(css)).toEqual([
      "@media (max-width: 900px) › .a, .b | padding: 3px",
      ".c › &:hover | margin: 5px",
      ".d | width: 9px",
    ]);
  });

  it("reads an inline style's literal sizes, and leaves its variables and zeros", () => {
    const tsx = `
      <div style={{ padding: 7, width: "12px", marginTop: '0.5rem', fontSize: \`13px\` }} />
      <div style={{ height: 0, gap: size, minWidth: "100%", width: "1px", flex: 1 }} />
    `;
    expect(componentLiterals(tsx)).toEqual([
      "style padding: 7",
      'style width: "12px"',
      "style marginTop: '0.5rem'",
      "style fontSize: `13px`",
    ]);
  });

  it("reads a Tailwind class off the core scale, and passes one on it", () => {
    const tsx = `
      <p className="p-5 gap-1 px-1.5 my-1 text-2xl text-sm rounded-2xl rounded-md md:mt-7" />
      <p className={cn("w-full w-0 h-px text-text-primary text-left rounded", on && "-mt-2")} />
    `;
    expect(componentLiterals(tsx)).toEqual([
      "class p-5",
      "class text-2xl",
      "class rounded-2xl",
      "class md:mt-7",
      "class rounded",
    ]);
  });

  it("lists by file and in order, so a removal is one line in one place", () => {
    for (const [file, entries] of Object.entries(GEOMETRY_ALLOWANCE)) {
      expect(entries.length, `${file}: an empty list is deleted, not kept`).toBeGreaterThan(0);
    }
    expect(Object.keys(GEOMETRY_ALLOWANCE)).toEqual(Object.keys(GEOMETRY_ALLOWANCE).sort());
    const source = readFileSync(join(SRC, "geometry.guard.allowance.ts"), "utf8");
    const entryLines = source.split("\n").filter((line) => /^\s+"(?:[^"\\]|\\.)*",$/.test(line));
    const total = Object.values(GEOMETRY_ALLOWANCE).flat().length;
    expect(entryLines.length, "one entry per line").toBe(total);
  });

  it("covers the window's stylesheets and components, so a reader that found nothing fails", () => {
    expect(sourceFiles((name) => name.endsWith(".css"))).toContain("App.css");
    expect(sourceFiles(isView)).toContain("PlaneView.tsx");
    expect(sourceFiles(isView).some((file) => file.includes(".test."))).toBe(false);
    expect(declarations(readFileSync(join(SRC, "App.css"), "utf8")).length).toBeGreaterThan(1000);
  });
});
