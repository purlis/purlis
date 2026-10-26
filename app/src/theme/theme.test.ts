// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { classicAppearances } from "./handed.testkit";
import {
  BUILT_IN,
  CLASSIC as CLASSIC_ID,
  CLASSIC_THEME,
  DEFAULT_THEME,
  PARTS_STYLE,
  TOKENS,
  apply,
  cssVariables,
  inForce,
  property,
  drawIn,
  sameTheme,
  searchDecorations,
  syntaxFaces,
  themeDraws,
  xtermLook,
  xtermTheme,
  type Theme,
} from "./theme";

/**
 * **The window only applies a theme.** Reading, validating and resolving one is the core's theme
 * engine's (`purlis_core::theme`, whose tests hold every rule a theme file is held to); what is
 * left here is how a resolved theme reaches the document, the terminal and the find bar.
 *
 * Purlis Classic's own folder is read from the core as the independent source of what each
 * token is, so a value checked here was not computed by the code under test.
 */
const CLASSIC = join(
  process.cwd(),
  "..",
  "crates",
  "purlis-core",
  "src",
  "theme",
  "built-in",
  "purlis-classic",
  "theme.json",
);
const [dark, light] = (
  JSON.parse(readFileSync(CLASSIC, "utf8")) as {
    appearances: { tokens: Record<string, string> }[];
  }
).appearances;

/** Purlis Classic's appearances, as the core resolves them (`handed.testkit.ts`): what the
 *  tests of how a theme is drawn are written against. */
const [CLASSIC_DARK, CLASSIC_LIGHT] = classicAppearances().map((one) => one.theme as Theme);

describe("the themes the core hands the window", () => {
  it("name Purlis Classic by the engine's own id, which draws its first appearance", () => {
    const engine = readFileSync(
      join(process.cwd(), "..", "crates", "purlis-core", "src", "theme.rs"),
      "utf8",
    );
    expect(engine).toContain(`pub const CLASSIC: &str = "${CLASSIC_ID}";`);
    expect(CLASSIC_THEME).toBe(BUILT_IN[CLASSIC_ID]);
    expect(CLASSIC_THEME).toEqual(CLASSIC_DARK);
  });

  it("are the core's built-ins, complete, the window coming up in the dark one", () => {
    expect(Object.keys(BUILT_IN).sort()).toEqual(["purlis-classic", "purlis-dark"]);
    // The window comes up in the default, Purlis's dark, as the core hands it (#3034).
    expect([DEFAULT_THEME.name, DEFAULT_THEME.appearance]).toEqual(["Purlis", "dark"]);
    for (const token of TOKENS) expect(DEFAULT_THEME.values[token], token).toBeTruthy();
    for (const name of Object.keys(BUILT_IN))
      for (const token of TOKENS) expect(BUILT_IN[name].values[token], token).toBeTruthy();
    expect(CLASSIC_LIGHT.values["surface.base"]).toBe(light.tokens["surface.base"]);
  });

  it("are told apart by value, not by identity, so an answer again draws nothing again", () => {
    const again: Theme = { ...DEFAULT_THEME, values: { ...DEFAULT_THEME.values } };
    expect(sameTheme(again, DEFAULT_THEME)).toBe(true);
    expect(sameTheme(CLASSIC_LIGHT, DEFAULT_THEME)).toBe(false);
    again.values["surface.base"] = light.tokens["surface.base"];
    expect(sameTheme(again, DEFAULT_THEME)).toBe(false);
  });
});

describe("one theme, two consumers", () => {
  it("writes every token as a custom property", () => {
    const written = cssVariables(CLASSIC_DARK);
    expect(Object.keys(written)).toHaveLength(TOKENS.length);
    expect(written["--surface-base"]).toBe(dark.tokens["surface.base"]);
    expect(written["--terminal-ansi-bright-white"]).toBe(dark.tokens["terminal.ansi.bright-white"]);
  });

  it("turns a token name into a property name", () => {
    expect(property("surface.base")).toBe("--surface-base");
    expect(property("terminal.ansi.bright-black")).toBe("--terminal-ansi-bright-black");
  });

  it("builds xterm's object with xterm's own key names", () => {
    const object = xtermTheme(CLASSIC_DARK);
    expect(object.background).toBe(dark.tokens["terminal.background"]);
    expect(object.foreground).toBe(dark.tokens["terminal.foreground"]);
    expect(object.cursorAccent).toBe(dark.tokens["terminal.cursor-accent"]);
    expect(object.selectionBackground).toBe(dark.tokens["terminal.selection"]);
    expect(object.brightMagenta).toBe(dark.tokens["terminal.ansi.bright-magenta"]);
    expect(object.selectionInactiveBackground).toBe(dark.tokens["terminal.selection-inactive"]);
    expect(object.scrollbarSliderBackground).toBe(dark.tokens["terminal.scrollbar"]);
    expect(object.scrollbarSliderHoverBackground).toBe(dark.tokens["terminal.scrollbar-hover"]);
    expect(object.scrollbarSliderActiveBackground).toBe(dark.tokens["terminal.scrollbar-active"]);
    // Sixteen ANSI colours plus eight, and no key xterm does not know. Classic's selected text
    // is clear, so xterm is not told one and each cell keeps its colour, as before (#2327).
    expect(Object.keys(object)).toHaveLength(25);
    expect(object).not.toHaveProperty("selectionForeground");
  });

  it("tells xterm a selected text colour only when the theme gives one (#2327)", () => {
    const theme: Theme = {
      ...DEFAULT_THEME,
      values: { ...DEFAULT_THEME.values, "terminal.selection-foreground": "#101010" },
    };
    expect(xtermTheme(theme).selectionForeground).toBe("#101010");
  });

  it("hands xterm Purlis Classic's cursor, line height and bold as xterm's own defaults (#2327)", () => {
    for (const theme of [CLASSIC_DARK, CLASSIC_LIGHT])
      expect(xtermLook(theme)).toEqual({
        cursorStyle: "block",
        cursorBlink: false,
        lineHeight: 1,
        drawBoldTextInBrightColors: true,
      });
    const theme: Theme = {
      ...DEFAULT_THEME,
      typed: {
        ...DEFAULT_THEME.typed,
        flags: {
          ...DEFAULT_THEME.typed.flags,
          "terminal.cursor-blink": true,
          "terminal.bold-as-bright": false,
        },
        choices: { "terminal.cursor-style": "bar" },
        numbers: { "terminal.line-height": 1.4 },
      },
    };
    expect(xtermLook(theme)).toEqual({
      cursorStyle: "bar",
      cursorBlink: true,
      lineHeight: 1.4,
      drawBoldTextInBrightColors: false,
    });
  });

  it("colours the find bar's matches in the pane from the theme (SI-4)", () => {
    const drawn = searchDecorations(CLASSIC_DARK);
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
    const theme = CLASSIC_LIGHT;
    expect(xtermTheme(theme).background).toBe(cssVariables(theme)["--terminal-background"]);
    expect(xtermTheme(theme).foreground).toBe(cssVariables(theme)["--terminal-foreground"]);
  });
});

describe("the tokens that are not the chrome (#2327)", () => {
  it("draw each syntax role italic and bold as the theme says", () => {
    const faces = syntaxFaces(CLASSIC_DARK);
    // The editor drew comments italic, and tags and headings bold, before a theme could say.
    expect(faces["--syntax-comment-font-style"]).toBe("italic");
    expect(faces["--syntax-comment-font-weight"]).toBe("normal");
    expect(faces["--syntax-tag-font-weight"]).toBe("bold");
    expect(faces["--syntax-heading-font-weight"]).toBe("bold");
    expect(faces["--syntax-keyword-font-style"]).toBe("normal");
    expect(Object.keys(faces)).toHaveLength(30);
  });

  it("leave Purlis Classic's selection, caret, placeholder and scrollbars to the platform", () => {
    expect(themeDraws(CLASSIC_DARK)).toBe("");
    expect(themeDraws(CLASSIC_LIGHT)).toBe("");
    const classic = CLASSIC_DARK;
    const theme: Theme = {
      ...classic,
      values: { ...classic.values, caret: "#ff0000", "scrollbar.thumb": "#80808080" },
    };
    expect(themeDraws(theme)).toBe("caret scrollbar.thumb");
  });
});

describe("putting a theme on the document", () => {
  it("sets every custom property, the colour scheme and the name", () => {
    const element = document.createElement("div");
    apply(CLASSIC_LIGHT, element);
    expect(element.style.getPropertyValue("--surface-base")).toBe(light.tokens["surface.base"]);
    expect(element.style.getPropertyValue("--state-failed")).toBe(light.tokens["state.failed"]);
    // `color-scheme` is what makes the platform's own scrollbars and form controls match, and
    // no custom property can do it.
    expect(element.style.colorScheme).toBe("light");
    expect(element.dataset.theme).toBe("Purlis Classic");
    // The syntax roles' faces, and what the window draws instead of the platform (#2327).
    expect(element.style.getPropertyValue("--syntax-comment-font-style")).toBe("italic");
    expect(element.dataset.themeDraws).toBe("");
  });

  it("draws a theme's component parts from the engine's stylesheet, and takes them away after", () => {
    // What the engine writes for a theme that rounds the button's root to a pill (#2903,
    // `purlis_core::theme::parts_tests`).
    const pill =
      '@layer parts {\n  :where([data-scope="button"][data-part="root"][data-size="sm"]) { --purlis-part-height: max(24px, var(--size-chrome), var(--space-7)); }\n' +
      '  :where([data-scope="button"][data-part="root"]) { border-radius: 9999px; }\n}\n';
    const shaped: Theme = { ...CLASSIC_DARK, parts: pill };
    const element = document.createElement("div");
    try {
      apply(shaped, element);
      const drawn = document.getElementById(PARTS_STYLE);
      expect(drawn?.tagName).toBe("STYLE");
      expect(drawn?.parentElement).toBe(document.head);
      expect(drawn?.textContent).toBe(pill);
      // Drawn again, it is still one stylesheet.
      apply(shaped, element);
      expect(document.querySelectorAll(`#${PARTS_STYLE}`)).toHaveLength(1);
    } finally {
      // A theme that names no parts draws every control as its family default. (Classic names
      // its tabs' strip look, #2914, and its activity items', #2917, so it is taken without
      // them here.)
      apply({ ...CLASSIC_DARK, parts: "" }, element);
    }
    expect(document.getElementById(PARTS_STYLE)).toBeNull();
  });

  it("leaves nothing of the theme before it behind", () => {
    const element = document.createElement("div");
    apply(CLASSIC_LIGHT, element);
    apply(CLASSIC_DARK, element);
    for (const token of TOKENS) {
      expect(element.style.getPropertyValue(property(token)), token).toBe(
        CLASSIC_DARK.values[token],
      );
    }
    expect(element.style.colorScheme).toBe("dark");
  });

  it("makes the theme in force the one a pane opens its terminal in", () => {
    // `SessionPane` reads `inForce()` when it constructs a terminal, so this is the hand-off
    // between the document's colours and the grid's.
    const element = document.createElement("div");
    try {
      drawIn(CLASSIC_LIGHT, element);
      expect(inForce()).toBe(CLASSIC_LIGHT);
      expect(xtermTheme(inForce()).background).toBe(light.tokens["terminal.background"]);
    } finally {
      drawIn(DEFAULT_THEME, element);
    }
  });
});
