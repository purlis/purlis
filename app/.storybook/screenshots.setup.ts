// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The screenshot gate** (#2901): after a story has rendered, run its `play` function and passed
 * axe, its frame is compared with the baseline drawn for this story in this browser. Loaded only
 * in the pinned Playwright image (`vitest.project.ts`, `PURLIS_STORY_PICTURES`).
 *
 * Loaded only in the matrix cells `pictured.json` names (`matrix.ts`), each its own baseline:
 * `src/<dir>/__screenshots__/<file>.stories.tsx/<story id>_<theme>_<appearance>-<browser>-linux.png`.
 * A missing one fails, as a changed one does; both are written only by the labelled job
 * (`.github/workflows/stories-baselines.yml`, `docs/component-stories.md`).
 *
 * A story says what is taken with `parameters.screenshot`: the frame (the default), `"page"` for
 * one that draws outside its frame (a dialog, a menu, a popover), or `false` for none. Which it
 * is goes to the reporter with the test, since no index of the stories has a story's
 * parameters: the stories check fails on a baseline of a story that takes none.
 */
import "./handed";
import { afterEach, expect } from "vitest";
import { page } from "vitest/browser";
import { facesLoaded } from "../src/theme/theme";
import { cellInTest, pictureOf } from "./matrix";

type Taken = false | "frame" | "page";

type Ran = { story?: { id: string; parameters: { screenshot?: Taken } } };

declare module "vitest" {
  interface TaskMeta {
    /** What this story's picture is of, for the reporter (`tools/story-failures.mjs`): a story
     *  that takes none has its baselines called obsolete (`tools/story-baselines.mjs`). */
    screenshot?: { story: string; taken: Taken };
  }
}

// No caret: a blinking one is a different picture each time.
const still = document.createElement("style");
still.textContent = "* { caret-color: transparent !important; }";
document.head.append(still);

afterEach(async (context) => {
  const story = (context as unknown as Ran).story;
  if (!story) return;
  const taken = story.parameters.screenshot ?? "frame";
  context.task.meta.screenshot = { story: story.id, taken };
  // A story that already failed says why itself; a picture of it adds nothing.
  if (taken === false || context.task.result?.state === "fail") return;
  const frames = document.querySelectorAll("[data-story-frame]");
  const frame = taken === "page" ? document.body : frames[frames.length - 1];
  if (!frame) throw new Error(`story ${story.id} drew no [data-story-frame] to take a picture of`);
  const cell = cellInTest();
  if (!cell) throw new Error(`story ${story.id} ran in no cell of the matrix (cell.setup.ts)`);
  // In the theme's own fonts: every face it asked for made, then laid out.
  await facesLoaded();
  await document.fonts.ready;
  await expect.element(page.elementLocator(frame)).toMatchScreenshot(pictureOf(story.id, cell));
});
