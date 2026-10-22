// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Which cell of the matrix this test page runs in** (#2945, `matrix.ts`): handed to the page by
 * its Vitest project (`vitest.project.ts`) and put where the decorator in `preview.tsx` reads it,
 * so every story in the page is drawn in that cell's theme, appearance and density.
 */
import { inject } from "vitest";
import { CELL_GLOBAL, type Cell } from "./matrix";

declare module "vitest" {
  export interface ProvidedContext {
    storyCell: Cell;
  }
}

(globalThis as Record<string, unknown>)[CELL_GLOBAL] = inject("storyCell");
