// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The faded-words gate** (#3790): after a story has rendered, run its `play` function and passed
 * axe, every word on the page that an `opacity` or a `filter: opacity()` fades, on its own box or
 * on any box above it, is measured as drawn on its ground, in this cell's theme
 * (`src/theme/fadedWords.ts`), and one under WCAG 1.4.3's floor fails the story.
 *
 * In Chromium only: what it reads is the cascade's answer, which is the same in WebKit, and a
 * second browser would only say each failure twice.
 */
import { afterEach, expect } from "vitest";
import { server } from "vitest/browser";
import { fadedWordsUnderFloor } from "../src/theme/fadedWords";
import { cellInTest } from "./matrix";

afterEach((context) => {
  if (server.browser !== "chromium" || context.task.result?.state === "fail") return;
  const cell = cellInTest()?.name ?? "no cell";
  const under = fadedWordsUnderFloor(document.body).map(
    ({ where, words, ratio, floor, opacity }) =>
      `${cell}: "${words}" (${where}) is ${ratio}:1 at opacity ${opacity}, under ${floor}:1`,
  );
  expect(under, "faded words under their floor (src/theme/fadedWords.ts)").toEqual([]);
});
