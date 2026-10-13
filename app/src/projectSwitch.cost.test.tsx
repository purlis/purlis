// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { appendFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";
import { switches, type Shape, type Switch } from "./projectSwitch.testkit";

/**
 * **A switch between two projects the person is working in builds nothing** (`projectShown.tsx`).
 *
 * The operator's report was a freeze of one to three seconds on every switch, worse the more
 * chats were open. Counted here in the real window, a switch then mounted about 800 components
 * and 15 more for each chat of the project it showed, unmounted as many of the one it hid, sent
 * the core some fifty commands, and closed one terminal to start another. With the view of a
 * project kept while another is in front, it mounts the gear on the project's tab and nothing
 * else, starts no terminal, and sends eight commands before it is painted, which say which
 * project is in front, and five after, which read again what the view held while it was away.
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

/** The most a switch between two projects kept drawn may mount or unmount. It mounts 3 now, the
 *  gear on the tab of the project in front; built again, a view of twenty chats mounts 1,099. */
const MOUNTS = 60;
/** The most commands such a switch may send the core before the press returns, which is before
 *  the project is painted. It sends 8 now, which say which project is in front and whose Inbox
 *  is open; a view built again sent 49, all of them then. */
const COMMANDS_AT_ONCE = 12;
/** The most it may send in all, with what it reads again once it is painted. It sends 13. */
const COMMANDS = 24;
/** The most times React may commit for it, the reads' answers among them. It commits 4 times
 *  now, one of them after the paint, and did 5 before it. */
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
        // The project is on screen when the press returns, with its chats and its pane.
        expect(one.paintedAtOnce).toBe(true);
        // No terminal is closed and none is started: the pane is the one it had.
        expect([one.panesStarted, one.panesStopped]).toEqual([0, 0]);
        expect(one.mounted).toBeLessThanOrEqual(MOUNTS);
        expect(one.unmounted).toBeLessThanOrEqual(MOUNTS);
        expect(one.invokedAtOnce).toBeLessThanOrEqual(COMMANDS_AT_ONCE);
        expect(one.invoked).toBeLessThanOrEqual(COMMANDS);
        expect(one.commits).toBeLessThanOrEqual(COMMITS);
      }
    }, 300_000);
  }
});
