// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
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
 * **No spacing, corner or type size is written outside the core tokens** (DS-4, #1837;
 * ADR 0094).
 *
 * Colour and motion stayed consistent because a guard refused a literal the day it was typed.
 * Geometry had no guard, and drifted to 132 paddings, 20 font sizes and 11 radii. This is that
 * guard for geometry: a stylesheet, an inline style or a Tailwind class that sets a **padding**,
 * a **margin**, a **gap**, a **corner** or a **font size** takes it from `theme/geometry.css`, as
 * `var(--space-4)`, `var(--radius-md)` or `var(--font-size-sm)`.
 *
 * **A box's size is not a step of the scale** (#2319). A width, a height and their min, max,
 * block and inline forms say how big one named box is (a dialog's width, an icon's square, a
 * strip's cap on a tab), and the scale has no step for them, so this guard does not read them.
 * A control's or a strip's height is still `--size-control` or `--size-chrome`, which the
 * control guard holds (`controls.guard.test.ts`, #2449).
 *
 * **What is not a literal.** Nothing (`0`, in any unit); a hairline (`1px`); one of the
 * element's own text (`1em`, `1lh`, `1ch`: an icon as big as the words beside it); a share of
 * the box or the window (`%`, `vh`, `vw`, `fr`); a keyword; and a token, or a token's multiple
 * or share inside `calc()`. A custom property that holds a length is held to the rule of what
 * reads it: one read by a padding, a margin, a gap, a corner or a font size (or by a custom
 * property that is), or read by no stylesheet at all, is checked, so `--pad: 7px; padding:
 * var(--pad)` is still a literal; one read only by a size or by another property this guard
 * does not cover (a `translate`) is not. A shadow's lengths are its elevation's
 * (`--elevation-*`), which this guard does not cover.
 *
 * **There is no allowance.** The surface tickets (DS-8, DS-12, DS-13, and #2315 to #2319) took
 * every literal the window had onto a token, and the list they shrank is gone: a literal in what
 * this guard reads, as said above, fails here. Widening what it reads is #2450.
 */

/** The one stylesheet that gives the core tokens their values. */
const THE_CORE = "theme/geometry.css";

/** A property that sets a step of the scale: a padding, margin, gap, corner, or font size (with
 *  the `font` shorthand, which carries one). */
const GEOMETRY_PROPERTY =
  /^(?:padding(?:-[\w-]+)?|margin(?:-[\w-]+)?|(?:row-|column-)?gap|border(?:-[\w-]+)?-radius|font-size|font)$/;

const CUSTOM = /^--[\w-]+$/;
const READS = /var\(\s*(--[\w-]+)/g;

/**
 * The custom properties held to the rule: each one a checked property reads, or a checked custom
 * property reads, and each one no stylesheet reads at all. The rest are read only by what this
 * guard does not cover (a size, a `translate`), and are not checked.
 */
export function checkedCustoms(sheets: readonly string[]): Set<string> {
  const all = sheets.flatMap((css) => declarations(css));
  const declared = new Set(
    all.filter(({ property }) => CUSTOM.test(property)).map((d) => d.property),
  );
  const read = new Set<string>();
  const checked = new Set<string>();
  const readBy = (value: string) => [...value.matchAll(READS)].map(([, name]) => name);
  for (const { property, value } of all) {
    for (const name of readBy(value)) {
      read.add(name);
      if (GEOMETRY_PROPERTY.test(property)) checked.add(name);
    }
  }
  for (let grew = true; grew;) {
    grew = false;
    for (const { property, value } of all) {
      if (!checked.has(property)) continue;
      for (const name of readBy(value)) {
        if (!checked.has(name)) {
          checked.add(name);
          grew = true;
        }
      }
    }
  }
  for (const name of declared) if (!read.has(name)) checked.add(name);
  return checked;
}

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

/** Each literal geometry declaration in a stylesheet, as `<rule> | <property>: <value>`, the
 *  rule being its selectors and at-rules, outermost first. A custom property is read by what
 *  reads it in `customs` (`checkedCustoms`), by default this stylesheet alone. */
export function cssLiterals(
  css: string,
  customs: ReadonlySet<string> = checkedCustoms([css]),
): string[] {
  return declarations(css)
    .filter(
      ({ property }) =>
        (GEOMETRY_PROPERTY.test(property) || customs.has(property)) && !/shadow/.test(property),
    )
    .filter(({ value }) => writesALiteral(value))
    .map(({ context, property, value }) => `${context.join(" › ")} | ${property}: ${value}`);
}

/** A React style key that sets a step of the scale. */
const STYLE_KEY = /^(?:padding\w*|margin\w*|gap|rowGap|columnGap|border\w*Radius|fontSize)$/;

/**
 * Tailwind utilities that set geometry off the core scale. Its `--spacing` is the core's 4px
 * (`styles.css`), so a spacing step is core where it lands on the scale (0, 2, 4, 6, 8, 12, 16,
 * 24 or 32px); its text sizes are the core's and nothing else, so another one draws nothing;
 * its corners are the core's but for the four Tailwind keeps of its own. A size (`w-*`, `h-*`,
 * `size-*` and their min and max) is a box's, as in a stylesheet, and is not read.
 */
const ON_THE_SCALE = new Set(["0", "px", "0.5", "1", "1.5", "2", "3", "4", "6", "8"]);
const SPACING_UTILITY =
  /^-?(?:p[xytrblse]?|m[xytrblse]?|gap(?:-[xy])?|space-(?:x|y)|inset(?:-[xy])?|top|right|bottom|left)-(\d+(?:\.\d+)?|px)$/;
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

const read = (file: string) => readFileSync(join(SRC, file), "utf8");

/** The window's stylesheets, the core aside, by file. */
function sheets(): Map<string, string> {
  const files = sourceFiles((name) => name.endsWith(".css")).filter((file) => file !== THE_CORE);
  return new Map(files.map((file) => [file, read(file)]));
}

/** Every literal in the window today, by file: its stylesheets (the core aside) and its
 *  components. A custom property is held by what reads it in any of them. */
function literalsByFile(): Map<string, string[]> {
  const byFile = new Map<string, string[]>();
  const css = sheets();
  const customs = checkedCustoms([...css.values()]);
  for (const [file, text] of css) byFile.set(file, cssLiterals(text, customs));
  for (const file of sourceFiles(isView)) byFile.set(file, componentLiterals(read(file)));
  return new Map([...byFile].filter(([, found]) => found.length > 0));
}

/** Each literal found, as a complaint: with no allowance and no ceiling, every one is new
 *  (`ratchet`). */
export function complaints(found: ReadonlyMap<string, readonly string[]>): string[] {
  return ratchet(found, {}, {}, { added: "new literal, take it from a core token" });
}

describe("the geometry guard (#1837)", () => {
  it("finds every padding, margin, gap, corner and type size in the window on the core tokens", () => {
    expect(complaints(literalsByFile())).toEqual([]);
  });

  it("fails a hand-written padding or font size planted in the window's own stylesheet", () => {
    const css = sheets();
    const planted = `${css.get("App.css")}\n.planted { padding: 7px; font-size: 13px; }\n`;
    const found = new Map([
      ...literalsByFile(),
      ["App.css", cssLiterals(planted, checkedCustoms([...css.values(), planted]))],
    ]);
    expect(complaints(found)).toEqual([
      "App.css: new literal, take it from a core token: .planted | padding: 7px",
      "App.css: new literal, take it from a core token: .planted | font-size: 13px",
    ]);
  });

  it("fails a padding taken from a custom property that holds a literal, in the real stylesheet", () => {
    const css = sheets();
    const planted = `${css.get("App.css")}\n.planted { --pad: 7px; padding: var(--pad); }\n`;
    const found = new Map([
      ...literalsByFile(),
      ["App.css", cssLiterals(planted, checkedCustoms([...css.values(), planted]))],
    ]);
    expect(complaints(found)).toEqual([
      "App.css: new literal, take it from a core token: .planted | --pad: 7px",
    ]);
  });

  it("gives the step under the words' floor to the scope switch alone (#2319)", () => {
    const readers = [...sheets()].flatMap(([file, text]) =>
      declarations(text)
        .filter(({ value }) => value.includes("--font-size-2xs"))
        .map(({ context }) => `${file} ${context.join(" › ")}`),
    );
    expect(readers).toEqual(["App.css .ui-levels-small"]);
    const views = sourceFiles(isView).filter((file) => read(file).includes("--font-size-2xs"));
    expect(views).toEqual([]);
  });

  it("counts a declaration written twice as two", () => {
    const twice = [".a | gap: 3px", ".a | gap: 3px"];
    expect(complaints(new Map([["x.css", twice]]))).toHaveLength(2);
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
      "border-radius",
      "border-top-left-radius",
      "font-size",
      "font",
      "--indent",
      "margin-top",
    ]);
  });

  it("holds a custom property to what reads it", () => {
    const css = `
      .a { --pad: 7px; padding: var(--pad); }
      .b { --inner: 5px; --outer: calc(var(--inner) * 2); margin: var(--outer); }
      .c { --wide: 30rem; max-inline-size: var(--wide); --from: 0 -0.25rem; translate: var(--from); }
      .d { --unread: 9px; }
      .e { --both: 3px; width: var(--both); gap: var(--both); }
    `;
    expect(cssLiterals(css)).toEqual([
      ".a | --pad: 7px",
      ".b | --inner: 5px",
      ".d | --unread: 9px",
      ".e | --both: 3px",
    ]);
    // Read in another stylesheet counts: a size there is a size here.
    const reader = ".menu { max-inline-size: var(--menu-max); }";
    expect(cssLiterals(".tasks { --menu-max: 30rem; }", checkedCustoms([reader]))).toEqual([]);
    expect(cssLiterals(".tasks { --menu-max: 30rem; }")).toEqual([".tasks | --menu-max: 30rem"]);
  });

  it("passes the tokens, nothing, a hairline, the text's own unit, shares, and a box's size", () => {
    const css = `
      .a { padding: var(--space-3) var(--space-4); gap: 0; margin: 0px auto; }
      .b { height: 1px; width: 1em; min-height: 1lh; max-width: 100%; max-height: 80vh; }
      .c { padding-block: calc((var(--size-control) - 1lh) / 2 - 1px); }
      .d { inline-size: calc(var(--space-5) * 3); border-radius: var(--radius-md); }
      .e { font-size: var(--font-size-sm); font: inherit; --row: var(--size-chrome); }
      .f { --shadow-md: 0 4px 6px var(--overlay-shadow); width: calc(100% - var(--space-4)); }
      .g { width: 34rem; min-height: 1.4rem; max-block-size: 14rem; }
    `;
    expect(cssLiterals(css)).toEqual([]);
  });

  it("names a declaration by its rule, inside media queries and nesting, comments aside", () => {
    const css = `
      /* .commented { padding: 9px; } */
      @media (max-width: 900px) { .a, .b { padding: 3px } }
      .c { &:hover { margin: 5px; } }
      .d { background: url("data:image/svg+xml;utf8,<svg width='9px'/>"); margin: 9px; }
    `;
    expect(cssLiterals(css)).toEqual([
      "@media (max-width: 900px) › .a, .b | padding: 3px",
      ".c › &:hover | margin: 5px",
      ".d | margin: 9px",
    ]);
  });

  it("reads an inline style's literal spacing, corners and sizes of text, and leaves the rest", () => {
    const tsx = `
      <div style={{ padding: 7, width: "12px", marginTop: '0.5rem', fontSize: \`13px\` }} />
      <div style={{ height: 0, gap: size, minWidth: "100%", borderRadius: "1px", flex: 1 }} />
    `;
    expect(componentLiterals(tsx)).toEqual([
      "style padding: 7",
      "style marginTop: '0.5rem'",
      "style fontSize: `13px`",
    ]);
  });

  it("reads a Tailwind class off the core scale, and passes one on it", () => {
    const tsx = `
      <p className="p-5 gap-1 px-1.5 my-1 text-2xl text-sm rounded-2xl rounded-md md:mt-7" />
      <p className={cn("w-full w-7 h-px text-text-primary text-left rounded", on && "-mt-2")} />
    `;
    expect(componentLiterals(tsx)).toEqual([
      "class p-5",
      "class text-2xl",
      "class rounded-2xl",
      "class md:mt-7",
      "class rounded",
    ]);
  });

  it("keeps no allowance list: the lists the migrations shrank are gone", () => {
    expect(existsSync(join(SRC, "geometry.guard.allowance.ts"))).toBe(false);
  });

  it("covers the window's stylesheets and components, so a reader that found nothing fails", () => {
    expect(sourceFiles((name) => name.endsWith(".css"))).toContain("App.css");
    expect(sourceFiles(isView)).toContain("PlaneView.tsx");
    expect(sourceFiles(isView).some((file) => file.includes(".test."))).toBe(false);
    expect(declarations(readFileSync(join(SRC, "App.css"), "utf8")).length).toBeGreaterThan(1000);
    const checked = declarations(read("App.css")).filter(({ property }) =>
      GEOMETRY_PROPERTY.test(property),
    );
    expect(checked.length).toBeGreaterThan(500);
  });
});
