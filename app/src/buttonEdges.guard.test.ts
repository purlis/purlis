// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { declarations, SRC, sourceFiles } from "./guards.testkit";

/**
 * **No view's stylesheet draws a button's edge** (#3129, spec #2897).
 *
 * A button is the button family's (`ui/button`), whose edge is `border.control`: none in Purlis,
 * kept only where the engine keeps one for consent or danger. A view's rule that drew an edge
 * around a button of its own, or around every button under it (`.bar button`, `.welcome
 * button`), kept a border the family no longer draws, and reached whatever button came to stand
 * there. `purlis/button-family` refuses the hand-drawn `<button>` in the markup; this holds the
 * stylesheets, which no lint reads.
 *
 * A rule draws a button's edge when it sets a border that shows (not `0`, `none` or
 * `transparent`) on a selector whose subject is a `button` or a `[role="button"]`, or when it
 * also says the pointer presses it (`cursor: pointer`). The families' own sheets (`src/ui`) and
 * the window's base and control classes (`base.css`, `primitives/`), which the choice and
 * select families move, are theirs, not a view's.
 */

/** The rules that still draw a button's edge, each with its reason; exact, so a rule that is
 *  gone fails until its line is deleted. */
const EXCEPTIONS: Record<string, string> = {
  "App.css | .welcome :is(.welcome-door, .repo-card)":
    "the welcome's door and repo cards: cards pressed whole, the card family's (`eslint.config.js`), not buttons of the button family",
};

/** The stylesheets a view writes: every one under `src` but the families', the base, the control
 *  classes and the theme's geometry. */
const VIEW_SHEETS = sourceFiles((name) => name.endsWith(".css")).filter(
  (file) => !/^(?:ui|primitives|theme)\//.test(file) && file !== "base.css",
);

const EDGE =
  /^border(?:-(?:top|right|bottom|left|block|inline)(?:-(?:start|end))?)?(?:-(?:color|width|style))?$/;
const NO_EDGE = /^(?:0(?:px)?|none|transparent)(?:\s|$)/;
const BUTTON_SUBJECT = /(?:^|[\s>+~(,])button(?![\w-])[^\s>+~]*$|\[role=["']?button["']?\]/;

/** Each rule in `css` that draws a button's edge, as `<file> | <selector list>`. */
export function buttonEdges(file: string, css: string): string[] {
  const rules = new Map<string, { edge: boolean; pointer: boolean; button: boolean }>();
  for (const { context, property, value } of declarations(css)) {
    const selector = context.filter((part) => !part.startsWith("@")).join(" › ");
    if (selector === "") continue;
    const rule = rules.get(selector) ?? {
      edge: false,
      pointer: false,
      button: selector.split(",").some((one) => BUTTON_SUBJECT.test(one.trim())),
    };
    if (EDGE.test(property) && !NO_EDGE.test(value)) rule.edge = true;
    if (property === "cursor" && value === "pointer") rule.pointer = true;
    rules.set(selector, rule);
  }
  return [...rules]
    .filter(([, rule]) => rule.edge && (rule.button || rule.pointer))
    .map(([selector]) => `${file} | ${selector}`);
}

const sheet = (file: string) => readFileSync(join(SRC, file), "utf8");

describe("a button's edge in a view's stylesheet (#3129)", () => {
  it("is drawn by no rule but a named exception", () => {
    const found = VIEW_SHEETS.flatMap((file) => buttonEdges(file, sheet(file)));
    expect(found.filter((one) => !(one in EXCEPTIONS))).toEqual([]);
    expect(Object.keys(EXCEPTIONS).filter((one) => !found.includes(one))).toEqual([]);
  });

  it("reads the stylesheets a view writes, and not the families'", () => {
    expect(VIEW_SHEETS).toContain("App.css");
    expect(VIEW_SHEETS).toContain("paneFrame.css");
    expect(VIEW_SHEETS.some((file) => file.startsWith("ui/"))).toBe(false);
  });

  it("finds the patterns the hand-drawn buttons used, planted", () => {
    const planted = `
      .bar button { color: var(--text-primary); border: 1px solid var(--border-subtle); }
      .welcome button:hover { border-color: var(--border-strong); }
      .pane-brief { border: 1px solid var(--border-subtle); cursor: pointer; }
      @media (min-width: 40rem) { .task-end { border-inline-start: 1px solid red; cursor: pointer; } }
      .menu [role="button"] { border-width: 1px; }
      .gauge { border: 1px solid var(--border-subtle); }
      .bar button.bare { border: 0; cursor: pointer; }
      .pane-harness { border-color: transparent; cursor: pointer; }
      .buttons-row { border-top: 1px solid var(--border-subtle); }
    `;
    expect(buttonEdges("A.css", planted)).toEqual([
      "A.css | .bar button",
      "A.css | .welcome button:hover",
      "A.css | .pane-brief",
      "A.css | .task-end",
      'A.css | .menu [role="button"]',
    ]);
  });
});
