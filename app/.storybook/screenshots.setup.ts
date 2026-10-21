// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The screenshot gate** (#2901): after a story has rendered, run its `play` function and passed
 * axe, its frame is compared with the baseline drawn for this story in this browser. Loaded only
 * in the pinned Playwright image (`vitest.project.ts`, `PURLIS_STORY_PICTURES`).
 *
 * A baseline is `src/<dir>/__screenshots__/<file>.stories.tsx/<story id>-<browser>-linux.png`.
 * A missing one fails, as a changed one does; both are written only by the labelled job
 * (`.github/workflows/stories-baselines.yml`, `docs/component-stories.md`).
 *
 * A story says what is taken with `parameters.screenshot`: the frame (the default), `"page"` for
 * one that draws outside its frame (a dialog, a menu, a popover), or `false` for none.
 */
import { afterEach, expect } from "vitest";
import { page } from "vitest/browser";

type Ran = { story?: { id: string; parameters: { screenshot?: false | "frame" | "page" } } };

// No caret: a blinking one is a different picture each time.
const still = document.createElement("style");
still.textContent = "* { caret-color: transparent !important; }";
document.head.append(still);

afterEach(async (context) => {
  const story = (context as unknown as Ran).story;
  // A story that already failed says why itself; a picture of it adds nothing.
  if (!story || context.task.result?.state === "fail") return;
  const taken = story.parameters.screenshot ?? "frame";
  if (taken === false) return;
  const frames = document.querySelectorAll("[data-story-frame]");
  const frame = taken === "page" ? document.body : frames[frames.length - 1];
  if (!frame) throw new Error(`story ${story.id} drew no [data-story-frame] to take a picture of`);
  await document.fonts.ready;
  await expect.element(page.elementLocator(frame)).toMatchScreenshot(story.id);
});
