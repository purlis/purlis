// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { toward } from "../src/theme/tint";
import { PICTURE_COMPARISON } from "./pictures";

/**
 * **The picture comparison catches a change of a flat fill** (#3178): the comparator the
 * screenshot gate runs, with the options it is given (`pictures.ts`), on two flat swatches, each
 * a colour as the screen draws it on its ground.
 */

type Diff = (
  a: Uint8Array,
  b: Uint8Array,
  output: undefined,
  width: number,
  height: number,
  options: { threshold: number },
) => number;

/** The very comparator Vitest's `pixelmatch` runs: the copy `@vitest/browser` resolves. */
const diff: Diff = (() => {
  const own = createRequire(import.meta.url);
  const browser = createRequire(own.resolve("@vitest/browser"));
  return (browser("@blazediff/core") as { diff: Diff }).diff;
})();

type Drawn = { values: Record<string, string> };
// A file named through a variable: Vite rewrites a literal `new URL(…, import.meta.url)` to an asset.
const json = (file: string): unknown =>
  JSON.parse(readFileSync(fileURLToPath(new URL(file, import.meta.url)), "utf8"));
const THEMES = json("built-in-themes.json") as Record<string, Record<string, Drawn>>;
const PURLIS_LIGHT = THEMES.purlis.light.values;

const SIDE = 16;

const byte = (hex: string, at: number) => Number.parseInt(hex.slice(1 + at * 2, 3 + at * 2), 16);

/** `colour` laid on the opaque `ground`, as the screen draws it: red, green, blue. */
function laid(colour: string, ground: string): [number, number, number] {
  const alpha = colour.length === 9 ? byte(colour, 3) / 255 : 1;
  const one = (at: number) => Math.round(byte(colour, at) * alpha + byte(ground, at) * (1 - alpha));
  return [one(0), one(1), one(2)];
}

/** A square of one colour, laid on `ground`. */
function swatch(colour: string, ground: string): Uint8Array {
  const [red, green, blue] = laid(colour, ground);
  const pixels = new Uint8Array(SIDE * SIDE * 4);
  for (let at = 0; at < pixels.length; at += 4) pixels.set([red, green, blue, 255], at);
  return pixels;
}

/** How many pixels of a swatch differ, compared with `threshold`. */
function differing(
  before: string,
  after: string,
  ground: string,
  threshold: number = PICTURE_COMPARISON.comparatorOptions.threshold,
): number {
  return diff(swatch(before, ground), swatch(after, ground), undefined, SIDE, SIDE, {
    threshold,
  });
}

/** Whether a whole swatch's change fails the gate: past the pixels it allows. */
function fails(before: string, after: string, ground: string, threshold?: number): boolean {
  return (
    differing(before, after, ground, threshold) >
    PICTURE_COMPARISON.comparatorOptions.allowedMismatchedPixels
  );
}

/** What the comparator lets through by default: what the gate ran at before #3178. */
const DEFAULT_THRESHOLD = 0.1;

describe("the picture comparison (#3178)", () => {
  // #3169: Purlis light's `wash.warning` at .12, from a grey-brown to amber, on the window's
  // grounds. About 0.061 on pixelmatch's scale.
  const was = "#5236011f";
  const now = PURLIS_LIGHT["wash.warning"];

  it("fails a picture whose wash changed hue at .12, on every surface step it is drawn on", () => {
    expect(now).toBe("#fdb0051f");
    for (const ground of ["surface.base", "surface.raised", "surface.overlay", "surface.deep"]) {
      expect(fails(was, now, PURLIS_LIGHT[ground]), ground).toBe(true);
      // The control: the comparator's own default let it through.
      expect(fails(was, now, PURLIS_LIGHT[ground], DEFAULT_THRESHOLD), ground).toBe(false);
    }
  });

  it("fails a picture whose accent changed between hues of one luminance", () => {
    // #5263: an accent given over Purlis light went from green to blue. The primary's fill and
    // the selected row's edge are the accent itself; the update pill's ground is
    // `accent.surface` turned toward it (`tint.ts`' `toward`), at the luminance it had.
    const green = PURLIS_LIGHT["terminal.ansi.green"];
    const blue = PURLIS_LIGHT["terminal.ansi.blue"];
    const ground = PURLIS_LIGHT["surface.base"];
    expect(fails(green, blue, ground)).toBe(true);
    const surface = PURLIS_LIGHT["accent.surface"];
    expect(fails(toward(surface, green), toward(surface, blue), ground)).toBe(true);
    // The control: the comparator's own default let the accent through.
    expect(fails(green, blue, ground, DEFAULT_THRESHOLD)).toBe(false);
  });

  it("fails a surface of Purlis light's drawn as its neighbour", () => {
    // Each step of Purlis light's grounds onto the next: the raised strip drawn as the window.
    const steps = ["surface.deep", "surface.base", "surface.raised", "surface.overlay"];
    for (const [at, token] of steps.slice(1).entries()) {
      const before = PURLIS_LIGHT[token];
      const after = PURLIS_LIGHT[steps[at]];
      expect(fails(before, after, before), `${token} drawn as ${steps[at]}`).toBe(true);
      // The control: the comparator's own default let it through.
      expect(fails(before, after, before, DEFAULT_THRESHOLD), token).toBe(false);
    }
  });

  it("passes a fill a rounding apart: two levels in each channel", () => {
    for (const [before, after] of [
      ["#f7f7fd", "#f9f9ff"],
      ["#17171b", "#15151a"],
      ["#7507ff", "#7709fd"],
    ])
      expect(fails(before, after, "#ffffff"), `${before} ${after}`).toBe(false);
  });

  it("passes a picture that did not change", () => {
    for (const token of ["wash.warning", "accent.wash", "surface.overlay"])
      expect(differing(PURLIS_LIGHT[token], PURLIS_LIGHT[token], "#ffffff")).toBe(0);
  });
});
