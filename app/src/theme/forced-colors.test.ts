// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { forcedFamiliesIn, forcedSheet } from "../ui/stylesheets";
import { FAMILY_THEME_BY_NAME, FAMILY_THEMES, type ForcedTheme } from "./families.testkit";

/**
 * **Forced colours** (#2328): `forced-colors.css` says again, in the system's palette and with a
 * shape, what the window says with a background or a shadow alone. jsdom evaluates no media
 * query and no cascade, so this reads the sheet the way `surfaces.test.ts` reads `App.css`: each
 * rule as its selector and its declarations. `tailwind.test.ts` holds that the build keeps it in
 * the last layer, and `e2e/specs/forced-colors.e2e.ts` that the WebView parses it. Each family's
 * own sheet (`ui/<family>/forced-colors.css`, #3387) is read with it, in the order the window
 * imports them: every family's, by name, and then the window's. What each family's sheet draws is
 * said in its own folder (`forced`, in `ui/<family>/theme.testkit.ts`), and held here beside the
 * window's own.
 */

const read = (file: string) =>
  readFileSync(join(process.cwd(), "src", file), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

/** The forced-colours sheets, in the order `styles.css` imports them into the `forced` layer. */
const SHEETS = [
  ...forcedFamiliesIn(join(process.cwd(), "src", "ui"))
    .sort()
    .map((family) => forcedSheet(family).slice(2)),
  "theme/forced-colors.css",
];

/** One sheet's one `@media` block's body. */
function insideOf(file: string): string {
  const sheet = read(file);
  const open = /@media\s*\(forced-colors:\s*active\)\s*\{/.exec(sheet);
  expect(open, `${file}: one @media (forced-colors: active) block`).not.toBeNull();
  const start = (open?.index ?? 0) + (open?.[0].length ?? 0);
  const end = sheet.lastIndexOf("}");
  // Nothing outside it: forced colours are all the sheet draws.
  expect(sheet.slice(0, open?.index).trim(), file).toBe("");
  expect(sheet.slice(end + 1).trim(), file).toBe("");
  return sheet.slice(start, end);
}

/** Every sheet's block, one after another, as the cascade reads them. */
function inside(): string {
  return SHEETS.map(insideOf).join("\n");
}

/** Each rule as its selectors (one per comma, whitespace collapsed) and its declarations. */
function rules(): { selectors: string[]; declarations: Map<string, string> }[] {
  return [...inside().matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((hit) => {
    const selectors = splitSelectors(hit[1]);
    const declarations = new Map(
      hit[2]
        .split(";")
        .map((one) => one.trim())
        .filter(Boolean)
        .map((one) => {
          const colon = one.indexOf(":");
          return [
            one.slice(0, colon).trim(),
            one
              .slice(colon + 1)
              .replace(/\s+/g, " ")
              .trim(),
          ];
        }),
    );
    return { selectors, declarations };
  });
}

/** A selector list split at its top-level commas: `:is(a, b)` stays whole. */
function splitSelectors(list: string): string[] {
  const out: string[] = [];
  let depth = 0;
  let current = "";
  for (const char of list) {
    if (char === "(") depth += 1;
    if (char === ")") depth -= 1;
    if (char === "," && depth === 0) {
      out.push(current);
      current = "";
    } else current += char;
  }
  out.push(current);
  return out.map((one) => one.replace(/\s+/g, " ").trim()).filter(Boolean);
}

/** The declarations every rule naming `selector` writes, merged in order. */
function drawn(selector: string): Map<string, string> {
  const merged = new Map<string, string>();
  for (const rule of rules())
    if (rule.selectors.includes(selector))
      for (const [property, value] of rule.declarations) merged.set(property, value);
  return merged;
}

/** The CSS system colours (CSS Color 4 §6.2) that forced colours draw in. */
const SYSTEM = new Set([
  "AccentColor",
  "AccentColorText",
  "ActiveText",
  "ButtonBorder",
  "ButtonFace",
  "ButtonText",
  "Canvas",
  "CanvasText",
  "Field",
  "FieldText",
  "GrayText",
  "Highlight",
  "HighlightText",
  "LinkText",
  "Mark",
  "MarkText",
  "SelectedItem",
  "SelectedItemText",
  "VisitedText",
]);

describe("forced colours", () => {
  it("draw in the system's colours and nothing of a theme's", () => {
    expect(inside()).not.toMatch(/var\(/);
    expect(inside()).not.toMatch(/#[0-9a-f]{3,8}\b/i);
    const system = new Set([...SYSTEM].map((one) => one.toLowerCase()));
    // Every word of a colour's value that is not a length or a style is a colour, and each
    // one is a system colour: a named colour, a hex or a function fails, whatever its case.
    const NOT_COLOURS = /^(?:none|solid|dashed|inset|-?[\d.]+(?:px|em|rem)?)$/;
    const colours = rules().flatMap(({ declarations }) =>
      [...declarations]
        .filter(([property]) => /color|background|shadow|^outline$|border-color/.test(property))
        .filter(([property]) => property !== "forced-color-adjust")
        .flatMap(([, value]) => value.split(/[\s,]+/).filter((word) => !NOT_COLOURS.test(word))),
    );
    expect(colours.length).toBeGreaterThan(0);
    expect(colours.filter((colour) => !system.has(colour.toLowerCase()))).toEqual([]);
  });

  it("show where Tab lands, everywhere the window's own focus floor does", () => {
    const focus = rules().find((rule) =>
      rule.selectors.some((one) => one.endsWith(":focus-visible")),
    );
    expect(focus?.declarations.get("outline-style")).toBe("solid");
    expect(focus?.declarations.get("outline-color")).toBe("Highlight");
    // Forced colours replace colour, not geometry: the ring keeps the theme's width and offset,
    // which the window's own focus rule gives it (#2867).
    expect(focus?.declarations.has("outline-width")).toBe(false);
    expect(focus?.declarations.has("outline-offset")).toBe(false);
    expect(focus?.declarations.get("outline")).toBeUndefined();
    const reached = focus?.selectors.join(" ") ?? "";
    for (const stop of [
      "button",
      "summary",
      "a",
      '[role="tab"]',
      '[role="separator"]',
      "[tabindex]",
    ])
      expect(reached, stop).toContain(stop);
  });

  it("draw the focus ring over every outline, last in the block (a later equal rule wins)", () => {
    const all = rules();
    const focus = all.findIndex((rule) =>
      rule.selectors.some((one) => one.endsWith(":focus-visible")),
    );
    const outlined = all
      .map((rule, at) => ({ rule, at }))
      .filter(({ rule }) =>
        [...rule.declarations.keys()].some((name) => name.startsWith("outline")),
      )
      .filter(({ at }) => at !== focus)
      .map(({ at }) => at);
    expect(outlined.length).toBeGreaterThan(0);
    expect(outlined.filter((at) => at > focus)).toEqual([]);
  });

  /** Every family's `forced` entries of one kind (`ui/<family>/theme.testkit.ts`). */
  const families = (kind: Exclude<keyof ForcedTheme, "drawn">): string[] =>
    FAMILY_THEMES.flatMap((family) => family.forced?.[kind] ?? []);

  it("hear from every family with a sheet of its own what it draws", () => {
    const stated = new Set(
      Object.entries(FAMILY_THEME_BY_NAME)
        .filter(([, family]) => family.forced !== undefined)
        .map(([name]) => name),
    );
    for (const family of forcedFamiliesIn(join(process.cwd(), "src", "ui")))
      expect(stated.has(family), `ui/${family}/theme.testkit.ts says its forced colours`).toBe(
        true,
      );
  });

  it("say what is selected in the system's pair, and with a shape as well as a colour", () => {
    for (const selected of [
      '[role="tab"][aria-selected="true"]',
      '[role="option"][aria-selected="true"]',
      '[aria-current]:not([aria-current="false"])',
      ...families("selected"),
    ]) {
      const it = drawn(selected);
      expect(it.get("forced-color-adjust"), selected).toBe("none");
      expect(it.get("background-color"), selected).toBe("SelectedItem");
      expect(it.get("color"), selected).toBe("SelectedItemText");
    }
    for (const shaped of ['[role="tab"][aria-selected="true"]', ...families("underlined")])
      expect(drawn(shaped).get("text-decoration-line"), shaped).toBe("underline");
  });

  it("draw what each family says it draws, declaration by declaration", () => {
    const said = FAMILY_THEMES.flatMap((family) => Object.entries(family.forced?.drawn ?? {}));
    expect(said.length).toBeGreaterThan(0);
    for (const [selector, declarations] of said)
      for (const [property, value] of Object.entries(declarations))
        expect(drawn(selector).get(property), `${selector} { ${property} }`).toBe(value);
  });

  it("keep each state mark's shape, which tells the states apart without colour", () => {
    for (const mark of [
      ".state-running",
      ".state-waiting",
      ".state-failed",
      ".explorer .touch-mark",
    ]) {
      expect(drawn(mark).get("forced-color-adjust"), mark).toBe("none");
      expect(drawn(mark).get("background-color"), mark).toBe("CanvasText");
    }
    // Waiting is told from running by its ring.
    expect(drawn(".state-waiting").get("box-shadow")).toMatch(/CanvasText/);
  });

  it("say what cannot be used", () => {
    expect(drawn('[aria-disabled="true"]').get("color")).toBe("GrayText");
  });

  it("mark what needs you in the system's pair for what is marked", () => {
    for (const needs of [".invisible-char", ...families("marked")]) {
      const it = drawn(needs);
      expect(it.get("forced-color-adjust"), needs).toBe("none");
      expect(it.get("background-color"), needs).toBe("Mark");
      expect(it.get("color"), needs).toBe("MarkText");
    }
  });

  it("tell what ends or destroys, and a ground that says something is wrong, by a shape", () => {
    for (const danger of [".trouble", ".ui-setting-error p", ...families("dangerous")]) {
      expect(drawn(danger).get("outline"), danger).toBe("2px dashed CanvasText");
      expect(drawn(danger).get("outline-offset"), danger).toBe("-2px");
    }
  });

  it("tell a refused box from a taken one by its edge's shape, which takes no room (#3335)", () => {
    const refused = families("refused");
    expect(refused.length).toBeGreaterThan(0);
    for (const box of refused) {
      const it = drawn(box);
      expect(it.get("border-style"), box).toBe("dashed");
      expect(it.get("border-color"), box).toBe("CanvasText");
      // Its own edge, at the width it has: nothing moves, and the focus ring (an outline) stays.
      for (const property of it.keys())
        expect(property, property).not.toMatch(/width|outline|padding|margin|^border$/);
    }
    // Every box the parts manifest says can be refused is reached by some family's refused
    // selector: the manifest's families with the `invalid` state, read from the table the engine
    // writes from it (`crates/purlis-core/docs/themes.md`, which a core test keeps in step).
    const table = readFileSync(
      join(process.cwd(), "..", "crates", "purlis-core", "docs", "themes.md"),
      "utf8",
    );
    const refusable = [...table.matchAll(/^\| `([\w-]+)`[^\n]*\|([^|\n]*)\|\s*$/gm)]
      .filter(([, , states]) => /`invalid`/.test(states))
      .map(([, family]) => family);
    expect(refusable.length).toBeGreaterThan(0);
    for (const family of refusable)
      expect(refused.join(" "), family).toContain(`[data-scope="${family}"]`);
  });

  it("outline what is opened over the window, taking no room", () => {
    for (const over of [
      '[role="dialog"]',
      '[role="menu"]',
      '[role="listbox"]',
      '[role="tooltip"]',
      ...families("opened"),
    ])
      expect(drawn(over).get("outline"), over).toBe("1px solid CanvasText");
  });

  it("is the last layer of the window's one stylesheet", () => {
    const styles = read("styles.css");
    const order = (/@layer ([\w\s,-]+);/.exec(styles)?.[1] ?? "").split(/\s*,\s*/);
    expect(order.at(-1)).toBe("forced");
    // In two halves: the stylesheet guard reads a quoted path ending in the extension as an
    // import (`literals.test.ts`).
    expect(styles).toContain(['@import "./theme/forced-colors', 'css" layer(forced);'].join("."));
    // Every family's own sheet just before it, in the place of the `@forced` line.
    expect(styles.indexOf("/* @forced:")).toBeLessThan(styles.indexOf("./theme/forced-colors"));
  });
});
