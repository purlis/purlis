// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { appendFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";
import { aWindow, rootOf, settled } from "./projectSwitch.testkit";
import { keepTheirOrder } from "./commandOrder";
import { countReactsCommits, counts } from "./reactCommits.testkit";
import { STOPPED_MS, watchTheFrames } from "./whenDrawing";

keepTheirOrder();
countReactsCommits();

vi.mock("./SessionPane", async () => ({
  SessionPane: (await import("./countedPane.testkit")).CountedPane,
}));

// A read on a wall-clock beat (`everyWhileShown`: the saving rows every 10 s, the alerts), and a
// beat's catch-up as a project comes back (`aBeatSince`), is time and not a telling: paused while
// the window is hidden already (`whileShown.ts`), and here a slow runner's ten minutes away take
// more than a beat where one minute does not. Held still, as `projectSwitch.cost.test.tsx` holds
// them.
vi.mock("./whileShown", async (actual) => ({
  ...(await actual<typeof import("./whileShown")>()),
  everyWhileShown: () => () => {},
  aBeatSince: () => false,
}));

afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

/**
 * **A window come back to after time away owes no more work for the time it was away** (H11, the
 * black window after ~30 min away).
 *
 * Away is a hidden window, and also one that stays `visible` while the display sleeps or the
 * screen is locked: the engine then draws its frames slowly (one each 0.6 to 0.9 s in our own
 * stall log of 2026-11-13) or not at all (one gap of 76.8 s), and may hold the page's tasks back,
 * while the core goes on telling it what its chats and its plane did. Whatever the window does
 * for each telling meanwhile is work it may do all at once as it comes back, before its first
 * frame. So what it does for the time away is held to counts that do not grow with that time: ten
 * minutes away cost what one minute away costs.
 *
 * Held by counts, which are the same on every machine (ADR 0086). `AWAY_OUT=<file>` writes what
 * each case cost as a line of JSON.
 */

const SHAPE = { projects: 2, chats: 20, workspaces: 4 };

/** What the core's watcher says moved, in turn, every ten seconds of the time away. */
const ANSWERS = ["sidebar", "instructions", "git", "settings"];

/** How far apart the engine draws frames while the display is asleep, as our stall log saw. */
const DARK_FRAME_MS = 700;

/**
 * How the window is away: hidden (minimised, another desktop), or still `visible` with the
 * display asleep or the screen locked, its frames held back altogether or drawn slowly.
 */
type How = "hidden" | "dark, no frames" | "dark, slow frames";

type Away = {
  /** Commands sent to the core from the leaving until everything the return started has
   *  settled. */
  invoked: number;
  invokes: Record<string, number>;
  /** React commits over the same time. */
  commits: number;
  /** Those of them from the return on. */
  commitsBack: number;
};

/**
 * Mounts the window, leaves it `how`, has the core tell it `minutes` of busy chats and plane
 * changes (every chat of both projects moving each ten seconds, and one change to each project's
 * plane), then comes back to it, and answers what all of that cost.
 */
async function away(how: How, minutes: number): Promise<Away> {
  const core = await aWindow(SHAPE);
  let hidden = false;
  let dark = false;
  vi.spyOn(document, "hidden", "get").mockImplementation(() => hidden);
  vi.spyOn(document, "visibilityState", "get").mockImplementation(() =>
    hidden ? "hidden" : "visible",
  );
  // Away, the window draws no frames, or draws them slowly: what asked for one waits.
  const frame = window.requestAnimationFrame.bind(window);
  const waiting: FrameRequestCallback[] = [];
  vi.spyOn(window, "requestAnimationFrame").mockImplementation((then) => {
    if (!hidden && !dark) return frame(then);
    if (how === "dark, slow frames")
      return setTimeout(() => then(performance.now()), DARK_FRAME_MS) as unknown as number;
    waiting.push(then);
    return 0;
  });

  // As the window watches its frames (`main.tsx`), through the engine's frames above.
  const stopWatching = watchTheFrames();
  await act(() => new Promise((done) => setTimeout(done, 100)));

  Object.assign(counts, { rendered: 0, mounted: 0, unmounted: 0, commits: 0 });
  const from = core.asked.length;
  act(() => {
    if (how === "hidden") {
      hidden = true;
      document.dispatchEvent(new Event("visibilitychange"));
    } else dark = true;
  });
  // Time away is minutes, and the window can tell it is away within a second of it: the
  // first slow frame, or none for `STOPPED_MS` (`whenDrawing.ts`). What it is told comes after.
  await act(() => new Promise((done) => setTimeout(done, STOPPED_MS + DARK_FRAME_MS)));

  let sequence = 10_000;
  for (let tick = 0; tick < minutes * 6; tick++) {
    act(() => {
      for (let project = 0; project < SHAPE.projects; project++)
        for (let session = 1; session <= SHAPE.chats; session++)
          core.chatMoved({
            plane: rootOf(project),
            session,
            state: "working",
            needs_you: false,
            queue: [],
            sequence: (sequence += 1),
            moved_at: sequence,
            reports: [],
            refusals: [],
          });
    });
    for (let project = 0; project < SHAPE.projects; project++)
      await core.planeChanged(rootOf(project), ANSWERS[tick % ANSWERS.length]);
  }

  const commitsAway = counts.commits;
  act(() => {
    hidden = dark = false;
    if (how === "hidden") document.dispatchEvent(new Event("visibilitychange"));
    for (const then of waiting.splice(0)) frame(then);
  });
  // A slow frame asked for before the return still comes that late.
  await act(() => new Promise((done) => setTimeout(done, DARK_FRAME_MS * 2)));
  await settled(core.asked);
  stopWatching();

  const invokes: Record<string, number> = {};
  for (const one of core.asked.slice(from)) invokes[one.cmd] = (invokes[one.cmd] ?? 0) + 1;
  return {
    invoked: core.asked.length - from,
    invokes,
    commits: counts.commits,
    commitsBack: counts.commits - commitsAway,
  };
}

/**
 * Read on a timer while the window is hidden, by design: the asks are what the Dock badge counts
 * (`dockBadge.ts`), which is what the person sees while the window is away. One read once a
 * burst of moves settles (`asks.ts`), on a timer the engine slows while the window is hidden:
 * time, not tellings, so it is left out of what is compared.
 */
const READ_WHILE_HIDDEN = ["asks_waiting"];

const besides = (invokes: Record<string, number>) =>
  Object.fromEntries(Object.entries(invokes).filter(([cmd]) => !READ_WHILE_HIDDEN.includes(cmd)));

describe("a window come back to after time away", () => {
  for (const how of ["hidden", "dark, no frames", "dark, slow frames"] as const)
    it(`reads and draws no more for ten minutes away than for one, ${how}`, async () => {
      const minute = await away(how, 1);
      cleanup();
      clearMocks();
      vi.restoreAllMocks();
      const tenMinutes = await away(how, 10);
      if (process.env.AWAY_OUT)
        appendFileSync(process.env.AWAY_OUT, `${JSON.stringify({ how, minute, tenMinutes })}\n`);
      // Each project's plane is read again once, as the window comes back, however much changed.
      const once = besides(minute.invokes);
      const more = Object.entries(besides(tenMinutes.invokes)).filter(
        ([cmd, times]) => times > (once[cmd] ?? 0),
      );
      expect(more, "asked more often after ten minutes away than after one").toEqual([]);
      // And drawn no more often as it comes back, but for what the asks' answers draw
      // (`READ_WHILE_HIDDEN`).
      const asksMore = READ_WHILE_HIDDEN.reduce(
        (more, cmd) => more + (tenMinutes.invokes[cmd] ?? 0) - (minute.invokes[cmd] ?? 0),
        0,
      );
      expect(tenMinutes.commitsBack).toBeLessThanOrEqual(
        minute.commitsBack + Math.max(asksMore, 0),
      );
      // Nor drawn for each telling while away: ten times the tellings, and what is drawn then is
      // on the clock alone (the chats' store tells its readers at most every 250 ms with no
      // frame, `heldStore.ts`), which a slow runner's ten minutes take longer over.
      expect(tenMinutes.commits).toBeLessThan(minute.commits * 2);
    }, 300_000);
});
