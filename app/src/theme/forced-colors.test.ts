// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * **Forced colours** (#2328): `forced-colors.css` says again, in the system's palette and with a
 * shape, what the window says with a background or a shadow alone. jsdom evaluates no media
 * query and no cascade, so this reads the sheet the way `surfaces.test.ts` reads `App.css`: each
 * rule as its selector and its declarations. `tailwind.test.ts` holds that the build keeps it in
 * the last layer, and `e2e/specs/forced-colors.e2e.ts` that the WebView parses it.
 */

const read = (file: string) =>
  readFileSync(join(process.cwd(), "src", file), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

const sheet = read("theme/forced-colors.css");

/** The sheet's one `@media` block's body. */
function inside(): string {
  const open = /@media\s*\(forced-colors:\s*active\)\s*\{/.exec(sheet);
  expect(open, "one @media (forced-colors: active) block").not.toBeNull();
  const start = (open?.index ?? 0) + (open?.[0].length ?? 0);
  const end = sheet.lastIndexOf("}");
  // Nothing outside it: forced colours are all the sheet draws.
  expect(sheet.slice(0, open?.index).trim()).toBe("");
  expect(sheet.slice(end + 1).trim()).toBe("");
  return sheet.slice(start, end);
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

  it("say what is selected in the system's pair, and with a shape as well as a colour", () => {
    for (const selected of [
      '[role="tab"][aria-selected="true"]',
      '[role="option"][aria-selected="true"]',
      '[aria-current]:not([aria-current="false"])',
      '[data-scope="segment"][data-part="root"][data-selected]',
      ".ui-menu-item[data-highlighted]",
      '.ui-menu-item[data-state="open"]',
      '[data-scope="menu"][data-part="root"][data-focused]:not([data-disabled])',
      '[data-scope="menu"][data-part="root"][data-hovered]:not([data-disabled])',
      '[data-scope="menu"][data-part="root"][data-open]',
      '[data-scope="list"][data-part="root"][data-selected]',
      '[data-scope="tree"][data-part="root"][data-selected]',
    ]) {
      const it = drawn(selected);
      expect(it.get("forced-color-adjust"), selected).toBe("none");
      expect(it.get("background-color"), selected).toBe("SelectedItem");
      expect(it.get("color"), selected).toBe("SelectedItemText");
    }
    for (const shaped of [
      '[role="tab"][aria-selected="true"]',
      '[data-scope="segment"][data-part="root"][data-selected]',
    ])
      expect(drawn(shaped).get("text-decoration-line"), shaped).toBe("underline");
    expect(
      drawn(
        '[data-scope="activity"][data-part="item"][data-selected] > [data-part="indicator"]',
      ).get("background-color"),
    ).toBe("SelectedItemText");
    // A list's and a tree's chosen row keeps its edge (#2911).
    for (const row of ["list", "tree"])
      expect(
        drawn(`[data-scope="${row}"][data-part="root"][data-selected]`).get("box-shadow"),
      ).toBe("inset 2px 0 0 SelectedItemText");
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

  it("keep a switch's thumb and a radio's dot, and say what cannot be used", () => {
    expect(drawn('[data-scope="switch"][data-part="thumb"]').get("background-color")).toBe(
      "CanvasText",
    );
    expect(
      drawn(
        '[data-scope="switch"][data-part="root"][data-selected] [data-scope="switch"][data-part="thumb"]',
      ).get("background-color"),
    ).toBe("HighlightText");
    expect(drawn('[data-scope="radio"][data-part="mark"]::after').get("background-color")).toBe(
      "CanvasText",
    );
    expect(drawn('[aria-disabled="true"]').get("color")).toBe("GrayText");
  });

  it("mark what needs you in the system's pair for what is marked", () => {
    for (const needs of [
      '[data-scope="activity"][data-part="count"][data-tone="needs-you"]',
      '[data-scope="count"][data-part="root"][data-variant="hot"]',
      '[data-scope="status-item"][data-variant="waiting"] > [data-part="count"]',
      ".repo-instruction-text .invisible-char",
    ]) {
      const it = drawn(needs);
      expect(it.get("forced-color-adjust"), needs).toBe("none");
      expect(it.get("background-color"), needs).toBe("Mark");
      expect(it.get("color"), needs).toBe("MarkText");
    }
  });

  it("tell what ends or destroys, and a ground that says something is wrong, by a shape", () => {
    for (const danger of [
      '[data-scope="button"][data-part="root"][data-variant="danger"]',
      '[data-scope="menu"][data-part="root"][data-variant="danger"]',
      '[data-scope="status-item"][data-part="root"][data-tone="danger"]',
      ".trouble",
      '[data-scope="notice"][data-part="root"][data-variant="trouble"]',
      ".ui-setting-error p",
    ]) {
      expect(drawn(danger).get("outline"), danger).toBe("2px dashed CanvasText");
      expect(drawn(danger).get("outline-offset"), danger).toBe("-2px");
    }
  });

  it("tell a refused box from a taken one by its edge's shape, which takes no room (#3335)", () => {
    const refused =
      ':is( [data-scope="text-input"], [data-scope="textarea"], [data-scope="select"], [data-scope="combobox"] )[data-part="root"][data-invalid]';
    const it = drawn(refused);
    expect(it.get("border-style")).toBe("dashed");
    expect(it.get("border-color")).toBe("CanvasText");
    // Its own edge, at the width it has: nothing moves, and the focus ring (an outline) stays.
    for (const property of it.keys())
      expect(property, property).not.toMatch(/width|outline|padding|margin|^border$/);
    // Every box a value is given to is reached.
    for (const family of ["text-input", "textarea", "select", "combobox"])
      expect(refused).toContain(`[data-scope="${family}"]`);
  });

  it("outline what is opened over the window, taking no room", () => {
    for (const over of [
      '[role="dialog"]',
      '[role="menu"]',
      '[role="listbox"]',
      '[role="tooltip"]',
      '[data-scope="popover"][data-part="root"]',
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
  });
});
