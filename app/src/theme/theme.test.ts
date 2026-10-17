// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  BUILT_IN,
  DEFAULT_THEME,
  TOKENS,
  apply,
  cssVariables,
  inForce,
  property,
  drawIn,
  sameTheme,
  searchDecorations,
  xtermTheme,
  type Theme,
} from "./theme";

/**
 * **The window only applies a theme.** Reading, validating and resolving one is the core's theme
 * engine's (`purlis_core::theme`, whose tests hold every rule a theme file is held to); what is
 * left here is how a resolved theme reaches the document, the terminal and the find bar.
 *
 * The built-ins' own files are read from the core as the independent source of what each token
 * is, so a value checked here was not computed by the code under test.
 */
const CORE = join(process.cwd(), "..", "crates", "purlis-core", "src", "theme", "built-in");
const file = (name: string) =>
  JSON.parse(readFileSync(join(CORE, `${name}.json`), "utf8")) as {
    tokens: Record<string, string>;
  };
const dark = file("purlis-dark");
const light = file("purlis-light");

describe("the themes the core hands the window", () => {
  it("are the core's built-ins, complete, the window coming up in the dark one", () => {
    expect(Object.keys(BUILT_IN).sort()).toEqual(["purlis-dark", "purlis-light"]);
    expect(DEFAULT_THEME).toBe(BUILT_IN["purlis-dark"]);
    for (const name of Object.keys(BUILT_IN))
      for (const token of TOKENS) expect(BUILT_IN[name].values[token], token).toBeTruthy();
    expect(BUILT_IN["purlis-light"].values["surface.base"]).toBe(light.tokens["surface.base"]);
  });

  it("are told apart by value, not by identity, so an answer again draws nothing again", () => {
    const again: Theme = { ...DEFAULT_THEME, values: { ...DEFAULT_THEME.values } };
    expect(sameTheme(again, DEFAULT_THEME)).toBe(true);
    expect(sameTheme(BUILT_IN["purlis-light"], DEFAULT_THEME)).toBe(false);
    again.values["surface.base"] = light.tokens["surface.base"];
    expect(sameTheme(again, DEFAULT_THEME)).toBe(false);
  });
});

describe("one theme, two consumers", () => {
  it("writes every token as a custom property", () => {
    const written = cssVariables(BUILT_IN["purlis-dark"]);
    expect(Object.keys(written)).toHaveLength(TOKENS.length);
    expect(written["--surface-base"]).toBe(dark.tokens["surface.base"]);
    expect(written["--terminal-ansi-bright-white"]).toBe(dark.tokens["terminal.ansi.bright-white"]);
  });

  it("turns a token name into a property name", () => {
    expect(property("surface.base")).toBe("--surface-base");
    expect(property("terminal.ansi.bright-black")).toBe("--terminal-ansi-bright-black");
  });

  it("builds xterm's object with xterm's own key names", () => {
    const object = xtermTheme(BUILT_IN["purlis-dark"]);
    expect(object.background).toBe(dark.tokens["terminal.background"]);
    expect(object.foreground).toBe(dark.tokens["terminal.foreground"]);
    expect(object.cursorAccent).toBe(dark.tokens["terminal.cursor-accent"]);
    expect(object.selectionBackground).toBe(dark.tokens["terminal.selection"]);
    expect(object.brightMagenta).toBe(dark.tokens["terminal.ansi.bright-magenta"]);
    // Sixteen ANSI colours plus four, and no key xterm does not know.
    expect(Object.keys(object)).toHaveLength(21);
  });

  it("colours the find bar's matches in the pane from the theme (SI-4)", () => {
    const drawn = searchDecorations(BUILT_IN["purlis-dark"]);
    expect(drawn.matchBackground).toBe(dark.tokens["terminal.find-match"]);
    expect(drawn.matchOverviewRuler).toBe(dark.tokens["terminal.find-match"]);
    expect(drawn.activeMatchBackground).toBe(dark.tokens["terminal.find-match-active"]);
    expect(drawn.activeMatchColorOverviewRuler).toBe(dark.tokens["terminal.find-match-active"]);
  });

  it("hands the search addon a short or translucent match colour as the #rrggbb it takes", () => {
    // The addon's own contract: a match background "must use #RRGGBB format". A theme may
    // write any of the four forms, so the long form is made here, once.
    const theme: Theme = {
      ...DEFAULT_THEME,
      values: {
        ...DEFAULT_THEME.values,
        "terminal.find-match": "#abc",
        "terminal.find-match-active": "#11223380",
      },
    };
    const drawn = searchDecorations(theme);
    expect(drawn.matchBackground).toBe("#aabbcc");
    expect(drawn.activeMatchBackground).toBe("#112233");
  });

  it("gives the terminal and the window the same theme, which is the whole point", () => {
    // The defect this layer removes: `SessionPane` used to say `#181818`, `App.css` used to
    // say `--paper: #181818`, and nothing on earth made the two agree.
    const theme = BUILT_IN["purlis-light"];
    expect(xtermTheme(theme).background).toBe(cssVariables(theme)["--terminal-background"]);
    expect(xtermTheme(theme).foreground).toBe(cssVariables(theme)["--terminal-foreground"]);
  });
});

describe("putting a theme on the document", () => {
  it("sets every custom property, the colour scheme and the name", () => {
    const element = document.createElement("div");
    apply(BUILT_IN["purlis-light"], element);
    expect(element.style.getPropertyValue("--surface-base")).toBe(light.tokens["surface.base"]);
    expect(element.style.getPropertyValue("--state-failed")).toBe(light.tokens["state.failed"]);
    // `color-scheme` is what makes the platform's own scrollbars and form controls match, and
    // no custom property can do it.
    expect(element.style.colorScheme).toBe("light");
    expect(element.dataset.theme).toBe("purlis-light");
  });

  it("leaves nothing of the theme before it behind", () => {
    const element = document.createElement("div");
    apply(BUILT_IN["purlis-light"], element);
    apply(BUILT_IN["purlis-dark"], element);
    for (const token of TOKENS) {
      expect(element.style.getPropertyValue(property(token)), token).toBe(
        BUILT_IN["purlis-dark"].values[token],
      );
    }
    expect(element.style.colorScheme).toBe("dark");
  });

  it("makes the theme in force the one a pane opens its terminal in", () => {
    // `SessionPane` reads `inForce()` when it constructs a terminal, so this is the hand-off
    // between the document's colours and the grid's.
    const element = document.createElement("div");
    try {
      drawIn(BUILT_IN["purlis-light"], element);
      expect(inForce()).toBe(BUILT_IN["purlis-light"]);
      expect(xtermTheme(inForce()).background).toBe(light.tokens["terminal.background"]);
    } finally {
      drawIn(DEFAULT_THEME, element);
    }
  });
});
