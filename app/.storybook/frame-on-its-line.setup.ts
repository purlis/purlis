// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Every story's frame stands at the top of its line** (#5463), in every cell and both
 * browsers, after every story: the frame is what a picture is taken of (`screenshots.setup.ts`),
 * and one standing a fraction of a pixel low comes out a row taller. Checked where no picture is
 * taken too, so a frame that moves is caught in the run that moves it, not in the run whose
 * picture happens to miss. In both browsers, unlike the faded-words and parts checks: where a
 * frame stands is the engine's own layout, and the miss that found it was WebKit's alone.
 */
import { afterEach, expect } from "vitest";
import { framesOffTheirLine } from "../src/stories.testkit";

afterEach((context) => {
  // A story that already failed says why itself.
  if (context.task.result?.state === "fail") return;
  expect(framesOffTheirLine(), "story frames off the top of their line (#5463)").toEqual([]);
});
