// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { ask, putBack, putBackSoon, showing, showWhileOn } from "./hoverPreview";
import { DEFAULT_THEME, drawIn, inForce, onDrawn, type Theme } from "./theme";
import { PURLIS_LIGHT } from "./handed.testkit";

/**
 * **A card's theme drawn while it is pointed at, and the person's put back** (#2331): what goes
 * back is what is in force underneath, a move between two cards never shows the person's
 * theme between them, and every follower (a terminal) ends on the theme the window shows.
 */

const LIGHT = PURLIS_LIGHT;
/** Another theme, as the window would draw one of its own while a card is shown. */
const OTHER: Theme = { ...DEFAULT_THEME, name: "another" };

afterEach(() => {
  putBack();
  vi.useRealTimers();
  drawIn(DEFAULT_THEME);
});

describe("the hover preview", () => {
  it("draws the card's theme and puts the one in force back", () => {
    drawIn(DEFAULT_THEME);
    showWhileOn(LIGHT, ask());
    expect(inForce()).toBe(LIGHT);
    expect(showing()).toBe(true);
    putBack();
    expect(inForce()).toBe(DEFAULT_THEME);
    expect(showing()).toBe(false);
  });

  it("goes from one card to the next without drawing the person's theme between", () => {
    vi.useFakeTimers();
    drawIn(DEFAULT_THEME);
    const drawn: string[] = [];
    const stop = onDrawn((theme) => drawn.push(theme.name));
    showWhileOn(LIGHT, ask());
    putBackSoon();
    showWhileOn(OTHER, ask());
    vi.advanceTimersByTime(1);
    expect(inForce()).toBe(OTHER);
    expect(drawn).toEqual([LIGHT.name, "another"]);
    putBackSoon();
    vi.advanceTimersByTime(1);
    expect(inForce()).toBe(DEFAULT_THEME);
    stop();
  });

  it("puts back a theme the window drew under the card, and keeps every follower on the card's", async () => {
    drawIn(DEFAULT_THEME);
    let followed: Theme | undefined;
    const stop = onDrawn((theme) => (followed = theme));
    showWhileOn(LIGHT, ask());
    // The window draws a theme of its own while the card is shown.
    drawIn(OTHER);
    await Promise.resolve();
    expect(inForce()).toBe(LIGHT);
    expect(followed).toBe(LIGHT);
    putBack();
    expect(inForce()).toBe(OTHER);
    expect(followed).toBe(OTHER);
    stop();
  });
});

describe("what the hover preview draws nothing for", () => {
  it("draws nothing for the theme already drawn, so no terminal redraws", () => {
    drawIn(DEFAULT_THEME);
    let told = 0;
    const stop = onDrawn(() => told++);
    expect(showWhileOn({ ...DEFAULT_THEME }, ask())).toBe(false);
    expect(told).toBe(0);
    expect(showing()).toBe(false);
    stop();
  });

  it("draws nothing for a ticket a newer ask or a put-back made stale", () => {
    drawIn(DEFAULT_THEME);
    const old = ask();
    ask();
    expect(showWhileOn(LIGHT, old)).toBe(false);
    const left = ask();
    putBack();
    expect(showWhileOn(LIGHT, left)).toBe(false);
    expect(inForce()).toBe(DEFAULT_THEME);
  });
});
