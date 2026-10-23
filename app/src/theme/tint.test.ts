// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  BUILT_IN,
  CLASSIC_THEME,
  drawIn,
  drawTint,
  hueIn,
  PLATFORM,
  property,
  tabTint,
  TOKENS,
  tinted,
  tintVariables,
  WORKSPACE_COLOURS,
  type Theme,
  type Token,
} from "./theme";
import { PURLIS_LIGHT, classicAppearances } from "./handed.testkit";
import { apartChroma, chromaFor, hueOf, oklabDistance, tintHex } from "./tint";

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
    expect(hueIn(CLASSIC_THEME, "teal")).toBe(190);
    const purple = hueIn(CLASSIC_THEME, "purple") ?? Number.NaN;
    // Within two degrees: each channel is rounded to its byte on the shade's own side.
    expect(apart(hueOf(tintHex("#6a9fb5", purple)) ?? Number.NaN, purple)).toBeLessThan(2);
    for (const none of [null, undefined, "", "mauve", "#fff", "#12345", "#808080"])
      expect(hueIn(CLASSIC_THEME, none), String(none)).toBeUndefined();
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
        accent_tinted: boolean;
        rules: {
          token?: string;
          token_tinted?: boolean;
          from: string;
          tinted: boolean;
          distance: number;
        }[];
        hue: number;
        most: number | null | "none";
        tinted: string | null;
      }[];
    };
    expect(cases.tints.length).toBeGreaterThan(100);
    // Which tokens a tint holds to the chroma that keeps the rules: the sides of a rule, and no
    // other (#3034).
    for (const { token, held } of (cases as unknown as { held: { token: string; held: boolean }[] })
      .held)
      expect(chromaFor(token, 0.01, CLASSIC_THEME.tint.apart), token).toBe(
        held ? 0.01 : Number.POSITIVE_INFINITY,
      );
    for (const { colour, hue } of cases.hues) {
      if (hue === null) expect(hueIn(CLASSIC_THEME, colour), colour).toBeUndefined();
      else expect(hueIn(CLASSIC_THEME, colour), colour).toBeCloseTo(hue, 9);
    }
    for (const { hex, colour, tinted: want } of cases.tints)
      expect(tintHex(hex, hueIn(CLASSIC_THEME, colour) ?? Number.NaN), `${hex} in ${colour}`).toBe(
        want,
      );
    // The accent held apart from danger: the chroma held to, and the accent drawn.
    expect(cases.apart.length).toBeGreaterThanOrEqual(40);
    // The path where no chroma keeps them apart, and the tint is dropped, is held too, and so is
    // a rule whose other colour the tint turns as well (Keep blocked's edge).
    expect(cases.apart.some((one) => one.most === "none")).toBe(true);
    expect(cases.apart.some((one) => one.rules.some((rule) => rule.tinted))).toBe(true);
    // And a recipe that turns Keep blocked's edge and leaves the accent as it is.
    expect(cases.apart.some((one) => !one.accent_tinted)).toBe(true);
    for (const { accent, accent_tinted, rules, hue, most, tinted: want } of cases.apart) {
      const got = apartChroma(
        rules.map((rule) => ({
          // A rule may name a token of its own beside the accent: one of the accent's kin.
          token: rule.token ?? accent,
          tokenTinted: rule.token_tinted ?? accent_tinted,
          from: rule.from,
          fromTinted: rule.tinted,
          distance: rule.distance,
        })),
        hue,
      );
      const expected =
        most === null ? Number.POSITIVE_INFINITY : most === "none" ? undefined : most;
      expect(got, `${accent} at ${hue}`).toBe(expected);
      const drawn = got === undefined ? null : accent_tinted ? tintHex(accent, hue, got) : accent;
      expect(drawn, `${accent} at ${hue}`).toBe(want);
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

  // Purlis Classic's: in Purlis, red, orange and pink keep the accent apart from danger by
  // drawing it untinted, and their strip in the colour (`purlis_tests.rs`, #3034).
  it.each(["dark", "light"])("gives Purlis Classic %s's accent the colour's hue", (id) => {
    const theme = classicAppearances().find((one) => one.id === id)?.theme as Theme;
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
        // Within a byte of a channel, and never across the middle's side it was on (#2526).
        expect(Math.abs(now - was) / Math.max(was, 0.01), `${colour} ${token}`).toBeLessThan(0.05);
        expect(was >= MIDDLE ? now >= was : now <= was, `${colour} ${token}`).toBe(true);
      }
    }
  });

  it("colours a grey shade, which has no hue of its own", () => {
    const shade = CLASSIC_THEME.values["layer.chat"];
    expect(tinted(CLASSIC_THEME, "green").values["layer.chat"]).not.toBe(shade);
    expect(hueOf(tinted(CLASSIC_THEME, "green").values["layer.chat"])).toBeDefined();
  });

  it("is nothing without a colour, or with one purlis does not read", () => {
    expect(tinted(CLASSIC_THEME, null)).toBe(CLASSIC_THEME);
    expect(tinted(CLASSIC_THEME, "mauve")).toBe(CLASSIC_THEME);
    expect(tabTint(CLASSIC_THEME, null)).toEqual({});
  });

  it("takes a custom colour's hue", () => {
    const custom = "#c06020";
    const hue = hueOf(custom) ?? Number.NaN;
    const accent = tinted(CLASSIC_THEME, custom).values["accent.base"];
    expect(apart(hueOf(accent) ?? Number.NaN, hue)).toBeLessThan(4);
    expect(accent).not.toBe(CLASSIC_THEME.values["accent.base"]);
  });

  it("is the same theme object each time it is asked, so a redraw repaints nothing", () => {
    expect(tinted(CLASSIC_THEME, "blue")).toBe(tinted(CLASSIC_THEME, "blue"));
  });

  it("is handed to an element as the custom properties of the tokens asked for", () => {
    const tint = tinted(CLASSIC_THEME, "pink");
    expect(tintVariables(CLASSIC_THEME, "pink", ["layer.chat", "accent.base"])).toEqual({
      [property("layer.chat")]: tint.values["layer.chat"],
      [property("accent.base")]: tint.values["accent.base"],
    });
  });
});

describe("the window's tint", () => {
  afterEach(() => {
    drawTint(null);
    drawIn(CLASSIC_THEME);
  });

  it("puts the workspace in front's accent and focus ring on the window, and no tab shade", () => {
    const root = document.documentElement;
    drawIn(CLASSIC_THEME);
    drawTint("yellow");
    const tint = tinted(CLASSIC_THEME, "yellow");
    for (const token of CLASSIC_THEME.tint.window as readonly Token[])
      expect(root.style.getPropertyValue(property(token)), token).toBe(tint.values[token]);
    // The tab shades are each tab's own, set on the tab: the project strip is not a workspace's.
    for (const token of ["layer.project", "layer.workspace", "layer.chat"] as const)
      expect(root.style.getPropertyValue(property(token)), token).toBe(CLASSIC_THEME.values[token]);
  });

  it("follows the theme drawn under it, and goes when the workspace has no colour", () => {
    const root = document.documentElement;
    drawTint("red");
    drawIn(PURLIS_LIGHT);
    expect(root.style.getPropertyValue(property("accent.base"))).toBe(
      tinted(PURLIS_LIGHT, "red").values["accent.base"],
    );
    drawTint(null);
    expect(root.style.getPropertyValue(property("accent.base"))).toBe(
      PURLIS_LIGHT.values["accent.base"],
    );
  });
});

describe("a theme's own recipe", () => {
  afterEach(() => {
    drawTint(null);
    drawIn(CLASSIC_THEME);
  });

  it("draws a theme that turns tint off untinted, on the window and on a workspace's tab", () => {
    const off = withTint(CLASSIC_THEME, { window: [], tabs: [] });
    const root = document.documentElement;
    drawIn(off);
    drawTint("teal");
    expect(root.style.getPropertyValue(property("accent.base"))).toBe(
      CLASSIC_THEME.values["accent.base"],
    );
    expect(tinted(off, "teal")).toBe(off);
    expect(tabTint(off, "teal")).toEqual({});
    // A persona's mark is its own colour, whatever the workspace tint does.
    expect(tintVariables(off, "teal", ["accent.base"])).not.toEqual({});
  });

  it("tints the tokens the theme names, where it names them", () => {
    const own = withTint(CLASSIC_THEME, {
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

  it("draws only the sides of a rule untinted when no amount of colour keeps them apart", () => {
    const grey = {
      ...CLASSIC_THEME,
      values: { ...CLASSIC_THEME.values, "accent.base": "#777777", "danger.base": "#777777" },
    };
    const tint = tinted(grey, "red");
    // The accent and its kin stay as written; the strip's shades still take red (#3034).
    for (const token of ["accent.base", "accent.surface", "focus.ring", "tab.active"] as const)
      expect(tint.values[token], token).toBe(grey.values[token]);
    const red = hueIn(grey, "red") ?? Number.NaN;
    expect(apart(hueOf(tint.values["layer.chat"]) ?? Number.NaN, red)).toBeLessThan(6);
  });

  it("leaves a value left to the platform as it is", () => {
    const own = {
      ...withTint(CLASSIC_THEME, { tabs: ["accent.base", "caret"] }),
      values: { ...CLASSIC_THEME.values, caret: PLATFORM },
    };
    const tint = tinted(own, "teal");
    expect(tint.values.caret).toBe(PLATFORM);
    expect(tint.values["accent.base"]).not.toBe(own.values["accent.base"]);
  });

  it("gives a workspace colour the hue the theme gives it", () => {
    const blue = hueIn(CLASSIC_THEME, "blue") ?? Number.NaN;
    const own = withTint(CLASSIC_THEME, {
      hues: CLASSIC_THEME.tint.hues.map((one) =>
        one.name === "red" ? { ...one, hue: blue } : one,
      ),
    });
    expect(tinted(own, "red").values).toEqual(tinted(CLASSIC_THEME, "blue").values);
    expect(tinted(own, "teal").values).toEqual(tinted(CLASSIC_THEME, "teal").values);
  });
});

describe("every tint", () => {
  it("keeps Allow's edge apart from Keep blocked's at every whole degree of a custom colour", () => {
    // Keep blocked's edge the accent turned red with half its colour: one colour with Allow's
    // under any tint that turns both alike.
    const accent = CLASSIC_THEME.values["accent.base"];
    const edges = {
      ...withTint(CLASSIC_THEME, { tabs: ["accent.base", "border.strong"] }),
      values: { ...CLASSIC_THEME.values, "border.strong": tintHex(accent, 25, 0.05) },
    };
    for (let degree = 0; degree < 360; degree++) {
      const colour = tintHex("#c0392b", degree);
      const tint = tinted(edges, colour);
      expect(
        oklabDistance(tint.values["accent.base"], tint.values["border.strong"]),
        `${degree}`,
      ).toBeGreaterThanOrEqual(0.1);
      expect(
        oklabDistance(tint.values["accent.base"], tint.values["danger.base"]),
        `${degree}`,
      ).toBeGreaterThanOrEqual(0.1);
    }
  });
});

describe("a recipe that turns only Keep blocked's edge", () => {
  it("keeps the edge apart from Allow's at every whole degree, and turns it still", () => {
    const accent = CLASSIC_THEME.values["accent.base"];
    const edge = {
      ...withTint(CLASSIC_THEME, { tabs: ["border.strong"] }),
      values: { ...CLASSIC_THEME.values, "border.strong": tintHex(accent, 25, 0.05) },
    };
    let turned = 0;
    for (let degree = 0; degree < 360; degree++) {
      const tint = tinted(edge, tintHex("#c0392b", degree));
      expect(tint.values["accent.base"], `${degree}`).toBe(accent);
      expect(
        oklabDistance(accent, tint.values["border.strong"]),
        `${degree}`,
      ).toBeGreaterThanOrEqual(0.1);
      if (tint.values["border.strong"] !== edge.values["border.strong"]) turned++;
    }
    expect(turned).toBeGreaterThan(300);
  });
});

/** WCAG's middle luminance: black and white contrast with a colour this bright alike. */
const MIDDLE = Math.sqrt(1.05 * 0.05) - 0.05;

/** The tokens in the accent's hue drawn beside danger, which every tint keeps apart from it. */
const ACCENT_KIN = ["focus.ring", "list.selected-edge", "accent.surface", "tab.active"] as const;

describe("the consent floors under every tint (#2526)", () => {
  it.each(Object.keys(BUILT_IN))(
    "keep every shade of %s on its own side of the middle at every whole degree",
    (name) => {
      const theme = BUILT_IN[name];
      for (const token of TOKENS) {
        const value = theme.values[token];
        if (typeof value !== "string" || !/^#[0-9a-f]{6}/i.test(value)) continue;
        const was = luminance(value.slice(0, 7));
        for (let degree = 0; degree < 360; degree++) {
          const now = luminance(tintHex(value, degree).slice(0, 7));
          expect(was >= MIDDLE ? now >= was : now <= was, `${token} ${value} at ${degree}`).toBe(
            true,
          );
        }
      }
    },
  );

  it("is handed a rule keeping each accent-hued token apart from danger", () => {
    for (const token of ACCENT_KIN)
      expect(CLASSIC_THEME.tint.apart).toContainEqual({
        token,
        from: "danger.base",
        distance: 0.1,
      });
  });

  it.each(ACCENT_KIN)("keeps %s apart from danger at every whole degree", (token) => {
    // The token written as danger turned to teal: apart as written, one colour with danger under
    // a red tint that turned it whole.
    const danger = CLASSIC_THEME.values["danger.base"];
    const kin = {
      ...CLASSIC_THEME,
      values: { ...CLASSIC_THEME.values, [token]: tintHex(danger, 190) },
    };
    expect(oklabDistance(kin.values[token], danger)).toBeGreaterThanOrEqual(0.1);
    expect(oklabDistance(tintHex(kin.values[token], 25), danger)).toBeLessThan(0.1);
    for (let degree = 0; degree < 360; degree++) {
      const tint = tinted(kin, tintHex("#c0392b", degree));
      for (const rule of CLASSIC_THEME.tint.apart)
        expect(
          oklabDistance(tint.values[rule.token as Token], tint.values[rule.from as Token]),
          `${rule.token} at ${degree}`,
        ).toBeGreaterThanOrEqual(rule.distance);
    }
    expect(tinted(kin, "blue").values[token]).not.toBe(kin.values[token]);
  });

  it("gives a grey accent as much colour as keeps it apart from danger, not none", () => {
    const grey = {
      ...CLASSIC_THEME,
      values: {
        ...CLASSIC_THEME.values,
        "accent.base": "#808080",
        "focus.ring": "#808080",
        "list.selected-edge": "#808080",
      },
    };
    const danger = grey.values["danger.base"];
    expect(oklabDistance(tintHex("#808080", 25), danger)).toBeLessThan(0.1);
    for (let degree = 0; degree < 360; degree++) {
      const accent = tinted(grey, tintHex("#c0392b", degree)).values["accent.base"];
      expect(accent, `${degree}`).not.toBe("#808080");
      expect(oklabDistance(accent, danger), `${degree}`).toBeGreaterThanOrEqual(0.1);
    }
  });

  it("keeps a persona's mark on its floor in a theme that turns the workspace tint off", () => {
    // A mark's initial is the accent on the accent's surface, tinted by the persona's colour
    // whatever the recipe says: its contrast holds at every whole degree as it does untinted.
    for (const theme of Object.values(BUILT_IN)) {
      const off = withTint(theme, { window: [], tabs: [] });
      const ratio = (front: string, back: string) => {
        const [a, b] = [luminance(front), luminance(back)];
        return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
      };
      const was = ratio(off.values["accent.base"], off.values["accent.surface"]);
      for (let degree = 0; degree < 360; degree++) {
        const mark = tintVariables(off, tintHex("#c0392b", degree), [
          "accent.base",
          "accent.surface",
        ]);
        const drawn = (token: Token) => mark[property(token)] ?? off.values[token];
        const now = ratio(drawn("accent.base"), drawn("accent.surface"));
        expect(now, `${theme.name} at ${degree}`).toBeGreaterThanOrEqual(was);
      }
    }
  });
});
