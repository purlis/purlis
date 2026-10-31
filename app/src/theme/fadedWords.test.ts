// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Faded words, as drawn** (#3790): the arithmetic the stories' guard measures with
 * (`fadedWords.ts`, `.storybook/faded.setup.ts`). The guard itself runs in Chromium against every
 * story; these hold its sums to worked examples, each figured by hand from WCAG 2.2's relative
 * luminance and the "over" compositing a browser does.
 */
import { describe, expect, it } from "vitest";
import { asDrawn, floorOf, type Rgba } from "./fadedWords";

const BLACK: Rgba = { r: 0, g: 0, b: 0, a: 1 };
const WHITE: Rgba = { r: 255, g: 255, b: 255, a: 1 };
const CLEAR: Rgba = { r: 0, g: 0, b: 0, a: 0 };

describe("words as drawn through the opacities above them", () => {
  it("are solid words' own contrast when nothing fades them", () => {
    const drawn = asDrawn(BLACK, [{ background: WHITE, opacity: 1 }]);
    expect(drawn.ratio).toBeCloseTo(21, 2);
    expect(drawn.opacity).toBe(1);
  });

  it("lay faded words over the ground behind the faded box", () => {
    // Black at 0.6 over white is #666666: (1.05) / (0.1329 + 0.05) = 5.74.
    const drawn = asDrawn(BLACK, [
      { background: CLEAR, opacity: 0.6 },
      { background: WHITE, opacity: 1 },
    ]);
    expect(drawn.ratio).toBeCloseTo(5.74, 2);
    expect(drawn.opacity).toBeCloseTo(0.6, 5);
  });

  it("multiply every opacity up the ancestors", () => {
    // 0.75 × 0.8 = 0.6: the same grey as one box at 0.6.
    const drawn = asDrawn(BLACK, [
      { background: CLEAR, opacity: 0.75 },
      { background: CLEAR, opacity: 0.8 },
      { background: WHITE, opacity: 1 },
    ]);
    expect(drawn.ratio).toBeCloseTo(5.74, 2);
    expect(drawn.opacity).toBeCloseTo(0.6, 5);
  });

  it("fade a faded box's own ground with its words", () => {
    // Black words on a white box at 0.5 over black: the words stay black and the box is
    // mid-grey (0.5, linear 0.2140), so 0.2640 / 0.05 = 5.28.
    const drawn = asDrawn(BLACK, [
      { background: WHITE, opacity: 0.5 },
      { background: BLACK, opacity: 1 },
    ]);
    expect(drawn.ratio).toBeCloseTo(5.28, 2);
  });

  it("are drawn over a white page when no box below them has a ground", () => {
    const drawn = asDrawn(BLACK, [{ background: CLEAR, opacity: 0.6 }]);
    expect(drawn.ratio).toBeCloseTo(5.74, 2);
  });
});

describe("the floor faded words are held to", () => {
  it("is 4.5:1 for body text", () => {
    expect(floorOf(16, 400)).toBe(4.5);
    expect(floorOf(18, 700)).toBe(4.5);
  });

  it("is 3:1 for large text: 24px, or 18.66px (14pt) bold", () => {
    expect(floorOf(24, 400)).toBe(3);
    expect(floorOf(19, 700)).toBe(3);
  });

  it("is 4.5:1 for 18.66px text that is not bold", () => {
    expect(floorOf(19, 400)).toBe(4.5);
  });
});
