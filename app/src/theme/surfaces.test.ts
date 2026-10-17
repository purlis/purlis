// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  BUILT_IN,
  CLEAR_IS_UNSET,
  SYNTAX_ROLES,
  TINTED_WINDOW,
  property,
  xtermTheme,
  type Token,
} from "./theme";

/**
 * **Each token #2327 adds reaches the surface it says it paints.** `literals.test.ts` proves
 * every token is read somewhere; this says where: the editor's highlighting reads each syntax
 * role, a markdown render each markdown token, the window's selection, caret, placeholder and
 * scrollbar rules the platform tokens, a chart's bar the first chart colour, the window its
 * background, and xterm the terminal's extras.
 */

const read = (file: string) =>
  readFileSync(join(process.cwd(), "src", file), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

/** Each rule of `sheet` as `[selector, body]`. */
function rules(sheet: string): [string, string][] {
  return [...sheet.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map(([, selector, body]) => [
    selector.trim().replace(/\s+/g, " "),
    body,
  ]);
}

/** The declarations `sheet` makes of `token` as `selector | property`. */
function readers(sheet: string, token: Token): string[] {
  const name = property(token);
  return rules(sheet).flatMap(([selector, body]) =>
    [...body.matchAll(/([\w-]+)\s*:\s*([^;]+)/g)]
      .filter(([, , value]) => value.includes(`var(${name})`))
      .map(([, declared]) => `${selector} | ${declared}`),
  );
}

describe("the editor's highlighting", () => {
  const look = read("editor/look.ts");

  it("reads every syntax role, its colour, italic and bold, and no terminal colour", () => {
    for (const role of SYNTAX_ROLES) {
      const name = property(role);
      expect(look, role).toContain(`color: "var(${name})"`);
      expect(look, role).toContain(`fontStyle: "var(${name}-font-style)"`);
      expect(look, role).toContain(`fontWeight: "var(${name}-font-weight)"`);
    }
    // The editor stopped borrowing the terminal's ANSI colours for code.
    expect(look).not.toMatch(/--terminal-ansi-/);
  });
});

describe("the window's stylesheet", () => {
  const app = read("App.css");

  it("draws a markdown render's link, code, code block, quote and heading", () => {
    expect(readers(app, "markdown.link")).toEqual([".release-notes a | color"]);
    expect(readers(app, "markdown.code")).toEqual([".release-notes code | color"]);
    expect(readers(app, "markdown.code-block")).toEqual([
      ".release-notes pre, .release-notes pre code | color",
    ]);
    expect(readers(app, "markdown.quote")).toEqual([".release-notes blockquote | color"]);
    expect(readers(app, "markdown.heading")).toEqual([
      ".release-notes :is(h1, h2, h3, h4, h5, h6) | color",
    ]);
  });

  it("draws the selection, caret, placeholder and scrollbars only when the theme gives them", () => {
    expect(readers(app, "selection.background")).toEqual([
      ':root[data-theme-draws~="selection.background"] ::selection | background-color',
    ]);
    expect(readers(app, "selection.text")).toEqual([
      ':root[data-theme-draws~="selection.text"] ::selection | color',
    ]);
    expect(readers(app, "caret")).toEqual([
      ':root[data-theme-draws~="caret"] :is(input, textarea, [contenteditable]) | caret-color',
    ]);
    expect(readers(app, "placeholder")).toEqual([
      ':root[data-theme-draws~="placeholder"] ::placeholder | color',
    ]);
    const scrollbar = [':root[data-theme-draws~="scrollbar.thumb"] | scrollbar-color'];
    expect(readers(app, "scrollbar.thumb")).toEqual(scrollbar);
    expect(readers(app, "scrollbar.track")).toEqual(scrollbar);
    // Every gate names a token the window may leave to the platform.
    for (const [selector] of rules(app))
      for (const [, gate] of selector.matchAll(/data-theme-draws~="([^"]+)"/g))
        expect(CLEAR_IS_UNSET as readonly string[]).toContain(gate);
  });

  it("draws a chart's series in the first chart colour, which turns with the accent", () => {
    expect(readers(app, "chart.1")).toEqual([".chart-bar | background"]);
    expect(readers(app, "accent.base")).not.toContain(".chart-bar | background");
    expect(TINTED_WINDOW).toEqual(expect.arrayContaining(["chart.1", "markdown.link"]));
  });

  it("paints the window in the window's background", () => {
    expect(readers(app, "window.background")).toEqual([":root | background"]);
  });
});

describe("the terminal", () => {
  it("is handed the inactive selection and the scrollbar's three colours", () => {
    const object = xtermTheme(BUILT_IN["purlis-dark"]);
    const values = BUILT_IN["purlis-dark"].values;
    expect(object.selectionInactiveBackground).toBe(values["terminal.selection-inactive"]);
    expect(object.scrollbarSliderBackground).toBe(values["terminal.scrollbar"]);
    expect(object.scrollbarSliderHoverBackground).toBe(values["terminal.scrollbar-hover"]);
    expect(object.scrollbarSliderActiveBackground).toBe(values["terminal.scrollbar-active"]);
  });

  it("is given the theme's cursor, line height and bold where a pane opens it and on a change", () => {
    const pane = read("SessionPane.tsx");
    expect(pane).toMatch(/\.\.\.xtermLook\(inForce\(\)\)/);
    expect(pane).toMatch(/Object\.assign\(pane\.options, look\)/);
  });
});
