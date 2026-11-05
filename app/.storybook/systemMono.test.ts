// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { STORIES_MONO, faces, inStoriesMono } from "./systemMono";

type Font = { stack: string; faces: { asset: string; weight: number; style: string }[] };
type Drawn = { fonts: Record<string, Font> };
// Named through a variable, as `matrix.test.ts` does: Vite rewrites `new URL("<literal>", …)`
// into a served URL, which is not a file.
const json = (file: string): unknown =>
  JSON.parse(readFileSync(fileURLToPath(new URL(file, import.meta.url)), "utf8"));
const THEMES = json("built-in-themes.json") as Record<string, Record<string, Drawn>>;

/** A stack's first family, unquoted. */
const first = (stack: string): string => stack.split(",")[0].trim().replace(/^"|"$/g, "");

describe("a story's monospace (#5425)", () => {
  const slots = Object.entries(THEMES).flatMap(([theme, appearances]) =>
    Object.entries(appearances).flatMap(([appearance, handed]) =>
      (["mono", "terminal"] as const).map((slot) => ({
        cell: `${theme} ${appearance} ${slot}`,
        handed: handed.fonts[slot],
        drawn: inStoriesMono(handed).fonts[slot],
      })),
    ),
  );

  it.each(slots)("$cell is drawn first in a face the stories make", ({ handed, drawn }) => {
    if (handed.faces.length > 0) {
      expect(drawn).toEqual(handed);
    } else {
      expect(first(drawn.stack)).toBe(STORIES_MONO);
      expect(drawn.stack).toBe(`"${STORIES_MONO}", ${handed.stack}`);
    }
  });

  it("leaves Purlis Classic's handed theme as the core resolved it", () => {
    const handed = THEMES["purlis-classic"].light;
    const before = JSON.stringify(handed);
    inStoriesMono(handed);
    expect(JSON.stringify(handed)).toBe(before);
    expect(first(handed.fonts.mono.stack)).toBe("ui-monospace");
  });

  it("is made from Purlis's mono faces, upright, italic and bold", () => {
    const made = faces(THEMES);
    expect(made).toEqual(THEMES.purlis.dark.fonts.mono.faces);
    expect(made.map((face) => `${face.weight} ${face.style}`)).toEqual([
      "400 normal",
      "400 italic",
      "700 normal",
    ]);
  });
});
