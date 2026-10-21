// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The `stories` test project** (#2901): every story in `src/**\/*.stories.tsx` run as a test, in
 * real Chromium and real WebKit, through Storybook's Vitest addon. `docs/component-stories.md`
 * says what each story is held to and how a baseline changes.
 *
 * A story fails when it does not render, when its `play` function fails (the keyboard path), when
 * axe finds a violation (`preview.tsx`), and, where pictures are taken, when it no longer draws
 * its baseline (`screenshots.setup.ts`).
 *
 * **Pictures are taken only where `PURLIS_STORY_PICTURES=1`**, which only the pinned Playwright
 * image of `.github/workflows/stories.yml` sets: a picture drawn on any other machine differs by
 * its fonts and its rasteriser, so it is neither compared nor kept anywhere else.
 */
/// <reference types="node" />
import process from "node:process";
import { fileURLToPath } from "node:url";
import { storybookTest } from "@storybook/addon-vitest/vitest-plugin";
import { playwright } from "@vitest/browser-playwright";
import type { TestProjectInlineConfiguration } from "vitest/config";

/** The environment variable that turns the screenshot gate on. */
export const PICTURES = "PURLIS_STORY_PICTURES";

const here = (file: string) => fileURLToPath(new URL(file, import.meta.url));

export function stories(): TestProjectInlineConfiguration {
  const pictures = process.env[PICTURES] === "1";
  return {
    extends: true,
    plugins: [storybookTest({ configDir: here(".") })],
    test: {
      name: "stories",
      setupFiles: pictures ? [here("screenshots.setup.ts")] : [],
      browser: {
        enabled: true,
        headless: true,
        provider: playwright({}),
        // The engine purlis ships in on macOS and Linux (WKWebView, WebKitGTK), and the one
        // Windows' WebView2 is.
        instances: [{ browser: "chromium" }, { browser: "webkit" }],
      },
    },
  };
}
