// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";
import { chatMoves } from "./projectSwitch.testkit";
import { keepTheirOrder } from "./commandOrder";
import { countReactsCommits } from "./reactCommits.testkit";

keepTheirOrder();

/**
 * **A busy chat's moves do not draw the window again** (#4727, spec #4724).
 *
 * A harness at work moves its chat many times a second, and every move used to reach the window's
 * root: each project's view reported when its newest chat had moved (`PlaneReport.moved`), and a
 * new report is a new `reports` in `App`, with every memo keyed on it run again and the title bar
 * and the switchers drawn again. Nothing has read that time since the project strip went (ADR
 * 0054, amended 2026-10-23), so a move that changes nothing else the window shows reaches only
 * what shows that chat.
 *
 * Held by counts, which are the same on every machine (ADR 0086).
 */

countReactsCommits();

vi.mock("./SessionPane", async () => ({
  SessionPane: (await import("./countedPane.testkit")).CountedPane,
}));

afterEach(() => {
  cleanup();
  clearMocks();
});

const SHAPE = { projects: 2, chats: 4, workspaces: 2 };

describe("a busy chat's moves", () => {
  for (const [where, of] of [
    ["in the project in front", 0],
    ["in a project behind it", 1],
  ] as const) {
    it(`draw the window's root no more, ${where}`, async () => {
      const cost = await chatMoves(SHAPE, { of, times: 20 });
      expect(cost.windowDrawn).toBe(0);
      // The one thing they ask the core is the project's asks, read again once the burst has
      // settled, which is also what says the moves reached the window at all.
      expect(cost.invokes).toEqual({ asks_waiting: 1 });
    }, 120_000);
  }
});
