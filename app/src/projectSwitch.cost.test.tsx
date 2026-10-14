// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { appendFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";
import { switches, type Shape, type Switch } from "./projectSwitch.testkit";
import { keepTheirOrder } from "./commandOrder";

// As the window runs: the lanes that keep a read and a change in order (#1949), whose reads
// must not queue behind each other on a switch.
keepTheirOrder();

/**
 * **A switch between two projects the person is working in builds nothing** (`projectShown.tsx`).
 *
 * The operator's report was a freeze of one to three seconds on every switch, worse the more
 * chats were open. Counted here in the real window, a switch then mounted about 800 components
 * and 15 more for each chat of the project it showed, unmounted as many of the one it hid, sent
 * the core some fifty commands, and closed one terminal to start another. With the view of a
 * project kept while another is in front, it mounts the gear on the project's tab and nothing
 * and little else, starts no terminal, and sends nine commands before it is painted, which say
 * which project is in front, and eight after, which read again what the view held while it was
 * away. What a pane reads for itself, its gauge among them, is read after the paint too, so the
 * commands a switch waits on do not grow with the panes of the tab in front.
 *
 * **Held by counts, which are the same on every machine**, and not by milliseconds, which here
 * are jsdom's (ADR 0086). Each budget is far above what a switch costs now and far below what
 * it cost: a view that is built again on a switch goes through every one of them.
 *
 * `SWITCH_MATRIX=3x2,3x40,3x2x40` runs other shapes (projects x chats, and x the chats of the
 * second project alone), and `SWITCH_OUT=<file>` writes what each switch cost as a line of JSON.
 */

await vi.hoisted(async () => {
  const { countReactsCommits } = await import("./reactCommits.testkit");
  countReactsCommits();
});

vi.mock("./SessionPane", async () => ({
  SessionPane: (await import("./countedPane.testkit")).CountedPane,
}));

afterEach(() => {
  cleanup();
  clearMocks();
});

const line = (shape: Shape, one: Switch) =>
  JSON.stringify({
    projects: shape.projects,
    chats: shape.chats,
    chatsIn: shape.chatsIn,
    ...one,
    syncMs: Math.round(one.syncMs),
  });

/** The most a switch between two projects kept drawn may mount or unmount. It mounts 5 now, the
 *  gear on the tab of the project in front among them; built again, a view of twenty chats
 *  mounts 1,099. */
const MOUNTS = 20;
/** The most commands such a switch may send the core before the press returns, which is before
 *  the project is painted. It sends 9 now: which project is in front, whose Inbox is open, and
 *  three reads the core answers off the window's thread. One more is a new read in the switch;
 *  a view built again sent 49, all of them then. */
const COMMANDS_AT_ONCE = 10;
/** The most it may send in all where each tab holds one pane, with what it reads again once it
 *  is painted. It sends 17. With more panes it sends more, after the paint: two reads a pane. */
const COMMANDS = 20;
/** The most times React may commit for it, the reads' answers among them. It commits 5 times
 *  now, two of them small ones after the paint. */
const COMMITS = 6;

describe("what a project switch costs", () => {
  const matrix: Shape[] = (process.env.SWITCH_MATRIX ?? "3x20")
    .split(",")
    .map((cell) => cell.split("x").map(Number))
    .map(([projects, chats, inSecond]) => ({
      projects,
      chats,
      workspaces: 12,
      chatsIn: inSecond === undefined ? undefined : { 1: inSecond },
    }));

  for (const shape of matrix) {
    it(`builds nothing, among ${shape.projects} projects of ${shape.chats} chats ${JSON.stringify(shape.chatsIn ?? {})}`, async () => {
      const took = await switches(shape, [1, 0, 1, 0]);
      for (const one of took)
        if (process.env.SWITCH_OUT) appendFileSync(process.env.SWITCH_OUT, `${line(shape, one)}\n`);

      expect(took).toHaveLength(4);
      for (const one of took) {
        held(one);
        expect(one.invoked).toBeLessThanOrEqual(COMMANDS);
      }
    }, 300_000);
  }

  it("waits on no more commands before the paint, however many panes the tab in front holds", async () => {
    // A pane's own reads are the ones that grow with a window: a tab split forty ways has forty
    // gauges, and a gauge's command is one the core answers on the window's own thread. Read in
    // the switch, that was a command per pane ahead of the paint. They are read behind it.
    const atOnce: number[] = [];
    for (const panes of [1, 20, 40]) {
      const shape = { projects: 2, chats: 2, workspaces: 2, panes };
      const took = await switches(shape, [1, 0]);
      for (const one of took) {
        if (process.env.SWITCH_OUT) appendFileSync(process.env.SWITCH_OUT, `${line(shape, one)}\n`);
        expect(one.panes).toBe(panes);
        held(one);
        atOnce.push(one.invokedAtOnce);
      }
      cleanup();
      clearMocks();
    }
    expect(new Set(atOnce).size).toBe(1);
  }, 600_000);
});

/** Holds one switch between two kept projects to every budget. */
function held(one: Switch): void {
  // The project is on screen when the press returns, with its chats and its pane.
  expect(one.paintedAtOnce).toBe(true);
  // No terminal is closed and none is started: the pane is the one it had.
  expect([one.panesStarted, one.panesStopped]).toEqual([0, 0]);
  expect(one.mounted).toBeLessThanOrEqual(MOUNTS);
  expect(one.unmounted).toBeLessThanOrEqual(MOUNTS);
  expect(one.invokedAtOnce).toBeLessThanOrEqual(COMMANDS_AT_ONCE);
  expect(one.commits).toBeLessThanOrEqual(COMMITS);
}
