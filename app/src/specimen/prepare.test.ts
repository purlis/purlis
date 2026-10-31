// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  DEFAULT_THEME,
  drawTint,
  fetchFacesWith,
  forgetFaces,
  property,
  tinted,
  type Theme,
  type Token,
} from "../theme/theme";
import { handedFace, type Given } from "./page";
import { prepare } from "./prepare";

/**
 * **The specimen's page is drawn as the window draws itself** (#2536): tinted by the window's
 * own arithmetic with the workspace colour it was handed, and in the theme's own faces, made
 * from the bytes it was handed before anything is laid out.
 */

/** Every face made: the name it was made under and its bytes. */
let madeFaces: { family: string; bytes: number[] }[] = [];

class StandInFace {
  constructor(
    readonly family: string,
    bytes: ArrayBuffer,
  ) {
    madeFaces.push({ family, bytes: [...new Uint8Array(bytes)] });
  }
  load() {
    return new Promise((done) => setTimeout(() => done(this), 5));
  }
}

beforeEach(() => {
  madeFaces = [];
  forgetFaces();
  vi.stubGlobal("FontFace", StandInFace);
  Object.defineProperty(document, "fonts", {
    value: { add: () => undefined, ready: Promise.resolve() },
    configurable: true,
  });
});

afterEach(() => {
  drawTint(null);
  fetchFacesWith(undefined);
  vi.unstubAllGlobals();
});

const ASSET = "b".repeat(64);

/** Purlis Classic dark with a terminal font of one face. */
const typed: Theme = {
  ...DEFAULT_THEME,
  fonts: {
    ui: { family: null, stack: "sans-serif", faces: [] },
    mono: { family: null, stack: "monospace", faces: [] },
    terminal: {
      family: "Mono",
      stack: '"purlis-terminal-0123456789ab", monospace',
      faces: [{ asset: ASSET, weight: 400, style: "normal" }],
    },
  },
};

const given = (tint: string | null): Given => ({
  theme: typed,
  width: 1280,
  height: 800,
  tint,
  faces: { [ASSET]: btoa("wOF2face") },
});

describe("the specimen's page", () => {
  it("is tinted with the colour it was handed, as the window is tinted there", async () => {
    const root = document.createElement("div");
    await prepare(given("teal"), root);
    const tint = tinted(typed, "teal");
    expect(typed.tint.window.length).toBeGreaterThan(0);
    for (const token of typed.tint.window as readonly Token[])
      expect(root.style.getPropertyValue(property(token)), token).toBe(tint.values[token]);
    expect(tint.values["accent.base" as Token]).not.toBe(typed.values["accent.base" as Token]);
  });

  it("is drawn untinted with no colour", async () => {
    const root = document.createElement("div");
    await prepare(given(null), root);
    for (const token of typed.tint.window as readonly Token[])
      expect(root.style.getPropertyValue(property(token)), token).toBe(typed.values[token]);
  });

  it("makes each face from the bytes it was handed before it settles", async () => {
    await prepare(given(null), document.createElement("div"));
    expect(madeFaces).toEqual([
      {
        family: "purlis-terminal-0123456789ab",
        bytes: [...new TextEncoder().encode("wOF2face")],
      },
    ]);
  });

  it("is handed no face it was not given", () => {
    expect(handedFace(given(null), "c".repeat(64))).toBeUndefined();
    const face = handedFace(given(null), ASSET) ?? new ArrayBuffer(0);
    expect([...new Uint8Array(face)]).toEqual([...new TextEncoder().encode("wOF2face")]);
  });
});
