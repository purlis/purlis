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
import { cdp, page, server } from "vitest/browser";
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
  const probing = story.id.includes("over-a-real-terminal");
  if (probing) probe5463("before", story.id, cell, frame);
  const watching = probing ? watch5463(frame) : undefined;
  try {
    await expect.element(page.elementLocator(frame)).toMatchScreenshot(pictureOf(story.id, cell));
  } finally {
    if (probing) {
      const changes = watching?.();
      console.log("PROBE-5463-CHANGES " + JSON.stringify({ id: story.id, cell, changes }));
      probe5463("after", story.id, cell, frame);
    }
  }
}

/** PROBE (#5463, temporary): every frame in which a rect of the story's parts changed, until
 *  stopped. */
function watch5463(frame: Element): () => unknown[] {
  const parts = [
    frame,
    frame.querySelector(".pane-frame"),
    frame.querySelector(".pane-top"),
    frame.querySelector(".waiting-strip"),
    frame.querySelector('[data-testid="terminal"]'),
  ];
  const read = () =>
    parts
      .map((e) => {
        if (!e) return "-";
        const b = e.getBoundingClientRect();
        return `${b.y},${b.height}`;
      })
      .join(" ");
  const changes: unknown[] = [];
  let last = read();
  changes.push({ t: 0, at: last, scroll: window.scrollY });
  const began = performance.now();
  let going = true;
  const tick = () => {
    if (!going) return;
    const now = read();
    if (now !== last)
      changes.push({ t: Math.round(performance.now() - began), at: now, scroll: window.scrollY });
    last = now;
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
  return () => {
    going = false;
    changes.push({ t: Math.round(performance.now() - began), end: read() });
    return changes;
  };
}

/** PROBE (#5463, temporary): where the frame and what it holds stand, to the fraction. */
function probe5463(when: string, id: string, cell: unknown, frame: Element): void {
  const r = (e: Element | null | undefined) => {
    if (!e) return null;
    const b = e.getBoundingClientRect();
    return { y: b.y, h: b.height, b: b.bottom, x: b.x, w: b.width };
  };
  const chain: unknown[] = [];
  for (let at: Element | null = frame.parentElement; at; at = at.parentElement)
    chain.push({ tag: at.tagName, id: at.id, cls: at.className, ...r(at) });
  const mark = document.createElement("span");
  mark.style.cssText = "display:inline-block;inline-size:0;block-size:0";
  frame.after(mark);
  const baseline = mark.getBoundingClientRect().y;
  mark.remove();
  const host = frame.querySelector('[data-testid="terminal"]');
  const row = host?.querySelector(".xterm-rows > div");
  const parent = frame.parentElement ? getComputedStyle(frame.parentElement) : null;
  console.log(
    "PROBE-5463 " +
      JSON.stringify({
        when,
        id,
        cell,
        frames: document.querySelectorAll("[data-story-frame]").length,
        frame: r(frame),
        pane: r(frame.querySelector(".pane-frame")),
        top: r(frame.querySelector(".pane-top")),
        host: r(host),
        screen: r(host?.querySelector(".xterm-screen")),
        row: r(row),
        rows: host?.querySelectorAll(".xterm-rows > div").length,
        dataRows: (host as HTMLElement | null)?.dataset.rows,
        baseline,
        strut: parent && { font: parent.font, lineHeight: parent.lineHeight },
        scroll: [window.scrollX, window.scrollY],
        inner: [window.innerWidth, window.innerHeight],
        dpr: window.devicePixelRatio,
        vv: window.visualViewport && [
          window.visualViewport.offsetTop,
          window.visualViewport.pageTop,
          window.visualViewport.scale,
          window.visualViewport.height,
        ],
        iframe: r(window.frameElement),
        chain,
      }),
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
