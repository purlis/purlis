// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";
import { switches } from "./projectSwitch.testkit";
import { keepTheirOrder } from "./commandOrder";
import { countReactsCommits } from "./reactCommits.testkit";

keepTheirOrder();

/**
 * **A project view drawn again with no skill run changed draws no skill-run chip again** (#4728,
 * part of #4724).
 *
 * Each tab's `SkillRunChip` is a `memo` that reads the project's open runs from `SkillRunsHere`.
 * While that context's value was a new object on every render of the project view, every chip of
 * every tab was drawn again with it, on a switch and on each chat that moved, though no run had.
 * Held by counts, in the real window, as `projectSwitch.cost.test.tsx` holds a switch (ADR 0086).
 */

countReactsCommits();

vi.mock("./SessionPane", async () => ({
  SessionPane: (await import("./countedPane.testkit")).CountedPane,
}));

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("what a project view's render costs the skill-run chips", () => {
  it("draws no skill-run chip again when the view is drawn again and no run changed", async () => {
    const took = await switches({ projects: 2, chats: 10, workspaces: 2 }, [1, 0]);

    expect(took).toHaveLength(2);
    // The project view was drawn again: a switch brings it to the front.
    for (const one of took) expect(one.renderedBy.PlaneView ?? 0).toBeGreaterThan(0);
    expect(took.map((one) => one.renderedBy.SkillRunChip ?? 0)).toEqual([0, 0]);
  }, 300_000);
});
