// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The `stories` test projects** (#2901, #2945): every story in `src/**\/*.stories.tsx` run as a
 * test, in real Chromium and real WebKit, through Storybook's Vitest addon, once in each cell of
 * the matrix (`matrix.ts`): each appearance of each built-in theme, in each density. One project
 * per cell and browser, named `stories <cell> (<browser>)`, so `--project "stories *"` runs them
 * all and `--project "stories purlis_*"` one theme's. `docs/component-stories.md` says what each
 * story is held to and how a baseline changes.
 *
 * A story fails when it does not render, when its `play` function fails (the keyboard path), when
 * axe finds a violation (`preview.tsx`), and, where pictures are taken, when it no longer draws
 * its baseline (`screenshots.setup.ts`).
 *
 * **Pictures are taken only where `PURLIS_STORY_PICTURES=1`**, which only the pinned Playwright
 * image of `.github/workflows/stories.yml` sets, and only in the cells `pictured.json` names: a
 * picture drawn on any other machine differs by its fonts and its rasteriser, so it is neither
 * compared nor kept anywhere else.
 */
/// <reference types="node" />
import { readFileSync } from "node:fs";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { storybookTest } from "@storybook/addon-vitest/vitest-plugin";
import { playwright } from "@vitest/browser-playwright";
import type { TestProjectInlineConfiguration } from "vitest/config";
import { cellsOf, type BuiltInThemes, type Cell } from "./matrix.ts";

/** Chromium's flag for greyscale text, which draws the same whatever ran before it. */
const LCD_OFF = ["--disable-lcd-text"];

/** The environment variable that turns the screenshot gate on. */
export const PICTURES = "PURLIS_STORY_PICTURES";

const here = (file: string) => fileURLToPath(new URL(file, import.meta.url));

const json = (file: string): unknown => JSON.parse(readFileSync(here(file), "utf8"));

/** Every cell of the matrix: the built-in themes the core resolves (`built-in-themes.json`),
 *  pictured where `pictured.json` says. */
export function cells(): Cell[] {
  return cellsOf(json("built-in-themes.json") as BuiltInThemes, json("pictured.json") as string[]);
}

export function stories(): TestProjectInlineConfiguration[] {
  const pictures = process.env[PICTURES] === "1";
  return cells().map((cell) => ({
    extends: true,
    plugins: [storybookTest({ configDir: here(".") })],
    test: {
      name: `stories ${cell.name}`,
      provide: { storyCell: cell },
      setupFiles: [
        here("cell.setup.ts"),
        ...(pictures && cell.pictured ? [here("screenshots.setup.ts")] : []),
      ],
      browser: {
        enabled: true,
        headless: true,
        provider: playwright({}),
        // The engine purlis ships in on macOS and Linux (WKWebView, WebKitGTK), and the one
        // Windows' WebView2 is.
        //
        // **Chromium draws its words greyscale** (`--disable-lcd-text`). Its subpixel text takes
        // each glyph from a cache every story file in the run shares, so a word could come out
        // a pixel apart depending on what the stories before it drew: the List family's
        // `ToChooseFrom` drew three different pictures in eight identical runs (one picture
        // whenever its file ran alone), and the same picture in all eight with greyscale text.
        instances: [
          { browser: "chromium", provider: playwright({ launchOptions: { args: LCD_OFF } }) },
          { browser: "webkit" },
        ],
      },
    },
  }));
}
