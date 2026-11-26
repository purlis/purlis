// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";

/**
 * **Waits until the window is drawn, and so ready for the keyboard** (#1469).
 *
 * The Tauri service hands a spec its session as soon as the window EXISTS: it waits for a
 * window handle and for its own plugin, not for the app. On a cold start the page can still be
 * loading its bundle then — `window.__TAURI__` is there (the core injects it before any page
 * script), so a spec's first `invoke` answers, but nothing of the app is drawn yet and a key
 * sent now reaches an empty `<body>`. The first spec of a run is the only one that starts that
 * early, and on a slow Linux runner its first `F2` was lost: the palette never came and every
 * case after it fell with it.
 *
 * The condition is the app's own root, `<main class="window">`, in the document. The palette's
 * key listener is added in the same commit that puts it there (`Palette.tsx`, a layout effect),
 * so once it is drawn a key is heard. Read inside the page, one `execute` per poll; a poll the
 * page cannot answer yet counts as not drawn.
 */
export async function theWindowIsDrawn(): Promise<void> {
  await browser.waitUntil(
    async () => {
      try {
        return await browser.execute(() => document.querySelector(".window") !== null);
      } catch {
        // A page still between documents answers nothing yet; the next poll asks again.
        return false;
      }
    },
    {
      timeout: 60_000,
      interval: 250,
      timeoutMsg: 'the app never drew its window: no <main class="window"> in 60 seconds',
    },
  );
}

/** What the page answers when asked whether it paints: the frames it saw in the time it waited,
 *  and what it says of its own visibility. */
export type Painting = { frames: number; waitedMs: number; visibility: string };

/** **The frames a window must paint** before a spec may lean on them: two, so one is a frame
 *  after another and not a callback run once as the page was shown. */
export const FRAMES_NEEDED = 2;

/** How long the page waits for those frames: at 60 Hz they take about 33 ms, so this is only
 *  ever reached by a window that is not painting at all. */
export const FRAMES_WAIT_MS = 5_000;

/**
 * **Why a window that paints no frames cannot run a spec, in a line** (#7738), or nothing when
 * it painted enough.
 *
 * WebKit runs `requestAnimationFrame` only for a window that is on a display it can see: one
 * that is occluded, on a display asleep, behind the lock screen or on a Mac with no display at
 * all paints nothing, while its DOM still answers every query. The Library draws its rows a
 * screen a frame (#4351) and the outcome bars time an opening by the frame after it, so on such
 * a window the first screen is drawn and nothing after it, and the failures read as the
 * studio's. On the benchmark Mac's first run (45897228656) that is what all three harnesses did.
 */
export function noFramesSaid(painting: Painting): string | undefined {
  if (painting.frames >= FRAMES_NEEDED) return undefined;
  return (
    `the window painted ${painting.frames} frames in ${Math.round(painting.waitedMs)} ms ` +
    `(document.visibilityState: ${painting.visibility}), so nothing that waits for a frame ` +
    `can happen: WebKit paints no frame for a window that is occluded, on a display asleep, ` +
    `behind the lock screen, or on a Mac with no display. On a self-hosted Mac: keep it ` +
    `logged in and unlocked, with a display (or a dummy one) awake (docs/updating.md, ` +
    `"The outcome bars on a self-hosted Mac")`
  );
}

/**
 * **Waits until the window has painted frames** (#7738), and fails at once, with the cause, when
 * it paints none: what the window draws "in the frames after" never comes on such a window, and
 * every spec after would fail for it, each as its own.
 */
export async function theWindowPaintsFrames(): Promise<void> {
  // No function is named inside the page's script: the spec's bundler wraps a named one in a
  // `__name` call the page does not have (`acting.ts`). So frames are counted by anonymous
  // `requestAnimationFrame` callbacks a timer asks for, and the timer ends the wait.
  const painting = await browser.executeAsync(
    (
      needed: number,
      waitMs: number,
      finish: (painting: { frames: number; waitedMs: number; visibility: string }) => void,
    ) => {
      const started = performance.now();
      const seen = { frames: 0 };
      const poll = setInterval(() => {
        requestAnimationFrame(() => {
          seen.frames += 1;
        });
        const waited = performance.now() - started;
        if (seen.frames >= needed || waited >= waitMs) {
          clearInterval(poll);
          finish({ frames: seen.frames, waitedMs: waited, visibility: document.visibilityState });
        }
      }, 50);
    },
    FRAMES_NEEDED,
    FRAMES_WAIT_MS,
  );
  const said = noFramesSaid(painting);
  if (said !== undefined) throw new Error(said);
}
