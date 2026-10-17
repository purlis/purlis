// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  classNameOf,
  classUtilities,
  type Declaration,
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
 * a **margin**, a **gap**, a **corner**, a **font size** or an **offset** (`top`, `right`,
 * `bottom`, `left`, `inset` and its forms) takes it from `theme/geometry.css`, as
 * `var(--space-4)`, `var(--radius-md)` or `var(--font-size-sm)`. A Tailwind class is read by the
 * step it names (`p-5` is off the scale) or by the value it writes of its own (`p-[7px]`,
 * `text-[13px]`, `rounded-[5px]`, read as the same value in a stylesheet).
 *
 * **A box's size is not a step of the scale** (#2319). A width, a height and their min, max,
 * block and inline forms say how big one named box is (a dialog's width, an icon's square, a
 * strip's cap on a tab), and the scale has no step for them, so this guard does not read them.
 * A control's or a strip's height is still `--size-control` or `--size-chrome`, by the
 * primitives that draw them.
 *
 * **What is not a literal.** Nothing (`0`, in any unit); a hairline (`1px`); one of the
 * element's own text (`1em`, `1lh`, `1ch`: an icon as big as the words beside it); a share of
 * the box or the window (`%`, `vh`, `vw`, `fr`); a keyword; and a token, or a token's whole
 * multiple or whole share inside `calc()` (`* 3`, `* -1`, `/ 2`). A multiplier or divisor that
 * is not a whole number is a literal (`calc(var(--space-1) * 3.5)`), but for the two structural
 * reserves sized in controls (`RESERVES`). A custom property that holds a length is held to the
 * rule of what reads it, in a stylesheet or in a component (its inline `style`, or a utility
 * such as `p-(--pad)`): one read by a padding, a margin, a gap, a corner, a font size or an
 * offset (or by a custom property that is), or read by nothing at all, is checked, so `--pad:
 * 7px; padding: var(--pad)` is still a literal, and so is a component's `"--pad": "7px"`; one
 * read only by a size or by another property this guard does not cover (a `translate`) is not.
 * A shadow's lengths are its elevation's (`--elevation-*`), which this guard does not cover.
 *
 * **There is no allowance.** The surface tickets (DS-8, DS-12, DS-13, and #2315 to #2319) took
 * every literal the window had onto a token, and the list they shrank is gone: a literal in what
 * this guard reads, as said above, fails here.
 */

/** The one stylesheet that gives the core tokens their values. */
const THE_CORE = "theme/geometry.css";

/** A property that sets a step of the scale: a padding, margin, gap, corner, or font size (with
 *  the `font` shorthand, which carries one), and an offset (`top`, `right`, `bottom`, `left`,
 *  `inset` and its forms), which places a box as a margin does. */
const GEOMETRY_PROPERTY =
  /^(?:padding(?:-[\w-]+)?|margin(?:-[\w-]+)?|(?:row-|column-)?gap|border(?:-[\w-]+)?-radius|font-size|font|top|right|bottom|left|inset(?:-[\w-]+)?)$/;

const CUSTOM = /^--[\w-]+$/;
const READS = /var\(\s*(--[\w-]+)/g;

/**
 * The custom properties held to the rule: each one a checked property reads, or a checked custom
 * property reads, and each one nothing reads at all, in the stylesheets or in the components
 * (`components`: their inline styles and utilities). The rest are read only by what this guard
 * does not cover (a size, a `translate`), and are not checked.
 */
export function checkedCustoms(
  sheets: readonly string[],
  components: readonly string[] = [],
): Set<string> {
  const all = [
    ...sheets.flatMap((css) => declarations(css)),
    ...components.flatMap((source) => componentDeclarations(source)),
  ];
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

/**
 * **The reserves that are not a whole number of controls** (D-2319-7), each written exactly so:
 * a structural reserve given as a padding or a margin, sized in controls rather than snapped.
 */
const RESERVES: readonly { written: string; why: string }[] = [
  {
    written: "var(--size-control) * 2.5",
    why: "the title bar's drag stretch (`--title-bar-drag`)",
  },
  {
    written: "var(--size-control) * 2.75",
    why: "the pane chips' room for the corner's controls before they are measured",
  },
];

/** A number a token is multiplied or divided by inside `calc()`: `* 3`, `3 *`, `/ 2`. */
const FACTOR = /([*/])\s*(-?\d*\.?\d+)(?![\w.%])|(?<![\w.-])(-?\d*\.?\d+)\s*\*/g;

/** The text inside each `calc()`, `min()`, `max()` and `clamp()` a value writes, where CSS does
 *  arithmetic; nested ones in their outer one too. */
function calcs(value: string): string[] {
  const found: string[] = [];
  for (const hit of value.matchAll(/(?<![\w-])(?:calc|min|max|clamp)\(/gi)) {
    let depth = 1;
    let end = hit.index + hit[0].length;
    for (; end < value.length && depth > 0; end++) {
      if (value[end] === "(") depth++;
      else if (value[end] === ")") depth--;
    }
    found.push(value.slice(hit.index + hit[0].length, end - 1));
  }
  return found;
}

/** Whether a value multiplies or divides by a number that is not whole: a token's multiple is a
 *  whole number of its steps, and its share a whole number of parts, but for the reserves. */
function multipliesALiteral(value: string): boolean {
  const kept = RESERVES.reduce((text, { written }) => {
    const exactly = new RegExp(`${written.replace(/[()*.]/g, "\\$&")}(?![\\d.])`, "g");
    return text.replace(exactly, "");
  }, value);
  return calcs(kept).some((body) =>
    [...body.matchAll(FACTOR)].some(([, , divisor, multiplier]) => {
      const factor = Number(divisor ?? multiplier);
      return !Number.isInteger(factor);
    }),
  );
}

/** Whether a value writes a literal size. */
export function writesALiteral(value: string): boolean {
  return (
    [...value.matchAll(LENGTH)].some(([length]) => isLiteral(length)) || multipliesALiteral(value)
  );
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

/** Each entry of a component's inline `style` that is written out (a number, or a string with
 *  nothing interpolated), as the property it sets: `paddingTop` is `padding-top`, and a custom
 *  property (`"--least"`) is its own name. */
function styleEntries(source: string): { key: string; property: string; value: string }[] {
  const found: { key: string; property: string; value: string }[] = [];
  const text = withoutComments(source, "ts");
  for (const style of text.matchAll(/\bstyle=\{\{([\s\S]*?)\}\}/g)) {
    for (const [, key, value] of style[1].matchAll(
      /(?<![\w$.])(\w+|"--[\w-]+"|'--[\w-]+')\s*:\s*(-?\d*\.?\d+(?![\w.])|"[^"]*"|'[^']*'|`[^`$]*`)/g,
    )) {
      const property = /^["']/.test(key)
        ? key.slice(1, -1)
        : key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);
      found.push({ key: key.replace(/^["']|["']$/g, ""), property, value });
    }
  }
  return found;
}

/**
 * A Tailwind utility that sets geometry from a value of its own, in brackets (`p-[7px]`,
 * `text-[13px]`, `rounded-tl-[5px]`) or a custom property in parentheses (`p-(--pad)`), as the
 * property it sets and the value it writes (`_` is a space). `md:` and the other variants are
 * read past.
 */
const ARBITRARY =
  /^(?:[\w-]+:)*-?(p[xytrblse]?|m[xytrblse]?|gap(?:-[xy])?|space-[xy]|inset(?:-[xy])?|top|right|bottom|left|rounded(?:-[trblse]{1,2})?|text)-(?:\[([^\]]+)\]|\((--[\w-]+)\))$/;

function arbitrary(word: string): { property: string; value: string } | undefined {
  const [, utility, written, custom] = ARBITRARY.exec(word) ?? [];
  if (utility === undefined) return undefined;
  const property = /^p/.test(utility)
    ? "padding"
    : /^(?:m|space)/.test(utility)
      ? "margin"
      : /^gap/.test(utility)
        ? "gap"
        : /^rounded/.test(utility)
          ? "border-radius"
          : utility === "text"
            ? "font-size"
            : "inset";
  return { property, value: custom ? `var(${custom})` : written.replace(/_/g, " ") };
}

/** What a component sets the way a stylesheet does, for `checkedCustoms`: each entry of its
 *  inline styles, and each utility that writes a value of its own. */
function componentDeclarations(source: string): Declaration[] {
  const found: Declaration[] = styleEntries(source).map(({ property, value }) => ({
    context: [],
    property,
    value: value.replace(/^["'`]|["'`]$/g, ""),
  }));
  for (const tag of openingTags(source, /^[A-Za-z]/)) {
    for (const word of classUtilities(classNameOf(tag.text))) {
      const set = arbitrary(word);
      if (set) found.push({ context: [], ...set });
    }
  }
  return found;
}

/**
 * Tailwind utilities that set geometry off the core scale. Its `--spacing` is the core's 4px
 * (`styles.css`), so a spacing step is core where it lands on the scale (0, 2, 4, 6, 8, 12, 16,
 * 24 or 32px); its text sizes are the core's and nothing else, so another one draws nothing;
 * its corners are the core's but for the four Tailwind keeps of its own. A size (`w-*`, `h-*`,
 * `size-*` and their min and max) is a box's, as in a stylesheet, and is not read. A value of
 * its own (`p-[7px]`, `text-[13px]`) is read as the same value in a stylesheet is.
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
  const own = arbitrary(word);
  if (own) return writesALiteral(own.value);
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
 *  and in a `className`, as `class <utility>`. A custom property it sets inline is read by what
 *  reads it in `customs` (`checkedCustoms`), as one a stylesheet sets. */
export function componentLiterals(
  source: string,
  customs: ReadonlySet<string> = new Set(),
): string[] {
  const found: string[] = [];
  for (const { key, property, value } of styleEntries(source)) {
    if (!(CUSTOM.test(property) ? customs.has(property) : GEOMETRY_PROPERTY.test(property))) {
      continue;
    }
    const literal = /^-?\d/.test(value) ? Number(value) !== 0 : writesALiteral(value.slice(1, -1));
    if (literal) found.push(`style ${key}: ${value}`);
  }
  for (const tag of openingTags(source, /^[A-Za-z]/)) {
    for (const word of classUtilities(classNameOf(tag.text))) {
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

/** Every literal in the window, by file: its stylesheets (the core aside) and its components,
 *  as they are or with a file's text replaced (`planted`). A custom property is held by what
 *  reads it in any of them. */
function literalsByFile(planted: Readonly<Record<string, string>> = {}): Map<string, string[]> {
  const text = (file: string) => planted[file] ?? read(file);
  const byFile = new Map<string, string[]>();
  const css = new Map([...sheets().keys()].map((file) => [file, text(file)]));
  const views = new Map(sourceFiles(isView).map((file) => [file, text(file)]));
  const customs = checkedCustoms([...css.values()], [...views.values()]);
  for (const [file, sheet] of css) byFile.set(file, cssLiterals(sheet, customs));
  for (const [file, source] of views) byFile.set(file, componentLiterals(source, customs));
  return new Map([...byFile].filter(([, found]) => found.length > 0));
}

/** Each literal found, as a complaint: with no allowance and no ceiling, every one is new
 *  (`ratchet`). */
export function complaints(found: ReadonlyMap<string, readonly string[]>): string[] {
  return ratchet(found, {}, {}, { added: "new literal, take it from a core token" });
}

describe("the geometry guard (#1837)", () => {
  it("finds every padding, margin, gap, corner, type size and offset in the window on the core tokens", () => {
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

  it("fails an offset and a token's odd multiple planted in the window's own stylesheet", () => {
    const planted = `${read("App.css")}
.planted { top: 3px; inset-inline-end: 0.3rem; margin: calc(var(--space-1) * 3.5); }
.planted-too { left: var(--space-4); padding: calc(var(--space-5) * 3) calc(var(--space-2) / 2); }
`;
    expect(complaints(literalsByFile({ "App.css": planted }))).toEqual([
      "App.css: new literal, take it from a core token: .planted | top: 3px",
      "App.css: new literal, take it from a core token: .planted | inset-inline-end: 0.3rem",
      "App.css: new literal, take it from a core token: .planted | margin: calc(var(--space-1) * 3.5)",
    ]);
  });

  it("fails a Tailwind arbitrary value planted in a real component", () => {
    const planted = `${read("PlaneView.tsx")}
export const Planted = () => (
  <div className="p-[7px] md:text-[13px] rounded-[5px] gap-x-[0.3rem] -mt-[3px] space-y-[9px] top-[2px] p-[var(--space-2)]" />
);
`;
    expect(complaints(literalsByFile({ "PlaneView.tsx": planted }))).toEqual([
      "PlaneView.tsx: new literal, take it from a core token: class p-[7px]",
      "PlaneView.tsx: new literal, take it from a core token: class md:text-[13px]",
      "PlaneView.tsx: new literal, take it from a core token: class rounded-[5px]",
      "PlaneView.tsx: new literal, take it from a core token: class gap-x-[0.3rem]",
      "PlaneView.tsx: new literal, take it from a core token: class -mt-[3px]",
      "PlaneView.tsx: new literal, take it from a core token: class space-y-[9px]",
      "PlaneView.tsx: new literal, take it from a core token: class top-[2px]",
    ]);
  });

  it("holds a custom property a component's inline style spaces with, in the real tree", () => {
    const css = `${read("App.css")}\n.planted { --planted-pad: 7px; width: var(--planted-pad); }\n`;
    const tsx = `${read("PlaneView.tsx")}
export const Planted = () => <div style={{ padding: "var(--planted-pad)" }} />;
`;
    expect(complaints(literalsByFile({ "App.css": css }))).toEqual([]);
    expect(complaints(literalsByFile({ "App.css": css, "PlaneView.tsx": tsx }))).toEqual([
      "App.css: new literal, take it from a core token: .planted | --planted-pad: 7px",
    ]);
    // Read through a utility's own spelling of a custom property, too.
    const utility = `${read("PlaneView.tsx")}
export const Planted = () => <div className="p-(--planted-pad)" />;
`;
    expect(complaints(literalsByFile({ "App.css": css, "PlaneView.tsx": utility }))).toEqual([
      "App.css: new literal, take it from a core token: .planted | --planted-pad: 7px",
    ]);
  });

  it("holds a custom property a component sets inline to what reads it", () => {
    const css = `${read("App.css")}\n.planted { padding: var(--planted-pad); }\n`;
    const tsx = `${read("PlaneView.tsx")}
export const Planted = () => <div style={{ "--planted-pad": "7px", "--planted-at": "9px" }} />;
`;
    expect(complaints(literalsByFile({ "App.css": css, "PlaneView.tsx": tsx }))).toEqual([
      'PlaneView.tsx: new literal, take it from a core token: style --planted-pad: "7px"',
      'PlaneView.tsx: new literal, take it from a core token: style --planted-at: "9px"',
    ]);
  });

  it("passes a whole multiple or a whole share of a token, and the named reserves alone", () => {
    const css = `
      .a { padding: calc(var(--space-5) * 3); margin: calc(-1 * var(--space-2)); }
      .b { gap: calc(var(--space-4) / 2); margin-inline: calc(var(--space-3) * -1); }
      .c { padding-inline-end: calc(var(--size-control) * 2.5); }
      .d { margin-right: var(--x, calc(var(--size-control) * 2.75)); }
      .e { padding-block: calc((var(--size-control) - 1lh) / 2 - 1px); }
      .f { inline-size: calc(var(--space-6) * 1.6); }
    `;
    expect(cssLiterals(css)).toEqual([]);
    const odd = `
      .a { padding: calc(var(--space-1) * 3.5); margin: calc(0.5 * var(--space-4)); }
      .b { gap: calc(var(--space-4) / 2.5); margin: calc(var(--space-5) * 2.5); }
      .c { padding: calc((var(--space-1) + var(--space-2)) * 1.5); }
      .d { padding-inline-end: calc(var(--size-control) * 2.55); gap: max(var(--space-1) * 1.5, 1px); }
    `;
    expect(cssLiterals(odd)).toEqual([
      ".a | padding: calc(var(--space-1) * 3.5)",
      ".a | margin: calc(0.5 * var(--space-4))",
      ".b | gap: calc(var(--space-4) / 2.5)",
      ".b | margin: calc(var(--space-5) * 2.5)",
      ".c | padding: calc((var(--space-1) + var(--space-2)) * 1.5)",
      ".d | padding-inline-end: calc(var(--size-control) * 2.55)",
      ".d | gap: max(var(--space-1) * 1.5, 1px)",
    ]);
  });

  it("reads an offset (top, right, bottom, left, inset) as it reads a margin", () => {
    const css = `
      .a { top: 3px; right: 8px; bottom: -2px; left: 0.5rem; }
      .b { inset: 0 4px; inset-inline-end: 0.3rem; inset-block-start: 1rem; }
      .c { top: 0; left: 50%; top: 8vh; right: var(--space-4); inset-inline: 0; bottom: 1px; }
    `;
    expect(cssLiterals(css).map((entry) => entry.split(" | ")[1])).toEqual([
      "top: 3px",
      "right: 8px",
      "bottom: -2px",
      "left: 0.5rem",
      "inset: 0 4px",
      "inset-inline-end: 0.3rem",
      "inset-block-start: 1rem",
    ]);
    const tsx = `<div style={{ top: 3, insetInlineEnd: "0.3rem", left: x, bottom: 0 }} />`;
    expect(componentLiterals(tsx)).toEqual(["style top: 3", 'style insetInlineEnd: "0.3rem"']);
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
      "top",
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

  it("reads a Tailwind arbitrary value by what it writes, and leaves a colour", () => {
    const tsx = `
      <p className="p-[7px] px-[var(--space-3)] m-[calc(var(--space-1)*3.5)] mx-[calc(var(--space-1)*3)]" />
      <p className="text-[13px] text-[var(--font-size-sm)] text-[#fff] rounded-tl-[5px] rounded-[0]" />
      <p className={cn("inset-x-[1rem] w-[37px] gap-(--gap) sm:-ml-[0.3rem]", on && "py-[1px]")} />
    `;
    expect(componentLiterals(tsx)).toEqual([
      "class p-[7px]",
      "class m-[calc(var(--space-1)*3.5)]",
      "class text-[13px]",
      "class rounded-tl-[5px]",
      "class inset-x-[1rem]",
      "class sm:-ml-[0.3rem]",
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
