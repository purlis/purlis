// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render as renderBare, screen, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { ProjectReady } from "./bindings";
import { aTabForEachChat } from "./laidOut.testkit";
import { forgetThisLaunch } from "./regions";
import { queryStripNamed } from "./test-strips";
import { themeInForce } from "./theme/inForce.testkit";

/**
 * **The startup promise, held at a launch that restores three projects** (#5326, the spec #5319,
 * stories 13 and 14): the window answers within 100 ms of appearing, with every project it is
 * restoring drawn on the rail and marked loading, and the project in front has its chats usable
 * within 3 s of launch, whatever the other projects and the slow panels are doing.
 *
 * **On a modelled clock.** The core here answers on Vitest's fake clock, each command after
 * what it took at the launch measured on 2026-11-05 (three projects, eight chats, about sixty
 * branch folders): the branch folder list 21.5 s, the other slow panels up to 3.4 s each, and
 * one project that took 20 s to open while the others took about a second. The window's own
 * work costs nothing on that clock, so what is held is what the window waits on, the same on
 * every machine. What its own work costs in frames is the release run's (ADR 0086, L11), since
 * jsdom's milliseconds are not a window's. The 200 ms stall, the window's thread held, and the
 * 100 ms answer of a project already open are held in the core, where both happen
 * (`src-tauri/src/off_the_main_thread/tests/startup.rs`).
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

declare global {
  interface Window {
    __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
  }
}

/** The window may take this long from appearing to answer (story 13), in ms. */
const FIRST_ANSWER_MS = 100;
/** The project in front has its chats usable this long after launch (story 14), in ms. */
const FRONT_USABLE_MS = 3_000;

/** How long each of the slow reads took at the launch measured on 2026-11-05, in ms. */
const MEASURED: Record<string, number> = {
  worktree_list: 21_500,
  workspace_repos: 3_400,
  plane_saving: 3_400,
  alerts_everywhere: 3_400,
  plane_doctor: 3_400,
  stale_clones: 3_400,
  sandbox_grants: 3_400,
  curation_offers: 3_400,
};

/** The three projects remembered from the last quit, in their order: `front` was in front, and
 *  `slow` (first) takes 20 s to open, as the slowest project did. */
const PROJECTS = [
  { name: "slow", opens: 20_000, session: 2 },
  { name: "other", opens: 1_000, session: 3 },
  { name: "front", opens: 600, session: 4 },
] as const;
const FRONT = 2;

/** How long after its open answers a project's chats are back (a chat starts in about 1 s). */
const CHATS_BACK_AFTER = 1_000;

const at = (name: string) => `/home/dev/${name}`;
const workspaceOf = (plane: string) => `${plane}/workspaces/alpha`;

function chatOf(project: (typeof PROJECTS)[number]) {
  return {
    session: project.session,
    name: `claude ${project.session}`,
    workspace: "alpha",
    cwd: workspaceOf(at(project.name)),
    in_front: true,
    resumed: false,
    fresh: false,
    profile: "claude",
    persona: null,
    unreported: null,
    guessed: null,
    pinned: false,
    label: null,
    from: null,
  };
}

const projectAt = (plane: string | undefined) =>
  PROJECTS.find((project) => at(project.name) === plane);

/** `value` once `ms` have gone by on the fake clock. */
const after = <T,>(ms: number, value: T) =>
  new Promise<T>((resolve) => setTimeout(() => resolve(value), ms));

/** A core that answers the window as the measured launch did, on the fake clock. */
function measuredCore() {
  const listeners = new Map<string, number[]>();
  const tell = (event: string, payload: unknown) => {
    for (const handler of listeners.get(event) ?? [])
      window.__TAURI_INTERNALS__.runCallback(handler, { event, id: 1, payload });
  };
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    if (cmd === "plugin:event|listen") {
      const { event, handler } = given as unknown as { event: string; handler: number };
      listeners.set(event, [...(listeners.get(event) ?? []), handler]);
      return 1;
    }
    const plane = given.plane as string | undefined;
    if (cmd === "plane_at_launch") return { plane: null, from: null, why: null };
    if (cmd === "planes_to_restore")
      return {
        windows: [{ planes: PROJECTS.map((project) => at(project.name)), active: FRONT }],
        dropped: [],
        gone: [],
      };
    if (cmd === "open_plane") {
      const path = given.path as string;
      const project = projectAt(path);
      if (project === undefined) return null;
      // Its chats come back a moment after it is held, and the core says so (#5322).
      setTimeout(
        () =>
          tell("project-ready", {
            plane: path,
            outcome: { kind: "ready", back: 1, wanted: 1 },
          } satisfies ProjectReady),
        project.opens + CHATS_BACK_AFTER,
      );
      return after(project.opens, { plane: path, ask: null });
    }
    if (cmd in MEASURED) {
      const slow = MEASURED[cmd];
      if (cmd === "worktree_list") return after(slow, []);
      if (cmd === "workspace_repos")
        return after(slow, { workspace: given.workspace, repos: [], cache_refused: null });
      if (cmd === "alerts_everywhere")
        return after(
          slow,
          PROJECTS.map((project) => ({ plane: at(project.name), alerts: [], stopped: null })),
        );
      return after(slow, null);
    }
    const project = projectAt(plane);
    if (cmd === "opened_chats") return project === undefined ? [] : [chatOf(project)];
    if (cmd === "reopened_layout")
      return project === undefined ? null : aTabForEachChat([chatOf(project)], () => "alpha");
    if (cmd === "plane_sidebar")
      return {
        root: plane,
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
        workspaces:
          project === undefined
            ? []
            : [
                {
                  name: "alpha",
                  path: workspaceOf(at(project.name)),
                  vision: "",
                  todos: [],
                  chats: [chatOf(project)],
                },
              ],
      };
    if (cmd === "workspace_panels")
      return {
        workspace: given.workspace,
        repos: [],
        paths: {},
        absent: [],
        refused: [],
        todos: [],
        todos_refused: null,
        personas: ["steward"],
        persona: "steward",
      };
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "owed_restarts") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
    if (cmd === "theme_in_force") return themeInForce(args);
    return null;
  });
}

/** The rail's projects in its order, `…` after each that is loading; none before it is drawn. */
const rows = () => {
  const rail = queryStripNamed("Projects");
  if (rail === null) return [];
  return within(rail)
    .queryAllByRole("tab")
    .map(
      (tab) =>
        `${tab.getAttribute("aria-label")}${tab.getAttribute("aria-busy") === "true" ? "…" : ""}`,
    );
};

/** Whether the project in front has its chat drawn and its row no longer loading. */
const frontUsable = () =>
  screen.queryByTestId("pane")?.textContent === `session ${PROJECTS[FRONT].session}` &&
  rows().includes(PROJECTS[FRONT].name);

/**
 * **How long on the fake clock until `done` holds**, stepping it `step` ms at a time, inside
 * `act` so each step's answers are committed before `done` reads the window; `undefined` if it
 * still does not hold at `until`.
 */
async function msUntil(done: () => boolean, until: number, step: number) {
  for (let now = 0; now <= until; now += step) {
    if (done()) return now;
    await act(() => vi.advanceTimersByTimeAsync(step));
  }
  return done() ? until : undefined;
}

beforeEach(() => {
  vi.useFakeTimers();
  globalThis.localStorage.clear();
  forgetThisLaunch();
});
afterEach(() => {
  cleanup();
  clearMocks();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("the startup targets with three projects restoring (#5326)", () => {
  it("answers within 100 ms of appearing, with every project it is restoring drawn loading", async () => {
    measuredCore();
    render(<App />);

    const drawnAt = await msUntil(
      () => rows().join() === "slow…,other…,front…",
      FIRST_ANSWER_MS,
      10,
    );

    expect(drawnAt, `the rail said ${JSON.stringify(rows())}`).toBeDefined();
    expect(drawnAt).toBeLessThanOrEqual(FIRST_ANSWER_MS);
  });

  it("has the front project's chats usable within 3 s of launch, while one project takes 20 s", async () => {
    measuredCore();
    render(<App />);

    const usableAt = await msUntil(frontUsable, FRONT_USABLE_MS, 50);

    expect(usableAt, `the rail said ${JSON.stringify(rows())}`).toBeDefined();
    expect(usableAt).toBeLessThanOrEqual(FRONT_USABLE_MS);
    // And the slow project is still on its way, drawn as such: nothing waited for it.
    expect(rows()).toContain("slow…");
  });
});
