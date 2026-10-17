// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import {
  DURATIONS,
  EASINGS,
  MOTION_TOKENS,
  motionProperty,
  motionVariables,
  REDUCE,
} from "./motion";
import { apply, BUILT_IN, DEFAULT_THEME } from "./theme";

/** The milliseconds a written duration says, read as a number so no test here spells out a
 *  CSS time — `literals.test.ts` holds this directory's tests to nothing, but the habit is the
 *  point. */
const ms = (written: string) => Number.parseFloat(written);

describe("motion is a vocabulary, like colour", () => {
  it("gives every built-in theme every duration and every easing", () => {
    for (const theme of Object.values(BUILT_IN)) {
      // What a duration or an easing may be is the core's theme engine's to hold
      // (`purlis_core::theme`); what it hands the window has every one.
      for (const token of DURATIONS)
        expect(Number.isInteger(theme.motion.durations[token]), token).toBe(true);
      for (const token of EASINGS) expect(theme.motion.easings[token], token).toHaveLength(4);
    }
  });

  it("names a token for what the motion is for, not for how long it is", () => {
    // `duration.enter`, never `duration.200`: a theme speeding arrivals up must not also
    // speed up whatever happened to share a rung of a scale with them.
    for (const token of MOTION_TOKENS) expect(token).not.toMatch(/\d/);
  });

  it("writes each token to its own property, apart from the colours", () => {
    expect(motionProperty("duration.enter")).toBe("--motion-duration-enter");
    expect(motionProperty("easing.steady")).toBe("--motion-easing-steady");
  });
});

describe("reduced motion is the layer's, and handled once", () => {
  it("writes the theme's durations and easings as custom properties", () => {
    const written = motionVariables(DEFAULT_THEME.motion, false);
    expect(Object.keys(written).sort()).toEqual(MOTION_TOKENS.map(motionProperty).sort());
    for (const token of DURATIONS) {
      expect(ms(written[motionProperty(token)])).toBe(DEFAULT_THEME.motion.durations[token]);
    }
    expect(written["--motion-easing-enter"]).toBe(
      `cubic-bezier(${DEFAULT_THEME.motion.easings["easing.enter"].join(", ")})`,
    );
  });

  it("collapses every duration when the operator asked for less motion", () => {
    // Which is what a component written tomorrow gets for free: it reads a token, and the token
    // is zero for anyone who asked.
    const written = motionVariables(DEFAULT_THEME.motion, true);
    for (const token of DURATIONS) expect(ms(written[motionProperty(token)])).toBe(0);
    // The easings stay, so the set of properties on the document is the same either way.
    expect(written["--motion-easing-enter"]).toMatch(/^cubic-bezier\(/);
  });

  it("puts the motion on the document with the colours", () => {
    const element = document.createElement("div");
    apply(DEFAULT_THEME, element, false);
    expect(ms(element.style.getPropertyValue("--motion-duration-enter"))).toBe(
      DEFAULT_THEME.motion.durations["duration.enter"],
    );
    apply(DEFAULT_THEME, element, true);
    expect(ms(element.style.getPropertyValue("--motion-duration-enter"))).toBe(0);
  });
});

describe("the window follows the setting while it is up", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.resetModules();
  });

  it("reads the setting when the theme is drawn, and redraws when it changes", async () => {
    // A fresh copy of the module, because the listener is installed once per window.
    vi.resetModules();
    let reduced = true;
    const listeners: ((event: { matches: boolean }) => void)[] = [];
    const matchMedia = vi.fn((query: string) => ({
      get matches() {
        return query === REDUCE && reduced;
      },
      addEventListener: (_: string, listener: (event: { matches: boolean }) => void) =>
        listeners.push(listener),
    }));
    vi.stubGlobal("matchMedia", matchMedia);
    const { drawIn, DEFAULT_THEME: fresh } = await import("./theme");

    const element = document.createElement("div");
    drawIn(fresh, element);
    expect(ms(element.style.getPropertyValue("--motion-duration-spin"))).toBe(0);
    expect(matchMedia).toHaveBeenCalledWith(REDUCE);

    // Drawn again — an extension's theme arriving after the first frame — and still one
    // listener, not one per draw.
    drawIn(fresh, element);
    expect(listeners).toHaveLength(1);

    reduced = false;
    for (const listener of listeners) listener({ matches: false });
    expect(ms(element.style.getPropertyValue("--motion-duration-spin"))).toBe(
      fresh.motion.durations["duration.spin"],
    );
  });
});
