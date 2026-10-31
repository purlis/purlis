// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * **A secondary way on a notice card stands apart from the card** (#3478, #4169's review): in every
 * built-in appearance, on each card ground (news on the overlay, trouble's and attention's own),
 * the fill `notice.css` draws a card's secondary button in is farther from the ground than the
 * button family's own fill was, at least {@link SEEN} from it, and farther still under the
 * pointer; and the primary words stay readable on it.
 *
 * Distances are OKLab's, as the theme engine measures how far apart two colours look
 * (`purlis_core::theme::parts::SEEN`). The fills are mixes of two tokens, which the contrast
 * guard cannot read from a pair (`contrast.test.ts`'s `READ_BY_HAND`): this is where they are
 * read.
 */

/** Below this, a person sees one colour (`purlis_core::theme::parts::SEEN`). */
const SEEN = 0.02;

const BUILT_IN = join(process.cwd(), "..", "crates", "purlis-core", "src", "theme", "built-in");
const GROUNDS = ["surface.overlay", "danger.surface", "attention.surface"] as const;

type Oklab = [number, number, number];

const linear = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);

function rgbOf(hex: string): [number, number, number] {
  const h = hex.replace("#", "");
  return [0, 2, 4].map((at) => linear(parseInt(h.slice(at, at + 2), 16) / 255)) as [
    number,
    number,
    number,
  ];
}

function oklabOf([r, g, b]: [number, number, number]): Oklab {
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}

function rgbOfOklab([L, a, b]: Oklab): [number, number, number] {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const clamp = (v: number) => Math.min(1, Math.max(0, v));
  return [
    clamp(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
    clamp(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
    clamp(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s),
  ];
}

const apart = (x: Oklab, y: Oklab) => Math.hypot(x[0] - y[0], x[1] - y[1], x[2] - y[2]);
const mix = (x: Oklab, y: Oklab, share: number): Oklab =>
  [0, 1, 2].map((at) => x[at] * (1 - share) + y[at] * share) as Oklab;
const luminance = ([r, g, b]: [number, number, number]) => 0.2126 * r + 0.7152 * g + 0.0722 * b;
const contrast = (x: number, y: number) => (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);

/** Each appearance of each built-in theme, its tokens as colours. */
function appearances(): { name: string; tokens: Record<string, string> }[] {
  const out: { name: string; tokens: Record<string, string> }[] = [];
  for (const theme of ["purlis", "purlis-classic"]) {
    const file = JSON.parse(readFileSync(join(BUILT_IN, theme, "theme.json"), "utf8")) as {
      palette?: Record<string, string>;
      appearances: { id: string; tokens: Record<string, unknown> }[];
    };
    const palette = file.palette ?? {};
    for (const one of file.appearances) {
      const tokens: Record<string, string> = {};
      for (const [token, value] of Object.entries(one.tokens)) {
        // A token left to the platform, or written another way, names no colour here.
        if (typeof value !== "string") continue;
        let at = value;
        for (let step = 0; step < 5 && !at.startsWith("#"); step += 1)
          at = palette[at] ?? tokens[at] ?? at;
        tokens[token] = at;
      }
      out.push({ name: `${theme} ${one.id}`, tokens });
    }
  }
  return out;
}

/** The share of the words each rule of the card's secondary way mixes in, read off the sheet. */
function shares(): { rest: number; pointed: number } {
  const css = readFileSync(join(process.cwd(), "src", "ui", "notice", "notice.css"), "utf8");
  const found = [
    ...css.matchAll(/color-mix\(in oklab, var\(--control-base\), var\(--text-primary\) (\d+)%\)/g),
  ].map((one) => Number(one[1]) / 100);
  expect(found, "the rest and the pointed fill").toHaveLength(2);
  return { rest: found[0], pointed: found[1] };
}

describe("a secondary way on a notice card (#3478)", () => {
  const { rest, pointed } = shares();
  for (const { name, tokens } of appearances()) {
    const base = oklabOf(rgbOf(tokens["control.base"]));
    const words = oklabOf(rgbOf(tokens["text.primary"]));
    const atRest = mix(base, words, rest);
    const underThePointer = mix(base, words, pointed);
    for (const ground of GROUNDS) {
      it(`${name}: stands apart from ${ground}, and farther under the pointer`, () => {
        const on = oklabOf(rgbOf(tokens[ground]));
        const before = apart(base, on);
        expect(apart(atRest, on)).toBeGreaterThan(before);
        expect(apart(atRest, on)).toBeGreaterThanOrEqual(SEEN);
        expect(apart(underThePointer, on)).toBeGreaterThan(apart(atRest, on));
      });
    }
    it(`${name}: keeps its words readable at rest and under the pointer`, () => {
      const read = luminance(rgbOf(tokens["text.primary"]));
      for (const fill of [atRest, underThePointer])
        expect(contrast(read, luminance(rgbOfOklab(fill)))).toBeGreaterThanOrEqual(4.5);
    });
  }
});
