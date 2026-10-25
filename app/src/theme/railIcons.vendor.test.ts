// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { VIEW_MARKS } from "../ActivityBar";
import { OWN_GLYPHS } from "./harnessGlyphs";
import { fromLucide, LUCIDE, type LucideNode, toPath } from "./railIcons.vendor";

/**
 * **Purlis's icon set is Lucide, converted** (#2337): each view's symbol in the Purlis theme
 * folder's `icons/icons.json` is what `fromLucide` makes of the Lucide icon {@link LUCIDE} names,
 * read from the `lucide-react` the window depends on, so a Lucide release that redraws a glyph
 * fails here instead of drifting.
 *
 * To change an icon: name it in {@link LUCIDE} and run this file with `PURLIS_ICONS_WRITE=1`.
 */

const FOLDER = join(process.cwd(), "..", "crates/purlis-core/src/theme/built-in/purlis");
const FILE = join(FOLDER, "icons/icons.json");
const LUCIDE_ICONS = join(process.cwd(), "node_modules/lucide-react/dist/esm/icons");

/** The elements of the Lucide icon `name`, as `lucide-react` ships them. */
async function lucide(name: string): Promise<readonly LucideNode[]> {
  const module = (await import(/* @vite-ignore */ join(LUCIDE_ICONS, `${name}.mjs`))) as {
    __iconData: { node: readonly LucideNode[] };
  };
  return module.__iconData.node;
}

async function converted() {
  const views: Record<string, { paths: string[] }> = {};
  for (const [view, name] of Object.entries(LUCIDE)) views[view] = fromLucide(await lucide(name));
  // The harness glyphs are purlis's own drawing, not Lucide's (#2922): the set ships the ones
  // the window draws, so a theme with no set and Purlis draw the same three.
  return { views, glyphs: OWN_GLYPHS };
}

describe("Purlis's icon set", () => {
  it("is every view's Lucide icon converted and the window's own harness glyphs, and nothing else", async () => {
    const made = await converted();
    if (process.env.PURLIS_ICONS_WRITE === "1")
      writeFileSync(FILE, `${JSON.stringify(made, null, 2)}\n`);
    expect(JSON.parse(readFileSync(FILE, "utf8"))).toEqual(made);
  });

  it("draws every view the rails have", () => {
    expect(Object.keys(LUCIDE).sort()).toEqual(Object.keys(VIEW_MARKS).sort());
  });

  it("ships Lucide's licence beside it", () => {
    const licence = readFileSync(join(FOLDER, "LICENSE"), "utf8");
    expect(licence).toContain("ISC License");
    expect(licence).toContain("Lucide Icons and Contributors");
    // `key` and `search` are among Lucide's icons derived from Feather, which is MIT.
    expect(licence).toContain("Copyright (c) 2013-present Cole Bemis");
  });
});

describe("a Lucide element as path data", () => {
  it("keeps a path's data as it is", () => {
    expect(toPath(["path", { d: "M12 7v5l4 2" }])).toBe("M12 7v5l4 2");
  });

  it("draws a circle as two half arcs", () => {
    expect(toPath(["circle", { cx: "11", cy: "11", r: "8" }])).toBe(
      "M3 11A8 8 0 1 0 19 11A8 8 0 1 0 3 11Z",
    );
  });

  it("draws a rectangle with its rounded corners", () => {
    expect(toPath(["rect", { x: "3", y: "3", width: "8", height: "8", rx: "2" }])).toBe(
      "M5 3H9A2 2 0 0 1 11 5V9A2 2 0 0 1 9 11H5A2 2 0 0 1 3 9V5A2 2 0 0 1 5 3Z",
    );
    expect(toPath(["rect", { x: "1", y: "2", width: "4", height: "2" }])).toBe("M1 2H5V4H1Z");
  });

  it("refuses an element it does not convert yet", () => {
    expect(() => toPath(["line", { x1: "0", y1: "0", x2: "1", y2: "1" }])).toThrow(
      "a line is not converted to a path yet",
    );
  });
});
