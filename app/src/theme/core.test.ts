// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { VIEW_MARKS } from "../ActivityBar";
import { GEOMETRY_PROPERTIES } from "./geometry";
import { DURATIONS, EASINGS } from "./motion";
import {
  PLATFORM,
  PLATFORM_DRAWN,
  CURSOR_STYLE,
  CURSOR_STYLES,
  FLAGS,
  LINE_HEIGHT,
  TINTED_TABS,
  TOKENS,
} from "./theme";
import { PALETTE } from "./tint";

/**
 * **The window writes the vocabulary the core's theme engine validates** (spec #2313).
 *
 * The engine (`purlis_core::theme`) decides what a theme may say: the token names, the motion
 * names, the names a theme may not set because they are core geometry, which tokens a
 * workspace's colour turns, and the palette's hues. The window names the same lists to write a
 * resolved theme to the document and to tint it live, and a list that drifted would be a token
 * the engine checks and the window never draws, or one the window draws that no theme can set.
 * So each is read out of the engine's source here, and has to be the window's exactly.
 */

const THEME = join(process.cwd(), "..", "crates", "purlis-core", "src", "theme");
const read = (file: string) => {
  const source = readFileSync(join(THEME, file), "utf8");
  expect(source.length, `${file} read as empty`).toBeGreaterThan(0);
  return source;
};

/** The strings of `pub const <name>: &[&str] = &[…];` in `source`. */
function list(source: string, name: string): string[] {
  const at = new RegExp(`pub const ${name}: &\\[&str\\] = &\\[([^\\]]*)\\];`).exec(source);
  expect(at, `${name} was not found`).not.toBeNull();
  return [...(at?.[1] ?? "").matchAll(/"([^"]+)"/g)].map((hit) => hit[1]);
}

describe("the engine's vocabulary", () => {
  const vocabulary = read("vocabulary.rs");

  it("names the window's tokens, in the window's order", () => {
    expect(list(vocabulary, "TOKENS")).toEqual([...TOKENS]);
  });

  it("names the window's tokens that are not colours (#2327)", () => {
    expect(list(vocabulary, "FLAGS")).toEqual([...FLAGS]);
    expect(list(vocabulary, "CURSOR_STYLES")).toEqual([...CURSOR_STYLES]);
    expect(vocabulary).toContain(`pub const CURSOR_STYLE: &str = "${CURSOR_STYLE}";`);
    expect(vocabulary).toContain(`pub const LINE_HEIGHT: &str = "${LINE_HEIGHT}";`);
  });

  it("leaves the same tokens to the platform by the same word (#2327)", () => {
    expect(list(vocabulary, "PLATFORM_DRAWN")).toEqual([...PLATFORM_DRAWN]);
    expect(vocabulary).toContain(`pub const PLATFORM: &str = "${PLATFORM}";`);
  });

  it("names the window's motion tokens", () => {
    expect(list(vocabulary, "DURATIONS")).toEqual([...DURATIONS]);
    expect(list(vocabulary, "EASINGS")).toEqual([...EASINGS]);
  });

  it("refuses the window's core geometry", () => {
    expect(list(vocabulary, "GEOMETRY")).toEqual([...GEOMETRY_PROPERTIES]);
  });

  it("tints the tokens the window tints on a workspace's tabs", () => {
    expect(list(vocabulary, "TINTED")).toEqual([...TINTED_TABS]);
  });

  it("lets an icon set draw the window's own views, in the rails' order", () => {
    expect(list(vocabulary, "RAIL_VIEWS")).toEqual(Object.keys(VIEW_MARKS));
  });
});

describe("the engine's palette", () => {
  it("is the window's names and hues", () => {
    const at = /pub const PALETTE: \[\(&str, f64\); \d+\] = \[([\s\S]*?)\];/.exec(read("tint.rs"));
    expect(at, "PALETTE was not found").not.toBeNull();
    const hues = Object.fromEntries(
      [...(at?.[1] ?? "").matchAll(/\("([^"]+)",\s*([\d.]+)\)/g)].map(([, name, hue]) => [
        name,
        Number(hue),
      ]),
    );
    expect(hues).toEqual({ ...PALETTE });
  });
});
