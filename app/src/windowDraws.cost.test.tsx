// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, screen, within } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";
import { aWindow, rootOf, settled } from "./projectSwitch.testkit";
import { SETTLE_MS as ASKS_SETTLE_MS } from "./asks";
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
 *
 * **The settings change's three**, traced commit by commit (2026-11-28): the `PlaneView` that
 * heard `plane-changed`, as it is told; the `App` the project's settings reads answer into
 * (`project_lives_here` to `ask_persona_offer`, which answer together and are one commit); and
 * the `App` the sandbox's `chats_level_why` answers into, asked only once `chats_on_older_sandbox`
 * has answered (`SandboxChanged.tsx`, #5207), so always a commit of its own. The theme's read
 * (`theme_in_force`) commits `App` again but draws none of these parts, and the asks' read 50 ms
 * later answers what it did before and draws nothing. Nothing in it waits on a clock: no frame is
 * watched in a test, so `whenDrawing` (`planeChanged.ts`) tells at once, and the core's answers
 * come in the order their reads are chained, whatever the runner's speed.
 */
const BASELINE = {
  tab: each(1),
  settings: each(3),
  move: each(0),
};

const WORKSPACES = 2;

/**
 * Past the wait after which a chat's move or a settings change has the asks read again
 * (`asks.ts`), which is longer than the one `settled` looks for: each moment owns the read it
 * starts, and none lands in the next.
 */
async function asksReadAgain(asked: Parameters<typeof settled>[0]): Promise<void> {
  await act(async () => {
    await new Promise((done) => setTimeout(done, ASKS_SETTLE_MS * 4));
  });
  await settled(asked);
}

/** What each of the three costs the parts of a window holding `chats` chats. */
async function drawsFor(chats: number): Promise<typeof BASELINE> {
  const plane = rootOf(0);
  renderedIn.clear();
  const { asked, planeChanged, chatMoved } = await aWindow({
    projects: 1,
    chats,
    workspaces: WORKSPACES,
  });
  // Each part is drawn where the table looks for it, so a 0 below is a part not drawn again,
  // never a part, or a region, under another name.
  for (const [region, name] of EVERY_PART)
    expect(drawnIn(region, name), `${region} ${name} at mount`).toBeGreaterThan(0);
  const tabs = within(stripNamed("Tabs")).getAllByRole("tab");
  // The load is there: a tab for each chat of the workspace in front, and one pane on screen.
  expect(tabs).toHaveLength(chats / WORKSPACES);
  expect(screen.getAllByTestId("pane")).toHaveLength(1);
  expect(tabs[1]).toHaveAttribute("aria-selected", "false");
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
  await asksReadAgain(asked);

  /** The moment's draws, and how many times it read the asks. */
  const measured = async (moment: () => Promise<void> | void) => {
    renderedIn.clear();
    const from = asked.length;
    await moment();
    await asksReadAgain(asked);
    const asksRead = asked.slice(from).filter((one) => one.cmd === "asks_waiting").length;
    return { draws: draws(), asksRead };
  };
  const tab = await measured(() => act(() => void fireEvent.click(tabs[1])));
  // The press reached the strip: the tab pressed is the one in front.
  expect(tabs[1]).toHaveAttribute("aria-selected", "true");
  const settings = await measured(() => planeChanged(plane, "settings"));
  const moved = await measured(() => act(() => move("idle", 1)));
  // The move reached the window: it read the project's asks again, once (as
  // `chatMoved.cost.test.tsx` holds), and the settings change did too, in its own moment.
  expect(moved.asksRead, "asks read for the move").toBe(1);
  expect(settings.asksRead, "asks read for the settings change").toBe(1);
  return { tab: tab.draws, settings: settings.draws, move: moved.draws };
}

describe("what the window's main parts draw", () => {
  it("draws each part as the baseline says for a tab, a settings change and a chat's move, at 50 chats", async () => {
    const atFifty = await drawsFor(50);
    // Each moment apart, so a red run names the one that moved.
    expect.soft(atFifty.tab, "a press on another tab").toEqual(BASELINE.tab);
    expect.soft(atFifty.settings, "a settings change").toEqual(BASELINE.settings);
    expect.soft(atFifty.move, "a chat's state moving").toEqual(BASELINE.move);
  }, 300_000);

  it("draws each part no more often with more chats", async () => {
    const atTen = await drawsFor(10);
    cleanup();
    clearMocks();
    const atFifty = await drawsFor(50);
    expect(atFifty).toEqual(atTen);
  }, 600_000);
});
