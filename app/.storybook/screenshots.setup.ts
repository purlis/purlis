// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The screenshot gate** (#2901): after a story has rendered, run its `play` function and passed
 * axe, its frame is compared with the baseline drawn for this story in this browser. Loaded only
 * in the pinned Playwright image (`vitest.project.ts`, `PURLIS_STORY_PICTURES`).
 *
 * Loaded only in the matrix cells `pictured.json` names (`matrix.ts`), each its own baseline:
 * `src/<dir>/__screenshots__/<file>.stories.tsx/<story id>_<theme>_<appearance>-<browser>-linux.png`.
 * A missing one fails, as a changed one does; both are drawn only by a pull request's stories run
 * and taken from it on request (`.github/workflows/stories.yml`, `docs/component-stories.md`).
 *
 * A story says what is taken with `parameters.screenshot`: the frame (the default), `"page"` for
 * one that draws outside its frame (a dialog, a menu, a popover), or `false` for none. Which it
 * is goes to the reporter with the test, since no index of the stories has a story's
 * parameters: the stories check fails on a baseline of a story that takes none.
 *
 * A story whose `play` ends with what it pictures gone (a dialog it answers) takes its picture
 * during `play`, with `pictureNow()` (`src/stories.testkit.ts`), and none is taken after it.
 */
import "./handed";
import { afterEach, beforeEach, expect, type TestContext } from "vitest";
import { page } from "vitest/browser";
import { facesLoaded } from "../src/theme/theme";
import { PICTURE_GLOBAL } from "../src/stories.testkit";
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

/** The test running now, and whether its picture has been taken. */
let running: { context: TestContext; taken: boolean } | undefined;

/** Compares the story's picture with its baseline. */
async function picture(context: TestContext): Promise<void> {
  const story = (context as unknown as Ran).story;
  if (!story) return;
  const taken = story.parameters.screenshot ?? "frame";
  if (taken === false) return;
  const frames = document.querySelectorAll("[data-story-frame]");
  const frame = taken === "page" ? document.body : frames[frames.length - 1];
  if (!frame) throw new Error(`story ${story.id} drew no [data-story-frame] to take a picture of`);
  const cell = cellInTest();
  if (!cell) throw new Error(`story ${story.id} ran in no cell of the matrix (cell.setup.ts)`);
  // In the theme's own fonts: every face it asked for made, then laid out.
  await facesLoaded();
  await document.fonts.ready;
  await expect.element(page.elementLocator(frame)).toMatchScreenshot(pictureOf(story.id, cell));
}

beforeEach((context) => {
  running = { context, taken: false };
});

// `pictureNow()`, from a story's `play`: its picture, once, while what it pictures is drawn.
(globalThis as Record<string, unknown>)[PICTURE_GLOBAL] = async () => {
  if (!running || running.taken) return;
  running.taken = true;
  await picture(running.context);
};

afterEach(async (context) => {
  const now = running;
  running = undefined;
  // What the story's picture is of goes to the reporter whatever happens next, a picture taken
  // during `play` or none at all.
  const story = (context as unknown as Ran).story;
  if (story)
    context.task.meta.screenshot = {
      story: story.id,
      taken: story.parameters.screenshot ?? "frame",
    };
  // A story that already failed says why itself; a picture of it adds nothing. One taken during
  // its `play` is not taken again.
  if (now?.taken || context.task.result?.state === "fail") return;
  await picture(context);
});
