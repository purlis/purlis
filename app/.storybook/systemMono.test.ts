// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { SYSTEM_MONO, faces } from "./systemMono";

type Font = { stack: string; faces: { asset: string; weight: number; style: string }[] };
// Named through a variable, as `matrix.test.ts` does: Vite rewrites `new URL("<literal>", …)`
// into a served URL, which is not a file.
const json = (file: string): unknown =>
  JSON.parse(readFileSync(fileURLToPath(new URL(file, import.meta.url)), "utf8"));
const THEMES = json("built-in-themes.json") as Record<
  string,
  Record<string, { fonts: Record<string, Font> }>
>;

/** A stack's family names, unquoted, in order. */
const names = (stack: string): string[] =>
  stack.split(",").map((name) => name.trim().replace(/^"|"$/g, ""));

describe("a story's system monospace (#5425)", () => {
  const stacks = Object.entries(THEMES).flatMap(([theme, appearances]) =>
    Object.entries(appearances).flatMap(([appearance, drawn]) =>
      (["mono", "terminal"] as const).map((slot) => ({
        cell: `${theme} ${appearance} ${slot}`,
        stack: drawn.fonts[slot].stack,
      })),
    ),
  );

  it.each(stacks)(
    "$cell names a system monospace the stories make before the machine's monospace",
    ({ stack }) => {
      const family = names(stack);
      const made = family.findIndex((name) => (SYSTEM_MONO as readonly string[]).includes(name));
      expect(made).toBeGreaterThanOrEqual(0);
      expect(made).toBeLessThan(family.indexOf("monospace"));
    },
  );

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
