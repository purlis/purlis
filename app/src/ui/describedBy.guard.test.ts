// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync, readdirSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * **A family's row builds its `aria-describedby` in one place** (#6101, #6123): its parts (a
 * tooltip, a line under its words, a mark with words) hand their ids to the row's registry
 * (`RowHeard`, `ui/list/Row.tsx`), and the row's root writes them. A part that wrote the
 * attribute itself would drop what the root wrote, or lose its own id the next time the root
 * did: the list's tooltip did, until #6101.
 *
 * A list's row draws its element through React Aria's `render` prop, so React writes it. A
 * tree's row cannot (React Aria's `TreeItem` takes no `render` and filters `aria-describedby`
 * out of its props), so its root writes it on its element, once, in `useTreeRowHeard`. That is
 * the one place under `ui/` it is written by hand, by exact count, so another fails.
 *
 * Read as text, as the window's other guards are: a rendered row shows only what its test set up.
 */
const ROOT_WRITERS: Record<string, number> = {
  // `useTreeRowHeard`: the tree row's registry, written on its element.
  "tree/Tree.tsx": 2,
};

const UI = join(__dirname);
/** A write of the attribute by hand: set, taken back or toggled by its name, or its elements set. */
const WRITE =
  /\.(?:set|remove|toggle)Attribute(?:NS)?\([^)]*["']aria-describedby["']|\.ariaDescribedByElements\s*=/g;

function sources(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const at = join(dir, entry.name);
    if (entry.isDirectory()) return sources(at);
    if (!/\.tsx?$/.test(entry.name)) return [];
    if (/\.(?:test|stories|testkit)\.tsx?$/.test(entry.name)) return [];
    return [at];
  });
}

describe("a family's row description, written by hand (#6123)", () => {
  it("is written only by a tree row's root, never by a part", () => {
    const written: Record<string, number> = {};
    for (const file of sources(UI)) {
      const count = readFileSync(file, "utf8").match(WRITE)?.length ?? 0;
      if (count > 0) written[relative(UI, file).split(sep).join("/")] = count;
    }
    expect(written).toEqual(ROOT_WRITERS);
  });
});
