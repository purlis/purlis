// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  classNameOf,
  classUtilities,
  declarations,
  isScript,
  isView,
  openingTags,
  ratchet,
  SRC,
  sourceFiles,
  type StyleSource,
  styleEntries,
  unprefixed,
  utilityOf,
  windowFiles,
} from "./guards.testkit";
import { LENGTH_UNITS } from "./cssMath.testkit";
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
 * other way of ranking it. In a stylesheet, in a component's or a script's style as the
 * geometry guard reads it (`styleEntries`: an inline `style`, a style object held in a variable,
 * an assignment such as `el.style.fontSize = "13px"`, `el.style.cssText` or a `style` attribute
 * set as text, `Object.assign(el.style, { … })`, a `setProperty` call, each followed wherever
 * the file hands it on, #2566), or as a Tailwind utility
 * read past its variants as the
 * declarations it writes (`utilityDeclarations`: `font-semibold` is a weight of `600`,
 * `leading-[1.3]` and the `/7` of `text-sm/7` a line height, `text-[13px]` a size, `font-mono`
 * a family of Tailwind's own, `[font-weight:600]` what it says):
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

/** The forced-colours sheet (#2328): a text colour there is the system's, which forced colours
 *  draw in, and never a token. `theme/forced-colors.test.ts` holds every colour in it to the
 *  system's; any other type declaration in it is held here as anywhere. */
const FORCED = /^(?:theme|ui\/[\w-]+)\/forced-colors\.css$/;
const SYSTEM_TEXT = / \| color: (?:CanvasText|GrayText|HighlightText|MarkText|SelectedItemText)$/;

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
    .filter(({ property: name }) => unprefixed(name) in MAY_BE)
    .filter(({ property: name, value }) => !isOnTheScale(unprefixed(name), value))
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

/** Tailwind's weights, by name, as the numbers it gives them. */
const WEIGHTS: Readonly<Record<string, string>> = {
  thin: "100",
  extralight: "200",
  light: "300",
  normal: "400",
  medium: "500",
  semibold: "600",
  bold: "700",
  extrabold: "800",
  black: "900",
};

/** A text size Tailwind names: after one, a `/` gives the line height (`text-sm/7`); after a
 *  colour, it is the colour's opacity. */
const NAMED_SIZE = /^(?:\d?xs|sm|md|base|lg|xl|[2-9]xl)$/;

/** What a `text-[…]` writes when it is a size: a length in any unit of `LENGTH_UNITS`
 *  (`13px`, `0.8rem`, `1cap`), a share of the text (`90%`), or a size token. */
const SIZE = new RegExp(String.raw`\d(?:${LENGTH_UNITS.join("|")}|%)\b|--font-size-`);

/** A line height as Tailwind spells it after `leading-` or a size's `/`. */
function leading(spelled: string): string {
  if (spelled === "none") return "1";
  const own = /^\[(.+)\]$/.exec(spelled)?.[1]?.replace(/_/g, " ");
  const custom = /^\((--[\w-]+)\)$/.exec(spelled)?.[1];
  return own ?? (custom ? `var(${custom})` : spelled);
}

/**
 * **A Tailwind utility that ranks a word**, as the declarations it writes, its variants
 * (`md:`, `data-[open]:`, `[&>*]:`) read past: `font-semibold` is `font-weight: 600`,
 * `font-mono` a family of Tailwind's own, `leading-6` and `leading-tight` a line height,
 * `leading-none` a line height of `1`, a value of its own in brackets (`text-[13px]`,
 * `font-[600]`, `leading-[1.3]`) or a custom property in parentheses
 * (`leading-(--line-height-body)`) the value it writes, and an arbitrary property
 * (`[font-size:13px]`) what it says. A size's `/` sets the line height too (`text-sm/7`,
 * `text-[13px]/[1.3]`). `text-[…]` is a size only when it writes one (a length in any unit of
 * `LENGTH_UNITS`, `1cap` and `1rlh` too, a share, `length:`, a `--font-size-*`); a colour is
 * not this table's. A named text size (`text-sm`) is the core's,
 * held by the geometry guard.
 */
export function utilityDeclarations(word: string): { name: string; value: string }[] {
  const utility = utilityOf(word);
  const property = /^\[([a-z-]+):(.+)\]$/.exec(utility);
  if (property) {
    const name = unprefixed(property[1]);
    return name in MAY_BE ? [{ name, value: property[2].replace(/_/g, " ") }] : [];
  }
  const [, kind, rest] = /^(text|font|leading)-(.+)$/.exec(utility) ?? [];
  if (kind === undefined) return [];
  if (kind === "leading") return [{ name: "line-height", value: leading(rest) }];
  if (kind === "text") {
    const [, base, height] = /^(\[[^\]]+\]|\([^)]+\)|[\w-]+)(?:\/(.+))?$/.exec(rest) ?? [];
    if (base === undefined) return [];
    const own = /^[[(]/.test(base) ? leading(base) : undefined;
    const isSize = own !== undefined && (/^length:/.test(own) || SIZE.test(own));
    if (!isSize && !NAMED_SIZE.test(base)) return [];
    const found = isSize ? [{ name: "font-size", value: own.replace(/^length:/, "") }] : [];
    if (height !== undefined) found.push({ name: "line-height", value: leading(height) });
    return found;
  }
  const own = /^[[(]/.test(rest) ? leading(rest) : undefined;
  if (own !== undefined) {
    const token = /^var\((--[\w-]+)\)$/.exec(own)?.[1];
    if (token !== undefined && MAY_BE["font-family"].has(token)) {
      return [{ name: "font-family", value: own }];
    }
    return [{ name: "font-weight", value: own }];
  }
  if (rest in WEIGHTS) return [{ name: "font-weight", value: WEIGHTS[rest] }];
  if (/^(?:sans|serif|mono)$/.test(rest)) return [{ name: "font-family", value: rest }];
  return [];
}

/** The declarations of a utility that are off the scale. */
const offTheScale = (word: string) =>
  utilityDeclarations(word).filter(({ name, value }) => !isOnTheScale(name, value));

/** Each type literal a component writes: in an inline `style` or a `setProperty` call, as
 *  `style <key>: <value>` or `setProperty <name>: <value>`, and each Tailwind utility that ranks
 *  a word off the tokens, as `class <utility>`. A value is read where it is written out, and
 *  where the file hands one on (`const size = "13px"; … fontSize: size`, #2684); a value worked
 *  out (`${n}px`, a parameter) is not. */
export function componentLiterals(source: string, from: StyleSource = {}): string[] {
  const found: string[] = [];
  for (const { label, property, value, origin } of styleEntries(source, from)) {
    const name = unprefixed(property);
    if (!(name in MAY_BE)) continue;
    const written = /^-?[\d.]/.test(value) ? value : value.slice(1, -1);
    if (!isOnTheScale(name, written)) {
      found.push(`${label}: ${value}${origin ? ` (${origin})` : ""}`);
    }
  }
  for (const tag of openingTags(source, /^[A-Za-z]/)) {
    for (const word of classUtilities(classNameOf(tag.text))) {
      if (offTheScale(word).length > 0) found.push(`class ${word}`);
    }
  }
  return found;
}

const read = (file: string) => readFileSync(join(SRC, file), "utf8");

/** Every type literal in the window, by file: its stylesheets (the core aside) and its
 *  components, as they are or with a file's text replaced (`planted`). */
function literalsByFile(planted: Readonly<Record<string, string>> = {}): Map<string, string[]> {
  const text = (file: string) => planted[file] ?? read(file);
  const byFile = new Map<string, string[]>();
  for (const file of sourceFiles((name) => name.endsWith(".css"))) {
    if (file === THE_CORE) continue;
    const found = cssLiterals(text(file));
    byFile.set(file, FORCED.test(file) ? found.filter((one) => !SYSTEM_TEXT.test(one)) : found);
  }
  const others = windowFiles(planted);
  for (const file of sourceFiles(isScript)) {
    byFile.set(file, componentLiterals(text(file), { file, read: others }));
  }
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
/** Where a value came from, at an entry's end (`(held as size)`), which names no role. */
const ORIGIN = / \((?:held as|returned by) .*\)$/;

export function roleFor(entry: string): string {
  const utility = /^class (.+)$/.exec(entry)?.[1];
  const set = utility === undefined ? undefined : offTheScale(utility)[0];
  const [, name = "", value = ""] = set
    ? ["", set.name, set.value]
    : (/(?:\| |^style (?:attribute |[\w$]+\.)?|^style\.(?:cssText )?|^setProperty )([\w-]+): ?(.*)$/.exec(
        entry.replace(ORIGIN, "").replace(/["'`]/g, ""),
      ) ?? []);
  const key = unprefixed(STYLE_KEY[name] ?? name);
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

  it("fails a Tailwind type utility planted in a real component, and says which role to wear", () => {
    const planted = `${read("Inbox.tsx")}
export const Planted = () => (
  <p className="md:text-[13px] font-semibold leading-[1.3] font-mono text-sm leading-none" />
);
`;
    const result = complaints(literalsByFile({ "Inbox.tsx": planted }));
    expect(result.map((complaint) => complaint.split(" (")[0])).toEqual([
      "Inbox.tsx: new type literal: class md:text-[13px]",
      "Inbox.tsx: new type literal: class font-semibold",
      "Inbox.tsx: new type literal: class leading-[1.3]",
      "Inbox.tsx: new type literal: class font-mono",
    ]);
    expect(result[0]).toContain("ui-text-body");
    expect(result[1]).toContain("ui-text-strong");
    expect(result[2]).toContain("var(--line-height-body)");
    expect(result[3]).toContain("ui-text-code");
  });

  it("reads Tailwind's size, weight, leading and family utilities, and passes their tokens", () => {
    const tsx = `
      <p className="text-[0.8rem] text-[length:12px] font-[600] font-bold leading-tight leading-6" />
      <p className="text-[var(--font-size-sm)] font-[var(--font-weight-strong)] leading-none" />
      <p className={cn("leading-(--line-height-body) font-(--font-mono) text-[#fff] text-text-muted", x && "font-sans")} />
      <p className="leading-[var(--space-2)] font-[var(--font-size-sm)] leading-[1]" />
    `;
    expect(componentLiterals(tsx)).toEqual([
      "class text-[0.8rem]",
      "class text-[length:12px]",
      "class font-[600]",
      "class font-bold",
      "class leading-tight",
      "class leading-6",
      "class font-sans",
      "class leading-[var(--space-2)]",
      "class font-[var(--font-size-sm)]",
    ]);
  });

  it("reads a size's line height, an arbitrary property and every variant's spelling", () => {
    const tsx = `
      <p className="text-[13px]/7 text-sm/7 text-sm/[1.3] text-sm/none text-sm/(--line-height-body)" />
      <p className="text-text-muted/50 [font-weight:600] md:[line-height:1.3] [color:var(--text-muted)]" />
      <p className="data-[open]:font-bold [&>*]:leading-6 group-hover/x:font-[600] !font-bold" />
    `;
    expect(componentLiterals(tsx)).toEqual([
      "class text-[13px]/7",
      "class text-sm/7",
      "class text-sm/[1.3]",
      "class [font-weight:600]",
      "class md:[line-height:1.3]",
      "class data-[open]:font-bold",
      "class [&>*]:leading-6",
      "class group-hover/x:font-[600]",
      "class !font-bold",
    ]);
    expect(roleFor("class text-sm/7")).toContain("var(--line-height-body)");
  });

  it("reads a type value a component sets in each way it can, in the real tree", () => {
    const planted = `${read("Inbox.tsx")}
export const Cast = () => <p style={{ fontSize: "13px" } as CSSProperties} />;
export const Lines = () => (
  <p
    style={
      {
        fontWeight: 600,
        lineHeight: size,
      } as CSSProperties
    }
  />
);
export function plant(row: HTMLElement) {
  row.style.setProperty("font-size", "13px");
}
`;
    const result = complaints(literalsByFile({ "Inbox.tsx": planted }));
    expect(result.map((complaint) => complaint.split(" (")[0])).toEqual([
      'Inbox.tsx: new type literal: style fontSize: "13px"',
      "Inbox.tsx: new type literal: style fontWeight: 600",
      'Inbox.tsx: new type literal: setProperty font-size: "13px"',
    ]);
    expect(result[2]).toContain("ui-text-body");
  });

  it("reads a type value a script sets, outside every component", () => {
    const planted = `${read("theme/theme.ts")}
export function plant(row: HTMLElement) {
  row.style.setProperty("font-weight", "600");
}
`;
    const result = complaints(literalsByFile({ "theme/theme.ts": planted }));
    expect(result.map((complaint) => complaint.split(" (")[0])).toEqual([
      'theme/theme.ts: new type literal: setProperty font-weight: "600"',
    ]);
  });

  it("reads a type value a component holds, assigns or writes as text, in the real tree (#2498)", () => {
    const planted = `${read("Inbox.tsx")}
const held = { fontWeight: 600, color: "var(--text-muted)" };
export const Either = ({ on }: { on: boolean }) => (
  <p style={{ fontSize: on ? "13px" : "var(--font-size-sm)", lineHeight: on ? 1 : "var(--line-height-body)" }} />
);
export const Held = () => <p style={held} className="text-[1.2cap] text-[2rlh]" />;
export function plant(row: HTMLElement) {
  row.style.fontSize = "13px";
  row.style?.setProperty("line-height", "1.3");
  row.style.cssText = "font-family: Menlo; padding: 7px";
}
`;
    const result = complaints(literalsByFile({ "Inbox.tsx": planted }));
    expect(result.map((complaint) => complaint.split(" (")[0])).toEqual([
      'Inbox.tsx: new type literal: style fontSize: "13px"',
      "Inbox.tsx: new type literal: style held.fontWeight: 600",
      'Inbox.tsx: new type literal: style.fontSize: "13px"',
      'Inbox.tsx: new type literal: style.cssText font-family: "Menlo"',
      'Inbox.tsx: new type literal: setProperty line-height: "1.3"',
      "Inbox.tsx: new type literal: class text-[1.2cap]",
      "Inbox.tsx: new type literal: class text-[2rlh]",
    ]);
    expect(result[0]).toContain("ui-text-body");
    expect(result[1]).toContain("ui-text-strong");
    expect(result[2]).toContain("ui-text-body");
    expect(result[3]).toContain("ui-text-code");
    expect(result[4]).toContain("var(--line-height-body)");
  });

  it("follows a type value wherever the file hands its style on, in the real tree (#2566)", () => {
    const planted = `${read("Inbox.tsx")}
import { plantedFace } from "./theme/theme";
const plantedRoles = { title: { fontSize: "13px" } };
const plantedHeld = { fontWeight: 600 };
const plantedAssigned = { fontWeight: 500 };
const plantedLine = () => ({ lineHeight: 1.3 });
export const PlantedMember = () => <p style={plantedRoles.title} />;
export const PlantedEither = ({ on }: { on: boolean }) => <p style={on ? plantedHeld : undefined} />;
export const PlantedCall = () => <p style={plantedLine()} />;
export const PlantedImported = () => <p style={plantedFace} />;
export function plantAlias(row: HTMLElement) {
  const { style } = row;
  style.fontSize = "15px";
  row.style.cssText += "font-weight: 500";
  row.style[\`font-size\`] = "11px";
  Object.assign(row.style, plantedAssigned);
}
`;
    const script = `${read("theme/theme.ts")}\nexport const plantedFace = { fontFamily: "Menlo" };\n`;
    const result = complaints(literalsByFile({ "Inbox.tsx": planted, "theme/theme.ts": script }));
    expect(result.map((complaint) => complaint.split(" (")[0])).toEqual([
      'Inbox.tsx: new type literal: style plantedRoles.title.fontSize: "13px"',
      "Inbox.tsx: new type literal: style plantedHeld.fontWeight: 600",
      "Inbox.tsx: new type literal: style plantedLine().lineHeight: 1.3",
      'Inbox.tsx: new type literal: style plantedFace.fontFamily: "Menlo"',
      "Inbox.tsx: new type literal: style plantedAssigned.fontWeight: 500",
      'Inbox.tsx: new type literal: style.fontSize: "15px"',
      'Inbox.tsx: new type literal: style.font-size: "11px"',
      'Inbox.tsx: new type literal: style.cssText font-weight: "500"',
    ]);
  });

  it("follows a type value held on its own, in the real tree (#2684)", () => {
    const planted = `${read("Inbox.tsx")}
import { plantedFamily } from "./theme/theme";
const plantedSize = "13px";
const plantedWeights = { strong: 600 };
const plantedLine = (tight: boolean) => (tight ? 1.3 : undefined);
export const PlantedHeld = () => <p style={{ fontSize: plantedSize }} />;
export const PlantedMember = () => <p style={{ fontWeight: plantedWeights.strong }} />;
export const PlantedCall = () => <p style={{ lineHeight: plantedLine(true) }} />;
export const PlantedImported = () => <p style={{ fontFamily: plantedFamily }} />;
export function plantAssigned(row: HTMLElement) {
  row.style.fontSize = plantedSize;
}
`;
    const script = `${read("theme/theme.ts")}\nexport const plantedFamily = "Menlo";\n`;
    const result = complaints(literalsByFile({ "Inbox.tsx": planted, "theme/theme.ts": script }));
    expect(result.map((complaint) => complaint.split(" (")[0])).toEqual([
      'Inbox.tsx: new type literal: style fontSize: "13px"',
      "Inbox.tsx: new type literal: style fontWeight: 600",
      "Inbox.tsx: new type literal: style lineHeight: 1.3",
      'Inbox.tsx: new type literal: style fontFamily: "Menlo"',
      'Inbox.tsx: new type literal: style.fontSize: "13px"',
    ]);
  });

  it("reads a type property past its vendor's prefix (#2498)", () => {
    const css =
      ".a { -webkit-font-weight: 600; -moz-color: transparent; -webkit-font-smoothing: auto; }";
    expect(cssLiterals(css).map((entry) => entry.split(" | ")[1])).toEqual([
      "-webkit-font-weight: 600",
      "-moz-color: transparent",
    ]);
    expect(componentLiterals(`<p style={{ WebkitLineHeight: 1.3, msColor: "red" }} />`)).toEqual([
      "style WebkitLineHeight: 1.3",
      'style msColor: "red"',
    ]);
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
    // Fewer as families take their rules out of App.css (the cards, Notices and counts, #2912).
    expect(typed.length).toBeGreaterThan(350);
    expect(COLOURS.has("--text-muted")).toBe(true);
  });
});
