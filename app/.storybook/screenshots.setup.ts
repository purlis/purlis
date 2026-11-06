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
import { afterEach, beforeEach, expect, inject, type TestContext } from "vitest";
import { cdp, commands, page, server } from "vitest/browser";
import { facesLoaded } from "../src/theme/theme";
import { PICTURE_GLOBAL } from "../src/stories.testkit";
import { cellInTest, pictureOf } from "./matrix";

type Taken = false | "frame" | "page";

type Ran = { story?: { id: string; parameters: { screenshot?: Taken } } };

declare module "vitest" {
  export interface ProvidedContext {
    /** The image's own monospace (`monospace.ts`), where pictures are taken. */
    storyMonospace?: string;
  }
  interface TaskMeta {
    /** What this story's picture is of, for the reporter (`tools/story-failures.mjs`): a story
     *  that takes none has its baselines called obsolete (`tools/story-baselines.mjs`). */
    screenshot?: { story: string; taken: Taken };
    /** Where the picture stood when it came out another size than its baseline (#5463). */
    pictureMiss?: PictureMiss;
  }
}

declare module "vitest/browser" {
  interface BrowserCommands {
    /** Playwright's own bounding box of what `selector` names in the story's page, the box its
     *  element picture is cut by (`vitest.project.ts`). */
    pictureBox: (selector: string) => Promise<Box | null>;
  }
}

/** A box, in CSS pixels. */
type Box = { x: number; y: number; width: number; height: number };

/** Where a picture's element stood, as the page lays it out. */
type LaidOut = {
  frame: Box;
  /** Each terminal (`.xterm`) in it: its height rounds as the frame's does not (#5463). */
  terminals: Box[];
  devicePixelRatio: number;
  scroll: { x: number; y: number };
  viewport: (Box & { scale: number }) | null;
};

/**
 * **A picture that came out another size than its baseline** (#5463): the size each had, and
 * where the element stood just before the picture and just after the miss, with the box
 * Playwright cut it by, so the next miss carries its evidence.
 */
export type PictureMiss = {
  said: string;
  before: LaidOut;
  after: LaidOut;
  playwright: Box | null;
};

/** The size miss Vitest's comparator says: `Expected image dimensions to be …`. */
const SIZE_MISS = /Expected image dimensions to be [^\n]*/;

const boxOf = (element: Element): Box => {
  const { x, y, width, height } = element.getBoundingClientRect();
  return { x, y, width, height };
};

function laidOut(frame: Element): LaidOut {
  const viewport = window.visualViewport;
  return {
    frame: boxOf(frame),
    terminals: [...frame.querySelectorAll(".xterm")].map(boxOf),
    devicePixelRatio: window.devicePixelRatio,
    scroll: { x: window.scrollX, y: window.scrollY },
    viewport: viewport && {
      x: viewport.offsetLeft,
      y: viewport.offsetTop,
      width: viewport.width,
      height: viewport.height,
      scale: viewport.scale,
    },
  };
}

/** The attribute that names a missed picture's element to Playwright, for as long as it measures. */
const MEASURED = "data-picture-measured";

/** Playwright's box of `frame`, or `null` where it can't be had: evidence, never a failure. */
async function playwrightBoxOf(frame: Element): Promise<Box | null> {
  frame.setAttribute(MEASURED, "");
  try {
    return await commands.pictureBox(`[${MEASURED}]`);
  } catch {
    return null;
  } finally {
    frame.removeAttribute(MEASURED);
  }
}

// No caret: a blinking one is a different picture each time.
const still = document.createElement("style");
still.textContent = "* { caret-color: transparent !important; }";
document.head.append(still);

/** How wide a line of code-like text draws in `family`, at a size where two families part. */
function widthIn(family: string): number {
  const probe = document.createElement("span");
  probe.textContent = "07e0ff5 mmmiii WWW";
  probe.style.cssText = "position: absolute; inset-inline-start: -9999px; white-space: pre";
  probe.style.font = `100px ${family}`;
  document.body.append(probe);
  const width = probe.getBoundingClientRect().width;
  probe.remove();
  return width;
}

/** **Chromium's `monospace` pinned to the image's own** (`monospace.ts`), once a page, before
 *  its first story renders; and checked: a page whose `monospace` still draws another family
 *  fails its stories rather than take their pictures in it. */
let pinned: Promise<void> | undefined;
function pinMonospace(): Promise<void> {
  const family = inject("storyMonospace");
  if (server.browser !== "chromium" || family === undefined) return Promise.resolve();
  pinned ??= (async () => {
    await cdp().send("Page.setFontFamilies", { fontFamilies: { fixed: family } });
    // The setting reaches the page's renderer after the call returns: a few frames at most.
    const named = `"${family}"`;
    const until = performance.now() + 2000;
    while (widthIn("monospace") !== widthIn(named)) {
      if (performance.now() > until)
        throw new Error(`monospace still draws another family than ${named} after pinning it`);
      await new Promise((frame) => requestAnimationFrame(frame));
    }
  })();
  return pinned;
}

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
  const before = laidOut(frame);
  try {
    await expect.element(page.elementLocator(frame)).toMatchScreenshot(pictureOf(story.id, cell));
  } catch (missed) {
    const said = SIZE_MISS.exec(missed instanceof Error ? missed.message : String(missed));
    if (said)
      context.task.meta.pictureMiss = {
        said: said[0],
        before,
        after: laidOut(frame),
        playwright: await playwrightBoxOf(frame),
      };
    throw missed;
  }
  // PROBE (#5463), to be removed: the measurement, on a picture that did not miss.
  if (/waitingstrip--over-a-real-terminal/.test(story.id))
    console.log(
      "PROBE5463",
      cell.name,
      JSON.stringify({ before, after: laidOut(frame), playwright: await playwrightBoxOf(frame) }),
    );
}

beforeEach(async (context) => {
  running = { context, taken: false };
  await pinMonospace();
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
