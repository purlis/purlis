// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { OpenChat } from "./bindings";

/**
 * **A chat another chat started is announced, never put on screen** (#4759, spec #4750 stories
 * 23 to 28): a handoff hangs a card on its workspace's chip, "<chat> arrived from <asking
 * chat>", with Go to it. The card takes no keyboard and leaves a dot when it is hidden. A task
 * is the asking chat's to read, so only one on a harness purlis has no hooks into is announced,
 * since it can't say when it needs the person.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/one";

function chat(session: number, name: string): OpenChat {
  return {
    session,
    name,
    cwd: `${PLANE}/workspaces/alpha`,
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

/** One project with `alpha` pinned, its chat `alpha.1` in front, and `beta` unpinned; a chat
 *  that arrives is listed in its workspace, as the core files it. */
function core() {
  const listeners = new Map<string, number[]>();
  const listed: Record<string, OpenChat[]> = { alpha: [chat(1, "alpha.1")], beta: [] };
  const kept: { key: string; kind: string; read: boolean }[] = [];
  /** What the asks registry lists. */
  const asks: unknown[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    if (cmd === "plugin:event|listen") {
      const { event, handler } = given as unknown as { event: string; handler: number };
      listeners.set(event, [...(listeners.get(event) ?? []), handler]);
      return 1;
    }
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        workspaces: Object.entries(listed).map(([name, chats]) => ({
          name,
          path: `${PLANE}/workspaces/${name}`,
          vision: "",
          todos: [],
          chats,
        })),
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
      };
    // Every chat open now, as the core answers a window that reads them again.
    if (cmd === "opened_chats") return Object.values(listed).flat();
    if (cmd === "plane_pins")
      return {
        project: false,
        workspaces: ["alpha"],
        missing: [],
        order: ["alpha"],
        certain: true,
      };
    // The Inbox's updates, kept as the core keeps them: one per key, unread.
    if (cmd === "note_inbox_updates") {
      for (const one of given.noted as { key: string }[])
        if (!kept.some((was) => was.key === one.key)) kept.push({ ...one, read: false });
      return kept;
    }
    if (cmd === "inbox_updates") return kept;
    if (cmd === "asks_waiting") return { plane: PLANE, asks, with_asking_chat: [] };
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
    return null;
  });
  /** The core says chat `session` arrived in `workspace`, started by `alpha.1`. */
  const arrive = ({
    session,
    label,
    workspace = "alpha",
    task = false,
    unreported = null,
  }: {
    session: number;
    label: string;
    workspace?: string;
    task?: boolean;
    unreported?: string | null;
  }) => {
    listed[workspace].push({
      ...chat(session, String(session)),
      label,
      in_front: false,
      harness: "codex",
      unreported,
      // The core marks it unseen where it announces it (`Chat::announced`).
      unseen: !task || unreported !== null ? true : null,
      from: {
        name: "alpha.1",
        workspace: "alpha",
        chat: 1,
        task,
        tab: !task,
        reported: false,
        unreported: false,
      },
    });
    const handlers = listeners.get("handoff-arrived") ?? [];
    if (handlers.length === 0) throw new Error("the window is not listening for arrivals");
    act(() => {
      for (const handler of handlers)
        (
          window as unknown as {
            __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
          }
        ).__TAURI_INTERNALS__.runCallback(handler, {
          event: "handoff-arrived",
          id: 1,
          payload: {
            plane: PLANE,
            session,
            name: String(session),
            label,
            workspace,
            persona: null,
            harness: "codex",
            unreported,
            from: { name: "alpha.1", workspace: "alpha", chat: 1, task, tab: !task },
          },
        });
      // And the sidebar's answer moved, as the core says when it files a chat it started.
      for (const handler of listeners.get("plane-changed") ?? [])
        (
          window as unknown as {
            __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
          }
        ).__TAURI_INTERNALS__.runCallback(handler, {
          event: "plane-changed",
          id: 1,
          payload: {
            plane: PLANE,
            changes: [{ kind: "chats", workspace: null, persona: null, path: "" }],
            answers: [{ answer: "sidebar" }],
          },
        });
    });
  };
  /** The asks registry says chat `session` waits on a prompt in its own terminal, which purlis
   *  can see and not answer (`AskSource::Terminal`). */
  const waitsInItsTerminal = (session: number) => {
    asks.push({
      session,
      ask: `terminal:${session}`,
      says: "a permission prompt",
      options: [],
      source: "terminal",
      chain: [String(session)],
      answer: { via: "in-its-pane" },
    });
    act(() => {
      for (const handler of listeners.get("asks-changed") ?? [])
        (
          window as unknown as {
            __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
          }
        ).__TAURI_INTERNALS__.runCallback(handler, {
          event: "asks-changed",
          id: 1,
          payload: { plane: PLANE, asks },
        });
      // And the board says it waits on the person, as its hook does.
      for (const handler of listeners.get("chat-moved") ?? [])
        (
          window as unknown as {
            __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
          }
        ).__TAURI_INTERNALS__.runCallback(handler, {
          event: "chat-moved",
          id: 1,
          payload: {
            plane: PLANE,
            session,
            state: "waiting",
            needs_you: true,
            queue: [session],
            moved_at: 7,
            sequence: 7,
            reports: [],
            refusals: [],
            children: [],
            needs: null,
            stopped: null,
          },
        });
    });
  };
  return { arrive, kept, waitsInItsTerminal };
}

const opened = () =>
  render(
    <StrictMode>
      <App />
    </StrictMode>,
  );

const bar = () => screen.getByTestId("title-bar");
const findCard = (workspace: string) =>
  screen.findByRole("region", { name: `Notices for ${workspace}` });
const queryCard = (workspace: string) =>
  screen.queryByRole("region", { name: `Notices for ${workspace}` });
const queryDot = (workspace: string) =>
  within(bar()).queryByRole("button", { name: new RegExp(`^Show the notice on ${workspace}`) });
/** Waits until the window has drawn its chat and listens for arrivals. */
const ready = () =>
  waitFor(() => expect(screen.getByTestId("pane")).toHaveTextContent("session 1"));

describe("a handoff's arrival card (#4759)", () => {
  it("hangs from its workspace's chip, says who started it, and offers Go to it", async () => {
    const { arrive } = core();
    opened();
    await ready();

    arrive({ session: 2, label: "fix the login" });

    const card = await findCard("alpha");
    expect(card).toHaveTextContent("fix the login arrived from alpha.1");
    expect(within(card).getByRole("button", { name: "Go to it" })).toBeInTheDocument();
  });

  it("never takes the keyboard, and the chat being read stays on screen", async () => {
    const { arrive } = core();
    opened();
    await ready();
    const before = document.activeElement;

    arrive({ session: 2, label: "fix the login" });

    const card = await findCard("alpha");
    expect(card.contains(document.activeElement)).toBe(false);
    expect(document.activeElement).toBe(before);
    expect(screen.getByTestId("pane")).toHaveTextContent("session 1");
  });

  it("hides on a press elsewhere and leaves a dot on the chip", async () => {
    const { arrive } = core();
    opened();
    await ready();
    arrive({ session: 2, label: "fix the login" });
    await findCard("alpha");

    await userEvent.click(document.body);

    await waitFor(() => expect(queryCard("alpha")).toBeNull());
    expect(queryDot("alpha")).not.toBeNull();
  });

  it("shows the chat that arrived when Go to it is pressed", async () => {
    const { arrive } = core();
    opened();
    await ready();
    arrive({ session: 2, label: "fix the login" });
    const card = await findCard("alpha");

    await userEvent.click(within(card).getByRole("button", { name: "Go to it" }));

    await waitFor(() => expect(screen.getByTestId("pane")).toHaveTextContent("session 2"));
  });
});

describe("a handoff's Update in the Inbox (#4759)", () => {
  it("is noted, and the ✋ counts nothing for it: a handoff asks nothing", async () => {
    const { arrive, kept } = core();
    opened();
    await ready();

    arrive({ session: 2, label: "fix the login" });

    await waitFor(() =>
      expect(kept.map((one) => [one.key, one.kind])).toEqual([["arrived:2", "arrived"]]),
    );
    expect(screen.queryByTestId("needs-you-button")).toBeNull();
  });
});

describe("a task's arrival (#4759)", () => {
  it("is not announced where its harness reports asks: the asking chat reads it", async () => {
    const { arrive } = core();
    opened();
    await ready();

    arrive({ session: 2, label: "check the queue", task: true });

    // Give the window its turn, then look: nothing hangs from the chip.
    await waitFor(() => expect(screen.getByTestId("pane")).toHaveTextContent("session 1"));
    expect(queryCard("alpha")).toBeNull();
    expect(queryDot("alpha")).toBeNull();
  });

  it("is announced like a handoff where its harness can't report asks, and says so", async () => {
    const { arrive } = core();
    opened();
    await ready();

    arrive({
      session: 2,
      label: "check the queue",
      task: true,
      unreported: "Codex can't tell purlis when it is waiting.",
    });

    const card = await findCard("alpha");
    expect(card).toHaveTextContent("check the queue arrived from alpha.1");
    expect(card).toHaveTextContent("It can't report asks, so look at it from time to time.");
    expect(within(card).getByRole("button", { name: "Go to it" })).toBeInTheDocument();
  });
});

describe("an ask from a chat in no pane (#4759)", () => {
  it("never takes a pane, and the Inbox offers Go to chat for a prompt in its terminal", async () => {
    const { arrive, waitsInItsTerminal } = core();
    opened();
    await ready();
    arrive({ session: 2, label: "fix the login" });

    waitsInItsTerminal(2);

    // The ✋ counts it, and the chat being read stays on screen.
    const hand = await screen.findByTestId("needs-you-button", undefined, { timeout: 5000 });
    expect(screen.getByTestId("pane")).toHaveTextContent("session 1");
    await userEvent.click(hand);
    const go = await screen.findByRole("button", { name: /^Go to chat/ });
    await userEvent.click(go);

    await waitFor(() => expect(screen.getByTestId("pane")).toHaveTextContent("session 2"));
  });
});
