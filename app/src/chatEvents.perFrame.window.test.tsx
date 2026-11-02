// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Profiler } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { ChatDoing, ChatSkillRun, Moved, OpenChat } from "./bindings";
import type { State } from "./chatState";
import { stripNamed } from "./test-strips";
import { aFramePassed } from "./frame.testkit";

/**
 * **A burst of chat events is one commit per frame** (#4731, part of #4724).
 *
 * With many busy agents, `chat-moved`, `chat-doing` and `chat-skill-run` arrive dozens of times
 * a second, each as its own task. Applied as each arrived, every one was a commit of its own,
 * and a window behind on its commits is the slow window the operator felt. The chat stores now
 * take every event as it comes and tell their readers once a frame, so what arrives within one
 * frame is drawn in one commit, in the order it arrived.
 *
 * **Held by counts** (ADR 0086): the commits React's own `Profiler` reports for the window.
 */

vi.mock("./SessionPane", async () => ({
  SessionPane: (await import("./countedPane.testkit")).CountedPane,
}));

declare global {
  interface Window {
    __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
  }
}

const PLANE = "/home/dev/plane";
const CHATS = [1, 2, 3, 4, 5];

function chat(session: number): OpenChat {
  return {
    session,
    name: `ide.${session}`,
    cwd: `${PLANE}/workspaces/ide`,
    harness: "claude",
    in_front: session === 1,
    resumed: null,
    fresh: null,
    profile: null,
    persona: null,
    unreported: null,
    card: null,
    guessed: null,
    pinned: false,
    label: null,
    from: null,
  };
}

/** The core holding five chats, and the wire its events come down: each event is a task of its
 *  own, as each Tauri event is. */
function core(): (event: string, payload: unknown) => void {
  const listeners = new Map<string, number[]>();
  const open = CHATS.map(chat);
  mockIPC((cmd, args) => {
    if (cmd === "plugin:event|listen") {
      const { event, handler } = args as { event: string; handler: number };
      listeners.set(event, [...(listeners.get(event) ?? []), handler]);
      return 1;
    }
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "opened_chats") return open;
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        workspaces: [
          {
            name: "ide",
            path: `${PLANE}/workspaces/ide`,
            vision: "",
            todos: [],
            chats: open,
            colour: null,
            live: false,
          },
        ],
        personas: [],
        persona: null,
        unfiled: [],
      };
    if (cmd === "chat_states" || cmd === "chat_doings" || cmd === "chat_skill_runs") return [];
    if (cmd === "chats_that_would_not_start") return [];
    return null;
  });
  return (event, payload) => {
    const handlers = listeners.get(event);
    if (handlers === undefined) throw new Error(`the window is not listening for ${event}`);
    act(() => {
      for (const handler of handlers)
        window.__TAURI_INTERNALS__.runCallback(handler, { event, id: 1, payload });
    });
  };
}

/** The window's commits since it came to rest. */
let commits = 0;
let sequence = 0;

function moving(session: number, state: State, queue: number[] = []): Moved {
  sequence += 1;
  return {
    plane: PLANE,
    session,
    state,
    needs_you: queue.includes(session),
    queue,
    moved_at: sequence,
    sequence,
    reports: [],
    refusals: [],
    children: [],
    needs: null,
    stopped: null,
  };
}

function doing(session: number, kind: string): ChatDoing {
  sequence += 1;
  return {
    plane: PLANE,
    session,
    sequence,
    doing: { kind, name: null, count: 0, over: false },
  };
}

function running(session: number): ChatSkillRun {
  return {
    plane: PLANE,
    session,
    run: {
      skill: "tdd",
      name: "tdd",
      id: `run-${session}`,
      tool_calls: 1,
      tokens: null,
      level: null,
    },
  };
}

/** The state mark on each chat's tab, by its accessible name. */
const marks = () =>
  within(stripNamed("Tabs"))
    .getAllByRole("tab")
    .map((tab) => within(tab).queryByRole("img")?.getAttribute("aria-label") ?? "");

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("chat events arriving within one frame", () => {
  async function aWindowAtRest() {
    const send = core();
    render(
      <Profiler id="window" onRender={() => (commits += 1)}>
        <App />
      </Profiler>,
    );
    await waitFor(() => expect(marks()).toHaveLength(CHATS.length));
    // Every answer the first draw asked for has landed, so what is counted is the burst.
    for (let turn = 0; turn < 10; turn += 1) await act(async () => {});
    await aFramePassed();
    commits = 0;
    return send;
  }

  /** What one chat moving costs the window: the commit that draws it, and what that commit
   *  sets off. (Today two more, from `PlaneView` reporting the move up to `App`, which #4727
   *  takes away.) */
  async function whatOneMoveCosts() {
    const send = await aWindowAtRest();
    send("chat-moved", moving(1, "running"));
    await aFramePassed();
    const cost = commits;
    cleanup();
    clearMocks();
    return cost;
  }

  it("draw nothing until the frame", async () => {
    const send = await aWindowAtRest();

    send("chat-moved", moving(2, "running"));
    send("chat-doing", doing(2, "thinking"));
    send("chat-skill-run", running(2));

    expect(commits).toBe(0);
    await aFramePassed();
    expect(commits).toBeGreaterThan(0);
  });

  it("cost what one move costs, however many arrived", async () => {
    const one = await whatOneMoveCosts();
    const send = await aWindowAtRest();

    // Twenty events across five chats, each a task of its own as on the wire. Told as each
    // arrived, they were sixteen commits.
    for (const session of CHATS) {
      send("chat-moved", moving(session, "running"));
      send("chat-doing", doing(session, "thinking"));
      send("chat-skill-run", running(session));
      send("chat-doing", doing(session, "editing"));
    }
    await aFramePassed();

    expect(commits).toBe(one);
    expect(one).toBeLessThanOrEqual(3);
  });

  it("leave every chat where its last event put it, in the order they arrived", async () => {
    const send = await aWindowAtRest();

    // Chat 3 runs, asks for the person, then runs again; chat 4 runs then finishes.
    send("chat-moved", moving(3, "running"));
    send("chat-moved", moving(4, "running"));
    send("chat-moved", moving(3, "waiting", [3]));
    send("chat-moved", moving(4, "done"));
    send("chat-moved", moving(3, "running"));
    await aFramePassed();

    const shown = marks();
    expect(shown[2]).toMatch(/running/i);
    expect(shown[3]).toMatch(/done|finished/i);
    expect(shown[0]).not.toMatch(/running|done|finished/i);
  });
});
