// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";
import type { PlaneAlerts, PlaneSaving } from "./bindings";
import { aWindow, rootOf, settled, type Answers, type Asked } from "./projectSwitch.testkit";
import { keepTheirOrder } from "./commandOrder";
import { countReactsCommits, renderedBy } from "./reactCommits.testkit";
import { SAVING_REREAD_MS } from "./saving";

keepTheirOrder();

/**
 * **An answer equal to the one a project view holds draws no project view again** (felt speed,
 * round 2).
 *
 * Each project's view (`PlaneView`) is held (`memo`) so the window redrawing is not every
 * project redrawing (SC-3). It was drawn again anyway, every project at once, each time the
 * window read the git standings: the save standing, the alerts, the stale clones and the chats
 * on old instructions each set a fresh object for an answer that said what the last one said,
 * and the alerts were handed to every project as the window's one reading, so one project's
 * alert moving drew every project. And every project read its save standing on the same 10 s
 * beat, in one tick, the ones behind the project in front among them.
 *
 * Held by counts, the same on every machine (ADR 0086).
 */

countReactsCommits();

vi.mock("./SessionPane", async () => ({
  SessionPane: (await import("./countedPane.testkit")).CountedPane,
}));

/** The window's beats, held here and run by the test (`beat`), never on the wall clock. */
const beats = vi.hoisted(() => new Map<number, Set<() => void>>());
vi.mock("./whileShown", async (actual) => ({
  ...(await actual<typeof import("./whileShown")>()),
  everyWhileShown: (ms: number, read: () => void) => {
    const readers = beats.get(ms) ?? new Set<() => void>();
    beats.set(ms, readers);
    const reader = () => read();
    readers.add(reader);
    return () => readers.delete(reader);
  },
}));

/** Runs every read on the beat of `ms` once, as its tick would. */
async function beat(ms: number): Promise<void> {
  await act(async () => {
    for (const read of [...(beats.get(ms) ?? [])]) read();
  });
}

afterEach(() => {
  cleanup();
  clearMocks();
  beats.clear();
});

const SHAPE = { projects: 3, chats: 2, workspaces: 1 };

const STANDING: PlaneSaving = {
  stage: "changed",
  changed: ["a.md"],
  ahead: 0,
  pr: null,
  request: "pull request",
  blocked: null,
  branch: "main",
  pushes: true,
  behind: 0,
  pushFailed: null,
  live: [],
  conflicts: [],
  conflictKeys: {},
  notice: null,
  mode: "push",
  modeFrom: "charter.toml",
  journal: [],
};

/** One alert about `plane`, saying `detail`. */
const anAlert = (plane: string, detail: string): PlaneAlerts => ({
  plane,
  alerts: [
    {
      severity: "warn",
      subject: "charter",
      detail,
      workspaces: [],
      way: { kind: "settings", group: "project", setting: null },
    },
  ],
  stopped: null,
});

/** A core that answers every git standing, each ask with a new object saying the same. */
function answers(): { answers: Answers; alertOfLast: { detail: string } } {
  const alertOfLast = { detail: "The pin is old" };
  const last = rootOf(SHAPE.projects - 1);
  return {
    alertOfLast,
    answers: {
      plane_saving: () => structuredClone(STANDING),
      alerts_everywhere: () =>
        Array.from({ length: SHAPE.projects }, (_, n) =>
          rootOf(n) === last
            ? anAlert(last, alertOfLast.detail)
            : { plane: rootOf(n), alerts: [], stopped: null },
        ),
      stale_clones: () => ({ stale: [], unread: [] }),
      chats_plane_updated: () => [],
    },
  };
}

const asksOf = (asked: readonly Asked[], from: number, cmd: string) =>
  asked.slice(from).filter((one) => one.cmd === cmd);

const planeViewsDrawn = () => renderedBy.get("PlaneView") ?? 0;

describe("what draws a project view again", () => {
  it("draws none for a git standing equal to the one each holds", async () => {
    const core = answers();
    const { asked, planeChanged } = await aWindow(SHAPE, core.answers);
    const from = asked.length;
    renderedBy.clear();

    for (let n = 0; n < SHAPE.projects; n++) await planeChanged(rootOf(n), "git");
    await settled(asked);

    // Each was asked again and answered as before.
    expect(asksOf(asked, from, "plane_saving").length).toBeGreaterThanOrEqual(SHAPE.projects);
    expect(asksOf(asked, from, "alerts_everywhere").length).toBeGreaterThanOrEqual(1);
    expect(asksOf(asked, from, "stale_clones").length).toBeGreaterThanOrEqual(SHAPE.projects);
    expect(planeViewsDrawn()).toBe(0);
  });

  it("draws only the project whose alert moved, when another project's alert says something new", async () => {
    const core = answers();
    const { asked, planeChanged } = await aWindow(SHAPE, core.answers);
    const from = asked.length;
    renderedBy.clear();

    // The last project's one alert now says something else: still one alert, still a warning,
    // which is all the other projects' Inboxes say of it.
    core.alertOfLast.detail = "The pin is older still";
    await planeChanged(rootOf(SHAPE.projects - 1), "git");
    await settled(asked);

    expect(asksOf(asked, from, "alerts_everywhere").length).toBeGreaterThanOrEqual(1);
    expect(planeViewsDrawn()).toBe(1);
  });

  it("reads the save standing of the project in front alone on the 10 s beat, and draws nothing for the same answer", async () => {
    const core = answers();
    const { asked } = await aWindow(SHAPE, core.answers);
    await settled(asked);
    const from = asked.length;
    renderedBy.clear();

    await beat(SAVING_REREAD_MS);
    await settled(asked);

    expect(asksOf(asked, from, "plane_saving").map((one) => one.plane)).toEqual([rootOf(0)]);
    expect(planeViewsDrawn()).toBe(0);
  });
});
