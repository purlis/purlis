// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  BUILT_IN,
  DEFAULT_THEME,
  drawIn,
  drawTint,
  hueIn,
  property,
  tabTint,
  TOKENS,
  tinted,
  tintVariables,
  WORKSPACE_COLOURS,
  type Theme,
  type Token,
} from "./theme";
import { apartChroma, hueOf, tintHex } from "./tint";

/**
 * **A workspace's colour** (charter-app#281): a hue that tints the tokens the theme's recipe
 * names, and nothing else. The core's theme engine resolves the recipe (#2329) and hands it with
 * the theme, and holds every hue on any theme to the same floors as the theme itself
 * (`purlis_core::theme::check`), with its own copy of this arithmetic; this file holds that the
 * window draws the recipe it is handed, and that the two copies of the arithmetic agree.
 */

/** WCAG relative luminance of `#rrggbb`. */
function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((at) => {
    const v = Number.parseInt(hex.slice(at, at + 2), 16) / 255;
    return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** The smallest angle between two hues, in degrees. */
function apart(a: number, b: number): number {
  const d = Math.abs(a - b) % 360;
  return Math.min(d, 360 - d);
}

/** `theme` with its recipe changed. */
function withTint(theme: Theme, tint: Partial<Theme["tint"]>): Theme {
  return { ...theme, tint: { ...theme.tint, ...tint } };
}

describe("a workspace's colour", () => {
  it("reads a name as the theme's hue, a #rrggbb as the hue it has, and anything else as none", () => {
    expect(hueIn(DEFAULT_THEME, "teal")).toBe(190);
    const purple = hueIn(DEFAULT_THEME, "purple") ?? Number.NaN;
    expect(hueOf(tintHex("#6a9fb5", purple))).toBeCloseTo(purple, 0);
    for (const none of [null, undefined, "", "mauve", "#fff", "#12345", "#808080"])
      expect(hueIn(DEFAULT_THEME, none), String(none)).toBeUndefined();
  });
});

describe("the core's copy of the tint", () => {
  it("turns every case of the shared fixture to the same colour", () => {
    // `crates/purlis-core/src/theme/tint-cases.json`: inputs and answers both suites read, so
    // the tint the engine checks is the tint the window draws.
    const cases = JSON.parse(
      readFileSync(
        join(process.cwd(), "..", "crates", "purlis-core", "src", "theme", "tint-cases.json"),
        "utf8",
      ),
    ) as {
      hues: { colour: string; hue: number | null }[];
      tints: { hex: string; colour: string; tinted: string }[];
      apart: {
        accent: string;
        danger: string;
        hue: number;
        most: number | null | "none";
        tinted: string | null;
      }[];
    };
    expect(cases.tints.length).toBeGreaterThan(100);
    for (const { colour, hue } of cases.hues) {
      if (hue === null) expect(hueIn(DEFAULT_THEME, colour), colour).toBeUndefined();
      else expect(hueIn(DEFAULT_THEME, colour), colour).toBeCloseTo(hue, 9);
    }
    for (const { hex, colour, tinted: want } of cases.tints)
      expect(tintHex(hex, hueIn(DEFAULT_THEME, colour) ?? Number.NaN), `${hex} in ${colour}`).toBe(
        want,
      );
    // The accent held apart from danger: the chroma held to, and the accent drawn.
    expect(cases.apart.length).toBeGreaterThanOrEqual(40);
    // The path where no chroma keeps them apart, and the tint is dropped, is held too.
    expect(cases.apart.some((one) => one.most === "none")).toBe(true);
    const { distance } = DEFAULT_THEME.tint.apart;
    for (const { accent, danger, hue, most, tinted: want } of cases.apart) {
      const got = apartChroma(accent, danger, hue, distance);
      const expected =
        most === null ? Number.POSITIVE_INFINITY : most === "none" ? undefined : most;
      expect(got, `${accent} at ${hue}`).toBe(expected);
      expect(got === undefined ? null : tintHex(accent, hue, got), `${accent} at ${hue}`).toBe(
        want,
      );
    }
  });
});

describe("a tint", () => {
  it.each(Object.keys(BUILT_IN))(
    "turns only %s's accent and tab shades, and never its text or its terminal",
    (name) => {
      const theme = BUILT_IN[name];
      for (const colour of WORKSPACE_COLOURS) {
        const tint = tinted(theme, colour);
        const moved = TOKENS.filter((token) => tint.values[token] !== theme.values[token]);
        expect(
          moved.filter((token) => !theme.tint.tabs.includes(token)),
          colour,
        ).toEqual([]);
        expect(tint.name).toBe(theme.name);
        expect(tint.appearance).toBe(theme.appearance);
      }
    },
  );

  it.each(Object.keys(BUILT_IN))("gives %s's accent the colour's hue", (name) => {
    const theme = BUILT_IN[name];
    for (const { name: colour, hue } of theme.tint.hues) {
      const accent = tinted(theme, colour).values["accent.base"];
      // Within a few degrees: the hex is rounded to eight bits a channel.
      // Within a few more where the tint gave less colour to keep the accent apart from danger.
      expect(apart(hueOf(accent) ?? Number.NaN, hue), `${colour}: ${accent}`).toBeLessThan(6);
    }
  });

  it.each(Object.keys(BUILT_IN))("keeps the luminance of every shade it turns in %s", (name) => {
    const theme = BUILT_IN[name];
    for (const colour of WORKSPACE_COLOURS) {
      const tint = tinted(theme, colour);
      for (const token of theme.tint.tabs as readonly Token[]) {
        const was = luminance(theme.values[token]);
        const now = luminance(tint.values[token]);
        expect(Math.abs(now - was) / Math.max(was, 0.01), `${colour} ${token}`).toBeLessThan(0.03);
      }
    }
  });

  it("colours a grey shade, which has no hue of its own", () => {
    const shade = DEFAULT_THEME.values["layer.chat"];
    expect(tinted(DEFAULT_THEME, "green").values["layer.chat"]).not.toBe(shade);
    expect(hueOf(tinted(DEFAULT_THEME, "green").values["layer.chat"])).toBeDefined();
  });

  it("is nothing without a colour, or with one purlis does not read", () => {
    expect(tinted(DEFAULT_THEME, null)).toBe(DEFAULT_THEME);
    expect(tinted(DEFAULT_THEME, "mauve")).toBe(DEFAULT_THEME);
    expect(tabTint(DEFAULT_THEME, null)).toEqual({});
  });

  it("takes a custom colour's hue", () => {
    const custom = "#c06020";
    const hue = hueOf(custom) ?? Number.NaN;
    const accent = tinted(DEFAULT_THEME, custom).values["accent.base"];
    expect(apart(hueOf(accent) ?? Number.NaN, hue)).toBeLessThan(4);
    expect(accent).not.toBe(DEFAULT_THEME.values["accent.base"]);
  });

  it("is the same theme object each time it is asked, so a redraw repaints nothing", () => {
    expect(tinted(DEFAULT_THEME, "blue")).toBe(tinted(DEFAULT_THEME, "blue"));
  });

  it("is handed to an element as the custom properties of the tokens asked for", () => {
    const tint = tinted(DEFAULT_THEME, "pink");
    expect(tintVariables(DEFAULT_THEME, "pink", ["layer.chat", "accent.base"])).toEqual({
      [property("layer.chat")]: tint.values["layer.chat"],
      [property("accent.base")]: tint.values["accent.base"],
    });
  });
});

describe("the window's tint", () => {
  afterEach(() => {
    drawTint(null);
    drawIn(DEFAULT_THEME);
  });

  it("puts the workspace in front's accent and focus ring on the window, and no tab shade", () => {
    const root = document.documentElement;
    drawIn(DEFAULT_THEME);
    drawTint("yellow");
    const tint = tinted(DEFAULT_THEME, "yellow");
    for (const token of DEFAULT_THEME.tint.window as readonly Token[])
      expect(root.style.getPropertyValue(property(token)), token).toBe(tint.values[token]);
    // The tab shades are each tab's own, set on the tab: the project strip is not a workspace's.
    for (const token of ["layer.project", "layer.workspace", "layer.chat"] as const)
      expect(root.style.getPropertyValue(property(token)), token).toBe(DEFAULT_THEME.values[token]);
  });

  it("follows the theme drawn under it, and goes when the workspace has no colour", () => {
    const root = document.documentElement;
    drawTint("red");
    drawIn(BUILT_IN["purlis-light"]);
    expect(root.style.getPropertyValue(property("accent.base"))).toBe(
      tinted(BUILT_IN["purlis-light"], "red").values["accent.base"],
    );
    drawTint(null);
    expect(root.style.getPropertyValue(property("accent.base"))).toBe(
      BUILT_IN["purlis-light"].values["accent.base"],
    );
  });
});

describe("a theme's own recipe", () => {
  afterEach(() => {
    drawTint(null);
    drawIn(DEFAULT_THEME);
  });

  it("draws a theme that turns tint off untinted, on the window and on a workspace's tab", () => {
    const off = withTint(DEFAULT_THEME, { window: [], tabs: [] });
    const root = document.documentElement;
    drawIn(off);
    drawTint("teal");
    expect(root.style.getPropertyValue(property("accent.base"))).toBe(
      DEFAULT_THEME.values["accent.base"],
    );
    expect(tinted(off, "teal")).toBe(off);
    expect(tabTint(off, "teal")).toEqual({});
    // A persona's mark is its own colour, whatever the workspace tint does.
    expect(tintVariables(off, "teal", ["accent.base"])).not.toEqual({});
  });

  it("tints the tokens the theme names, where it names them", () => {
    const own = withTint(DEFAULT_THEME, {
      window: ["surface.raised"],
      tabs: ["surface.raised", "layer.chat"],
    });
    const root = document.documentElement;
    drawIn(own);
    drawTint("green");
    const tint = tinted(own, "green");
    expect(root.style.getPropertyValue(property("surface.raised"))).toBe(
      tint.values["surface.raised"],
    );
    expect(tint.values["surface.raised"]).not.toBe(own.values["surface.raised"]);
    expect(root.style.getPropertyValue(property("accent.base"))).toBe(own.values["accent.base"]);
    expect(Object.keys(tabTint(own, "green"))).toEqual([
      property("surface.raised"),
      property("layer.chat"),
    ]);
  });

  it("draws a colour untinted when no amount of colour keeps the accent apart from danger", () => {
    const grey = {
      ...DEFAULT_THEME,
      values: { ...DEFAULT_THEME.values, "accent.base": "#777777", "danger.base": "#777777" },
    };
    expect(tinted(grey, "red")).toBe(grey);
  });

  it("gives a workspace colour the hue the theme gives it", () => {
    const blue = hueIn(DEFAULT_THEME, "blue") ?? Number.NaN;
    const own = withTint(DEFAULT_THEME, {
      hues: DEFAULT_THEME.tint.hues.map((one) =>
        one.name === "red" ? { ...one, hue: blue } : one,
      ),
    });
    expect(tinted(own, "red").values).toEqual(tinted(DEFAULT_THEME, "blue").values);
    expect(tinted(own, "teal").values).toEqual(tinted(DEFAULT_THEME, "teal").values);
  });
});
