// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { Moved, SlowMoment } from "./bindings";
import { forgetDismissals } from "./dismissals";
import { forgetThisLaunch } from "./regions";
import { hotChats, watchForStalls } from "./slowLog";

/**
 * **A stall in the window is written with what the window was doing** (#4725, `slowLog.ts`):
 * how many chats were working and which tab was in front, read from the real window, so a
 * stall the person felt can be matched to what was on screen.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

let written: SlowMoment[] = [];

/** Chat `session` is in its turn, as the core first says. */
function running(session: number): Moved {
  return {
    plane: PLANE,
    session,
    state: "running",
    needs_you: false,
    queue: [],
    moved_at: 1,
    sequence: 1,
    reports: [],
    refusals: [],
    children: [],
    needs: null,
    stopped: null,
  };
}

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

const CHAT = {
  session: 4,
  name: "claude 4",
  cwd: `${ALPHA}/svc`,
  harness: "claude",
  in_front: true,
  resumed: null,
  fresh: null,
  profile: "claude",
  persona: null,
  unreported: null,
  guessed: null,
  pinned: false,
  label: null,
  from: null,
};

/** A task chat 4 dispatched, with no tab. */
const TASK = {
  ...CHAT,
  session: 6,
  name: "devops 6",
  in_front: false,
  persona: "devops",
  label: "talk",
  from: {
    chat: 4,
    name: "claude 4",
    workspace: "alpha",
    task: true,
    tab: false,
    reported: false,
    unreported: false,
  },
};

function core() {
  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, unknown>;
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [CHAT];
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: ["steward", "devops"],
          persona: "steward",
          unfiled: [],
          workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats: [CHAT, TASK] }],
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
          personas: ["steward", "devops"],
          persona: "steward",
        };
      if (cmd === "workspace_repos")
        return { workspace: a.workspace, repos: [], cache_refused: null };
      if (cmd === "worktree_list") return [];
      if (cmd === "chat_states") return [running(CHAT.session)];
      if (cmd === "record_slow") written.push(a.moment as SlowMoment);
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "owed_restarts") return [];
      if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
      return null;
    },
    { shouldMockEvents: true },
  );
}

beforeEach(() => {
  written = [];
  globalThis.localStorage.clear();
  forgetThisLaunch();
});
afterEach(() => {
  cleanup();
  clearMocks();
  forgetDismissals();
});

describe("a stall in the window", () => {
  it("is written with the chats working and the tab in front", async () => {
    core();
    render(<App />);
    await screen.findByTestId("pane");
    await screen.findByRole("tab", { name: /claude 4/ });
    await waitFor(() => expect(hotChats()).toBe(1));

    let next: ((time: number) => void) | undefined;
    const stop = watchForStalls(
      (callback) => {
        next = callback;
      },
      () => true,
    );
    next?.(1_000);
    next?.(1_450);
    stop();

    await waitFor(() =>
      expect(written).toMatchObject([
        { kind: "stall", ms: 450, hot: 1, front: "claude claude 4", opening: false },
      ]),
    );
  });
});
