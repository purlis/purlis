// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Which cell of the matrix this test page runs in** (#2945, `matrix.ts`): handed to the page by
 * its Vitest project (`vitest.project.ts`) and put where the decorator in `preview.tsx` reads it,
 * so every story in the page is drawn in that cell's theme, appearance and density.
 *
 * **And the window a story may resize** (#3510, `inAWindow` in `src/stories.testkit.ts`): a
 * story that checks a short window asks for one, and the window is put back to its own size
 * after the story, so the next story is drawn in the window every other one is.
 */
import { afterEach, inject } from "vitest";
import { page } from "vitest/browser";
import { WINDOW_GLOBAL } from "../src/stories.testkit";
import { CELL_GLOBAL, type Cell } from "./matrix";

declare module "vitest" {
  export interface ProvidedContext {
    storyCell: Cell;
  }
}

(globalThis as Record<string, unknown>)[CELL_GLOBAL] = inject("storyCell");

/** The window's own size, while a story has resized it. */
let own: { width: number; height: number } | undefined;

(globalThis as Record<string, unknown>)[WINDOW_GLOBAL] = async (width: number, height: number) => {
  own ??= { width: window.innerWidth, height: window.innerHeight };
  await page.viewport(width, height);
};

afterEach(async () => {
  if (own === undefined) return;
  const { width, height } = own;
  own = undefined;
  await page.viewport(width, height);
});
