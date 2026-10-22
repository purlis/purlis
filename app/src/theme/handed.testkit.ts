// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The built-in themes, handed to a test's window as the app's core hands them to the real
 * one** (`windowprefs::handed_built_ins` and `default_theme`, `src-tauri/src/windowprefs.rs`).
 *
 * The real window finds them in the page before its own code runs; a test's window is jsdom,
 * which nothing hands anything. So `test-setup.ts` imports this first, and it puts them on the
 * same global, in the shape the core resolves them to: each built-in pick's appearance
 * (`purlis-dark` and `purlis-light`, Purlis's dark and light, #3034; `purlis-classic`, Purlis
 * Classic's dark), and the default's, Purlis's dark. They are read from the files a story's page
 * reads them from (`.storybook/built-in-themes.json`, every appearance of every built-in theme as
 * the core resolves it, and `.storybook/at-creation.json`, which of them a window is handed),
 * which a test of the app's crate holds to the core (`storythemes.rs`). One engine resolves the
 * themes every test draws.
 */

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";

/** Vitest's working directory is `app`. */
const STORYBOOK = join(process.cwd(), ".storybook");

/** One appearance of a built-in theme, by reference into `built-in-themes.json`. */
type Cell = { theme: string; appearance: string };

const themes = JSON.parse(readFileSync(join(STORYBOOK, "built-in-themes.json"), "utf8")) as Record<
  string,
  Record<string, { name: string; appearance: string }>
>;

const atCreation = JSON.parse(readFileSync(join(STORYBOOK, "at-creation.json"), "utf8")) as {
  default: Cell;
  built_in: Record<string, Cell>;
};

const of = (cell: Cell) => themes[cell.theme][cell.appearance];

/** One built-in appearance, by its id, as the core hands it. */
export type HandedAppearance = { id: string; theme: { name: string; appearance: string } };

/** Purlis Classic's appearances, by their ids, dark first, as the core hands each. */
export function classicAppearances(): HandedAppearance[] {
  return Object.entries(themes["purlis-classic"]).map(([id, theme]) => ({ id, theme }));
}

/** What the core hands a window as it is created, as far as a test's window needs it. */
function handedAtCreation() {
  return {
    built_in: Object.fromEntries(
      Object.entries(atCreation.built_in).map(([pick, cell]) => [pick, of(cell)]),
    ),
    default_theme: of(atCreation.default),
  };
}

const GLOBAL = "__CHARTER_AT_CREATION__";
(globalThis as Record<string, unknown>)[GLOBAL] ??= handedAtCreation();
