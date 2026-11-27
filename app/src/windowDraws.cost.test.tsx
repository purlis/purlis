// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, screen, within } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";
import { aWindow, rootOf, settled } from "./projectSwitch.testkit";
import { keepTheirOrder } from "./commandOrder";
import { countReactsCommits, drawnIn, renderedIn } from "./reactCommits.testkit";
import { stripNamed } from "./test-strips";

keepTheirOrder();

/**
 * **What the window's main parts draw for the three things that happen to it most** (FS-G1a,
 * #7894, epic #7889): a press on another tab, a change to the project's settings on disk, and a
 * chat's state moving.
 *
 * The slow log of 2026-11-27 put half the window's stall time inside React's renders, a third
 * of it in the side views. These are the counts the W-tickets lower: each part by its name, in
 * the measured region it is drawn in (`Measured`, `slowLog.ts`), never a total, so a part drawn
 * once less is seen to be. The three Inbox parts are one component in three regions, and the
 * Panels one in five.
 *
 * At a fixed load, one project of 50 chats over two workspaces, each chat in a tab of its own
 * holding one pane, and the same counts at 10 chats: what a part draws for one of these is per
 * window, never per chat. No beat runs and the window's clock is held, so nothing but the one
 * thing measured is drawn. Held by counts, the same on every machine (ADR 0086).
 *
 * **A count that goes down is a W-ticket's win**: lower the table here in the same pull request,
 * and say which part drew less and why. One that goes up is a regression.
 */

countReactsCommits();

vi.mock("./SessionPane", async () => ({
  SessionPane: (await import("./countedPane.testkit")).CountedPane,
}));

// The window's beats (the saving rows, the alerts, the state clock, a gauge) do not run, and
// none has passed: a beat landing inside a measured moment draws what it reads.
vi.mock("./whileShown", async (actual) => ({
  ...(await actual<typeof import("./whileShown")>()),
  everyWhileShown: () => () => {},
  aBeatSince: () => false,
}));

// The window's clocks are held still, so no read is caught up for the time a slow runner took
// and no chat's time in its state moves between two moments.
beforeEach(() => {
  const now = performance.now();
  vi.spyOn(performance, "now").mockReturnValue(now);
  const today = Date.now();
  vi.spyOn(Date, "now").mockReturnValue(today);
});

afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

/** The window's main parts, each by the component's name and the regions it is drawn in. */
const PARTS = {
  PlaneView: ["PlaneView"],
  ChatsSection: ["chats"],
  Inbox: ["asks", "updates", "notices"],
  ChangesView: ["changes"],
  Panels: ["personas", "vaults", "memory", "todos", "sessions"],
} as const;

/** Each part as `[region, name]`. */
const EVERY_PART = Object.entries(PARTS).flatMap(([name, regions]) =>
  regions.map((region) => [region, name] as const),
);

/** How many times each part was drawn, `"<region> <name>"`, since the counts were cleared. */
type Draws = Record<string, number>;

const draws = (): Draws =>
  Object.fromEntries(
    EVERY_PART.map(([region, name]) => [`${region} ${name}`, drawnIn(region, name)]),
  );

/** Every part drawn `times` times. */
const each = (times: number): Draws =>
  Object.fromEntries(EVERY_PART.map(([region, name]) => [`${region} ${name}`, times]));

/**
 * **What each part draws now: the baseline** (2026-11-28, on main at #7924). Every part is
 * drawn again for a tab and three times for a settings change, though neither moves what
 * most of them show; a chat's state moving draws none of them.
 */
const BASELINE = {
  tab: each(1),
  settings: each(3),
  move: each(0),
};

const WORKSPACES = 2;

/** What each of the three costs the parts of a window holding `chats` chats. */
async function drawsFor(chats: number): Promise<typeof BASELINE> {
  const plane = rootOf(0);
  const { asked, planeChanged, chatMoved } = await aWindow({
    projects: 1,
    chats,
    workspaces: WORKSPACES,
  });
  const tabs = within(stripNamed("Tabs")).getAllByRole("tab");
  // The load is there: a tab for each chat of the workspace in front, and one pane on screen.
  expect(tabs).toHaveLength(chats / WORKSPACES);
  expect(screen.getAllByTestId("pane")).toHaveLength(1);
  const move = (state: string, at: number) =>
    chatMoved({
      plane,
      session: 2,
      state,
      needs_you: false,
      queue: [],
      sequence: 1_000 + at,
      moved_at: 1_000 + at,
      reports: [],
      refusals: [],
    });
  // The chat is heard of once before, so what is counted is a move, not its first word.
  act(() => move("working", 0));
  await settled(asked);

  const measured = async (moment: () => Promise<void> | void): Promise<Draws> => {
    renderedIn.clear();
    await moment();
    await settled(asked);
    return draws();
  };
  return {
    tab: await measured(() => act(() => void fireEvent.click(tabs[1]))),
    settings: await measured(() => planeChanged(plane, "settings")),
    move: await measured(() => act(() => move("idle", 1))),
  };
}

/** The counts at the fixed load, measured once for both tests. */
let atFifty: typeof BASELINE | undefined;

describe("what the window's main parts draw", () => {
  it("draws each part as the baseline says for a tab, a settings change and a chat's move, at 50 chats", async () => {
    atFifty = await drawsFor(50);
    // Each moment apart, so a red run names the one that moved.
    expect.soft(atFifty.tab, "a press on another tab").toEqual(BASELINE.tab);
    expect.soft(atFifty.settings, "a settings change").toEqual(BASELINE.settings);
    expect.soft(atFifty.move, "a chat's state moving").toEqual(BASELINE.move);
  }, 300_000);

  it("draws each part no more often with more chats", async () => {
    const atTen = await drawsFor(10);
    if (atFifty === undefined) {
      cleanup();
      clearMocks();
      atFifty = await drawsFor(50);
    }
    expect(atFifty).toEqual(atTen);
  }, 300_000);
});
