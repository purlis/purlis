// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { PlaneSaving } from "./bindings";
import { aTabForEachChat } from "./laidOut.testkit";
import { inboxOpenAtLaunch } from "./test-inbox";
import { showTheRepos } from "./test-strips";
import { forgetThisLaunch } from "./regions";

/**
 * **A project is usable as soon as its chats are back** (#5325, the spec #5319): its slower
 * panels (branch folders, the health check, save status, stale clones, alerts) each load on
 * their own with their own mark, and none holds up the chats or another panel. The alerts of
 * every project are asked once the restore settles, not once per project that joins.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

const CHAT = {
  session: 4,
  name: "claude 4",
  workspace: "alpha",
  cwd: ALPHA,
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

const STANDING: PlaneSaving = {
  stage: "saved",
  changed: [],
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

/** What each slow panel's read answers, when the test lets it. */
const ANSWERS: Record<string, unknown> = {
  plane_saving: STANDING,
  stale_clones: { stale: [], unread: [] },
  plane_doctor: { rows: [], full: false, app_rows: [], path: "" },
  worktree_list: [],
  alerts_everywhere: [{ plane: PLANE, alerts: [], stopped: null }],
};

/** A one-project core whose slow panels answer only when the test says so. */
function slowCore() {
  const waiting = new Map<string, (answer: unknown) => void>();
  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, unknown>;
      if (cmd in ANSWERS)
        return new Promise((resolve) => waiting.set(cmd, resolve as (answer: unknown) => void));
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [CHAT];
      if (cmd === "reopened_layout") return aTabForEachChat([CHAT], () => "alpha");
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: ["steward"],
          persona: "steward",
          unfiled: [],
          workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats: [CHAT] }],
        };
      if (cmd === "workspace_panels")
        return {
          workspace: a.workspace,
          repos: ["svc"],
          paths: { svc: `${ALPHA}/svc` },
          absent: [],
          refused: [],
          todos: [],
          todos_refused: null,
          personas: ["steward"],
          persona: "steward",
        };
      if (cmd === "workspace_repos")
        return { workspace: a.workspace, repos: [], cache_refused: null };
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "owed_restarts") return [];
      return null;
    },
    { shouldMockEvents: true },
  );
  return {
    /** Lets `cmd`'s read answer: with what it would say, or with a refusal. */
    answer: async (cmd: string, refused = false) => {
      await waitFor(() => expect(waiting.has(cmd)).toBe(true));
      const landed = waiting.get(cmd) as (answer: unknown) => void;
      waiting.delete(cmd);
      await act(async () => landed(refused ? null : ANSWERS[cmd]));
    },
  };
}

const CHECKING_STALE = /Checking repos for a stale branch or address/;
const READING_ALERTS = /Reading the alerts/;

const saveMark = () =>
  within(screen.getByTestId("title-bar")).queryByRole("button", {
    name: "Saving: reading the project",
  });
const healthMark = () => screen.queryByRole("button", { name: "Doctor — checking" });
const branchesMark = () => screen.getByTestId("clone-svc");

beforeEach(() => {
  globalThis.localStorage.clear();
  forgetThisLaunch();
  inboxOpenAtLaunch();
});
afterEach(() => {
  cleanup();
  clearMocks();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("a project's slow panels (#5325)", () => {
  it("leaves the chats usable while every panel draws its own mark", async () => {
    slowCore();
    render(<App />);

    // The chat is back though nothing else has answered.
    expect(await screen.findByTestId("pane")).toHaveTextContent("session 4");

    await waitFor(() => expect(saveMark()).toBeInTheDocument());
    expect(healthMark()).toBeInTheDocument();
    expect(await screen.findByText(CHECKING_STALE)).toBeInTheDocument();
    expect(screen.getByText(READING_ALERTS)).toBeInTheDocument();
    await showTheRepos();
    await waitFor(() => expect(branchesMark()).toHaveAttribute("aria-busy", "true"));
  });

  it("clears each mark when its own read answers, whichever comes first", async () => {
    const { answer } = slowCore();
    render(<App />);
    await screen.findByTestId("pane");
    await showTheRepos();
    await screen.findByText(CHECKING_STALE);
    const marks = () => ({
      save: saveMark() !== null,
      health: healthMark() !== null,
      stale: screen.queryByText(CHECKING_STALE) !== null,
      alerts: screen.queryByText(READING_ALERTS) !== null,
      branches: branchesMark().getAttribute("aria-busy") === "true",
    });
    await waitFor(() =>
      expect(marks()).toEqual({
        save: true,
        health: true,
        stale: true,
        alerts: true,
        branches: true,
      }),
    );

    await answer("stale_clones");
    await waitFor(() => expect(marks().stale).toBe(false));
    expect(marks()).toEqual({
      save: true,
      health: true,
      stale: false,
      alerts: true,
      branches: true,
    });

    await answer("worktree_list");
    await waitFor(() => expect(marks().branches).toBe(false));
    expect(marks()).toMatchObject({ save: true, health: true, alerts: true });

    await answer("plane_saving");
    await waitFor(() => expect(marks().save).toBe(false));
    expect(marks()).toMatchObject({ health: true, alerts: true });

    await answer("plane_doctor");
    await waitFor(() => expect(marks().health).toBe(false));
    expect(marks()).toMatchObject({ alerts: true });

    await answer("alerts_everywhere");
    await waitFor(() => expect(marks().alerts).toBe(false));
  });

  it("stops marking a read that answered with nothing, and never spins on", async () => {
    const { answer } = slowCore();
    render(<App />);
    await screen.findByTestId("pane");
    await waitFor(() => expect(saveMark()).toBeInTheDocument());
    await screen.findByText(CHECKING_STALE);

    await answer("plane_saving", true);
    await answer("stale_clones", true);

    await waitFor(() => expect(saveMark()).toBeNull());
    await waitFor(() => expect(screen.queryByText(CHECKING_STALE)).toBeNull());
  });
});

describe("the alerts of a restore (#5325)", () => {
  const at = (name: string) => `/home/dev/${name}`;

  /** A core restoring `remembered`, each open answering when the test says so. */
  function restoringCore(remembered: string[]) {
    const opens = new Map<string, () => void>();
    let alerts = 0;
    mockIPC(
      (cmd, args) => {
        const given = (args ?? {}) as Record<string, unknown>;
        if (cmd === "alerts_everywhere") {
          alerts += 1;
          return [];
        }
        if (cmd === "plane_at_launch") return { plane: null, from: null, why: null };
        if (cmd === "planes_to_restore")
          return { windows: [{ planes: remembered.map(at), active: 0 }], dropped: [], gone: [] };
        if (cmd === "open_plane") {
          const path = given.path as string;
          return new Promise((resolve) =>
            opens.set(path, () => resolve({ plane: path, ask: null })),
          );
        }
        if (cmd === "plane_sidebar")
          return {
            root: given.plane,
            workspaces: [],
            personas: [],
            persona: null,
            unfiled: [],
          };
        if (cmd === "opened_chats") return [];
        if (cmd === "chat_states") return [];
        if (cmd === "chats_that_would_not_start") return [];
        if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
        if (cmd === "extensions_on") return [];
        return null;
      },
      { shouldMockEvents: true },
    );
    return {
      asked: () => alerts,
      answer: async (name: string) => {
        await waitFor(() => expect(opens.has(at(name))).toBe(true));
        await act(async () => (opens.get(at(name)) as () => void)());
      },
    };
  }

  it("asks once for a three-project restore, when the restore settles", async () => {
    const { asked, answer } = restoringCore(["one", "two", "three"]);
    render(<App />);

    await answer("one");
    await answer("two");
    // Two projects have joined, and the restore is not over.
    expect(asked()).toBe(0);
    await answer("three");

    await waitFor(() => expect(asked()).toBe(1));
    // Nothing is asked after it either: the projects' own answers do not ask again.
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });
    expect(asked()).toBe(1);
  });

  it("does not keep the alerts waiting on a project that never answers", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const { asked, answer } = restoringCore(["one", "two"]);
    render(<App />);

    await answer("one");
    expect(asked()).toBe(0);

    await act(async () => vi.advanceTimersByTime(15_000));
    await waitFor(() => expect(asked()).toBe(1));
  });
});
