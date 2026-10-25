// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **The control primitives** (`primitives/controls.css`, ADR 0094 §4, #1834): one class per kind of
 * control, drawn from the tokens alone, over Radix's behaviour.
 *
 * Each control is held to what a person sees and what a contributor can break, never to class
 * plumbing:
 * - **its height and its focus ring come from the tokens**, read through the window's cascade
 *   (`cascade.testkit.ts`, which knows `primitives/controls.css` is the layer above `App.css`), so a view
 *   rule that tries to redraw a classed control does not count;
 * - **the keyboard works it** as the primitive under it promises: Tab reaches it, and Enter,
 *   Space or an arrow does what a click would;
 * - **it is announced by name**, with its role and its state.
 *
 * The stylesheet itself is read as text too (jsdom applies none): it sits in the `components`
 * layer, reads only tokens, and writes no length the tokens do not give it.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";

import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { drawnWith } from "../cascade.testkit";
import { isGeometry } from "../theme/geometry";
import { MOTION_TOKENS, motionProperty } from "../theme/motion";
import { property, TOKENS } from "../theme/theme";

afterEach(cleanup);

beforeAll(() => {
  // jsdom lays nothing out: a list scrolls its chosen row into view and asks for pointer
  // capture, neither of which jsdom has. A real WebView has both.
  Element.prototype.scrollIntoView ??= () => {};
  Element.prototype.hasPointerCapture ??= () => false;
  Element.prototype.releasePointerCapture ??= () => {};
});

const SRC = join(process.cwd(), "src");
const sheet = (file: string) =>
  readFileSync(join(SRC, file), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const CSS = sheet("primitives/controls.css");

/** Every rule of the primitives, as `[selector, body]`. */
const RULES: [string, string][] = [...CSS.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map(
  ([, selector, body]) => [selector.trim().replace(/\s+/g, " "), body],
);

/** The ring a control draws outside itself, and the one a segment draws inside. */
const RING = "var(--focus-ring-width) solid var(--focus-ring)";

describe("the primitives stylesheet", () => {
  it("is in the components layer, above App.css", () => {
    const styles = sheet("styles.css");
    // In two halves: the stylesheet guard reads a quoted path ending in the extension as an
    // import (`literals.test.ts`).
    expect(styles).toContain(
      ['@import "./primitives/controls', 'css" layer(components);'].join("."),
    );
    const order = (/@layer ([\w\s,-]+);/.exec(styles)?.[1] ?? "").split(/\s*,\s*/);
    expect(order.indexOf("components")).toBeGreaterThan(order.indexOf("charter"));
  });

  it("reads only tokens: a theme's colours and motion, the core geometry, and its own", () => {
    const colours = new Set(TOKENS.map(property));
    const motion = new Set(MOTION_TOKENS.map(motionProperty));
    // Its own lengths, and the ones a view may set for it: none, now that a menu's row is its family's (`ui/menu/menu.css`).
    const own = new Set([...CSS.matchAll(/^\s+(--[\w-]+):/gm)].map((hit) => hit[1]));
    const fromViews = new Set<string>();
    for (const name of fromViews) expect(sheet("App.css")).toContain(`${name}:`);
    const read = [...CSS.matchAll(/var\((--[\w-]+)/g)].map((hit) => hit[1]);
    expect(read.length).toBeGreaterThan(0);
    expect(
      read.filter(
        (name) =>
          !colours.has(name) &&
          !motion.has(name) &&
          !isGeometry(name) &&
          !own.has(name) &&
          !fromViews.has(name),
      ),
    ).toEqual([]);
  });

  it("writes no length of its own: every size is a token, a hairline or nothing", () => {
    // `1px` is a border and a rule's hairline; `0px` is nothing, as a fallback. Everything else
    // is a token or a token's share.
    const lengths = RULES.flatMap(([selector, body]) =>
      [...body.matchAll(/(?<![\w-])(\d*\.?\d+)(px|rem|em|vh|vw|ch)\b/g)]
        .filter(([whole]) => whole !== "1px" && whole !== "0px")
        .map(([whole]) => `${selector} | ${whole}`),
    );
    expect(lengths).toEqual([]);
  });

  it("gives every colour as one token, so the contrast check reads every pair", () => {
    const loose = RULES.flatMap(([selector, body]) =>
      [...body.matchAll(/(?<![\w-])(color|background-color|background)\s*:\s*([^;]+)/g)]
        .map(([, , value]) => value.trim())
        .filter((value) => !/^var\(--[\w-]+\)$/.test(value) && value !== "transparent")
        .map((value) => `${selector} | ${value}`),
    );
    expect(loose).toEqual([]);
  });
});

  });
});
