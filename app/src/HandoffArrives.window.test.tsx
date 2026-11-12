// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { widenAsks } from "./test-inbox";
import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { openChipCard } from "./actionResult.testkit";
import type { OpenChat } from "./bindings";
import { onTheWire } from "./chatEvents.testkit";
import { aTabForEachChat } from "./laidOut.testkit";
import { showNotices } from "./test-strips";
import { forgetInboxOpen, inboxOpenAtLaunch } from "./test-inbox";

/**
 * **A chat another chat started is announced, never put on screen** (#4759, spec #4750 stories
 * 23 to 28): a handoff hangs a card on its workspace's chip, "<chat> arrived from <asking
 * chat>", with Go to it. The card takes no keyboard and leaves a dot when it is hidden. A task
 * is the asking chat's to read, so only one that can't report asks is announced, since it
 * can't say when it needs the person.
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
/** A second project the same window holds, whose chats are numbered on their own. */
const OTHER = "/home/dev/two";

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
function core({ alsoHolds }: { alsoHolds?: string } = {}) {
  /** Which project each layout the window reported was of. */
  const laidOut: string[] = [];
  const listeners = new Map<string, number[]>();
  const listed: Record<string, OpenChat[]> = { alpha: [chat(1, "alpha.1")], beta: [] };
  const kept: { key: string; kind: string; read: boolean }[] = [];
  /** What the asks registry lists. */
  const asks: unknown[] = [];
  /** The MCP servers each chat could not start, as the core keeps them. */
  const failed: Record<number, unknown[]> = {};
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    if (cmd === "plugin:event|listen") {
      const { event, handler } = given as unknown as { event: string; handler: number };
      listeners.set(event, [...(listeners.get(event) ?? []), handler]);
      return 1;
    }
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "planes_to_restore" && alsoHolds !== undefined)
      return { windows: [{ planes: [PLANE, alsoHolds], active: 0 }], dropped: [], gone: [] };
    if (cmd === "open_plane") return { plane: given.path, ask: null };
    if (cmd === "window_layout") {
      laidOut.push(String(given.plane));
      return null;
    }
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        workspaces: Object.entries(listed).map(([name, chats]) => ({
          name,
          path: `${PLANE}/workspaces/${name}`,
          vision: "",
          todos: [],
          // A copy, as each answer over the wire is: the window compares one read with the last.
          chats: chats.map((one) => ({ ...one })),
        })),
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
      };
    // Every chat open now, as the core answers a window that reads them again.
    if (cmd === "opened_chats") return Object.values(listed).flat();
    // The asking chat's tab, as its stored layout has it; a chat that arrives has none.
    if (cmd === "reopened_layout") return aTabForEachChat([chat(1, "alpha.1")], () => "alpha");
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
      for (const one of given.noted as { key: string; kind: string }[])
        if (!kept.some((was) => was.key === one.key)) kept.push({ ...one, read: false });
      return kept;
    }
    if (cmd === "inbox_updates") return kept;
    if (cmd === "asks_waiting") return { plane: PLANE, asks, with_asking_chat: [] };
    if (cmd === "mcp_failures") return failed[given.session as number] ?? [];
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
    cantReportAsks = false,
    unreported = null,
    byPerson = false,
  }: {
    session: number;
    label: string;
    workspace?: string;
    task?: boolean;
    /** The core's answer from its harness's card. */
    cantReportAsks?: boolean;
    /** What else its card lacks, which the chat says and which announces nothing. */
    unreported?: string | null;
    /** The person asked for it themselves, from the window: it is theirs, and not announced. */
    byPerson?: boolean;
  }) => {
    // The core's answer (`Chat::announced`), on the event and in the record alike.
    const announced = !byPerson && (!task || cantReportAsks);
    listed[workspace].push({
      ...chat(session, String(session)),
      label,
      in_front: false,
      harness: "codex",
      unreported,
      cant_report_asks: cantReportAsks ? true : null,
      unseen: announced ? true : null,
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
            cant_report_asks: cantReportAsks,
            announced,
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
      // And the board says it waits on the person, as its hook does, in the core's batch of
      // chat events (#4746).
      const [event, payload] = onTheWire("chat-moved", {
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
      });
      for (const handler of listeners.get(event) ?? [])
        (
          window as unknown as {
            __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
          }
        ).__TAURI_INTERNALS__.runCallback(handler, { event, id: 1, payload });
    });
  };
  /** The core says what chat `session`'s sandbox blocked, which nothing here can allow. */
  const blocked = (session: number) => {
    act(() => {
      for (const handler of listeners.get("chat-sandbox-blocked") ?? [])
        (
          window as unknown as {
            __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
          }
        ).__TAURI_INTERNALS__.runCallback(handler, {
          event: "chat-sandbox-blocked",
          id: 1,
          payload: {
            plane: PLANE,
            session,
            operation: "write",
            kind: "path",
            ours: false,
            harness: "claude",
            said: "a write to a path outside its folders",
            offer: "none",
            target: null,
            levels: [],
            route: "",
            choose: null,
          },
        });
    });
  };
  /** The core holds that chat `session`'s MCP server `server` could not start. */
  const mcpFails = (session: number, server: string) => {
    failed[session] = [{ plane: PLANE, session, server, persona: null, error: "exit 1" }];
  };
  return { arrive, blocked, kept, laidOut, mcpFails, waitsInItsTerminal };
}

const opened = () =>
  render(
    <StrictMode>
      <App />
    </StrictMode>,
  );

const bar = () => screen.getByTestId("title-bar");
/** The card on `workspace`'s chip, opened by its dot's press: none comes by itself (#6004). */
const findCard = (workspace: string) => openChipCard(workspace);
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

  it("pops no card and takes no keyboard: a dot on the chip, and the chat being read stays on screen (#6004)", async () => {
    const { arrive } = core();
    opened();
    await ready();
    const before = document.activeElement;

    arrive({ session: 2, label: "fix the login" });

    await waitFor(() => expect(queryDot("alpha")).not.toBeNull());
    expect(queryCard("alpha")).toBeNull();
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

  it("goes to the chat in its own project, and moves nothing in another this window holds", async () => {
    const { arrive, laidOut } = core({ alsoHolds: OTHER });
    opened();
    await ready();
    arrive({ session: 2, label: "fix the login" });
    const card = await findCard("alpha");
    laidOut.length = 0;

    await userEvent.click(within(card).getByRole("button", { name: "Go to it" }));

    await waitFor(() => expect(screen.getByTestId("pane")).toHaveTextContent("session 2"));
    await waitFor(() => expect(laidOut).toContain(PLANE));
    expect(laidOut).not.toContain(OTHER);
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
      cantReportAsks: true,
    });

    const card = await findCard("alpha");
    expect(card).toHaveTextContent("check the queue arrived from alpha.1");
    expect(card).toHaveTextContent("It can't report asks, so look at it from time to time.");
    expect(within(card).getByRole("button", { name: "Go to it" })).toBeInTheDocument();
  });

  it("is not announced where its harness reports its asks, whatever else its card lacks", async () => {
    const { arrive, kept } = core();
    opened();
    await ready();

    arrive({
      session: 2,
      label: "check the queue",
      task: true,
      unreported: "opencode's helpers are not shown under it.",
    });

    await waitFor(() => expect(screen.getByTestId("pane")).toHaveTextContent("session 1"));
    expect(queryCard("alpha")).toBeNull();
    expect(kept).toEqual([]);
  });

  it("is not announced where the person asked for it themselves, though it can't report asks", async () => {
    const { arrive, kept } = core();
    opened();
    await ready();

    arrive({
      session: 2,
      label: "check the queue",
      task: true,
      cantReportAsks: true,
      byPerson: true,
    });

    await waitFor(() => expect(screen.getByTestId("pane")).toHaveTextContent("session 1"));
    expect(queryCard("alpha")).toBeNull();
    expect(queryDot("alpha")).toBeNull();
    expect(kept).toEqual([]);
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
    await widenAsks();
    const go = await screen.findByRole("button", { name: "Go to chat 2" });
    await userEvent.click(go);

    await waitFor(() => expect(screen.getByTestId("pane")).toHaveTextContent("session 2"));
  });
});

describe("what the Notices view says of a chat in no pane (#5217, #5649)", () => {
  // The Notices centre is listed in Updates since #5644: the window opens on it here.
  beforeEach(inboxOpenAtLaunch);
  afterEach(forgetInboxOpen);
  it("is a row: whose it is, its state, Go to chat, a ×, and Retry its one leading link", async () => {
    const { arrive, mcpFails } = core();
    mcpFails(2, "github");
    opened();
    await ready();
    arrive({ session: 2, label: "fix the login" });

    const said = await screen.findByText(/could not start, twice/, undefined, { timeout: 5000 });
    const card = said.closest('[data-scope="message"][data-part="root"]') as HTMLElement;
    const part = (name: string) =>
      card.querySelector(`[data-scope="message"][data-part="${name}"]`) as HTMLElement;
    expect(part("meta")).toHaveTextContent("Problem");
    expect(part("top")).not.toHaveTextContent("could not start");
    const links = within(part("links"));
    expect(links.getByRole("button", { name: /^Go to chat/ })).toHaveAttribute(
      "data-variant",
      "quiet-link",
    );
    within(part("dismiss")).getByRole("button", { name: "Dismiss" });
    expect(links.getByRole("button", { name: "Retry" })).toHaveAttribute("data-lead");
    expect(card.querySelectorAll("[data-lead]")).toHaveLength(1);
    expect(card.querySelector('[data-variant="primary"]')).toBeNull();

    // Go to chat shows it, and what it had to say goes to its band.
    await userEvent.click(links.getByRole("button", { name: /^Go to chat/ }));
    await waitFor(() => expect(screen.getByTestId("pane")).toHaveTextContent("session 2"));
  });

  it("keeps a sandbox block's full waiting edge, and says it waits only if it can be answered", async () => {
    const { arrive, blocked } = core();
    opened();
    await ready();
    arrive({ session: 2, label: "fix the login" });

    blocked(2);

    const said = await screen.findByText(/The sandbox blocked a write/, undefined, {
      timeout: 5000,
    });
    const card = said.closest('[data-scope="message"][data-part="root"]') as HTMLElement;
    expect(card).toHaveAttribute("data-consent");
    expect(card.querySelector('[data-part="meta"]')).toHaveTextContent(/^Problem/);
    expect(card.querySelector('[data-variant="primary"]')).toBeNull();
    within(card.querySelector('[data-part="links"]') as HTMLElement).getByRole("button", {
      name: /^Go to chat/,
    });
  });

  it("keeps an arrival among its Results, with Go to chat a quiet link among its links", async () => {
    const { arrive } = core();
    opened();
    await ready();
    arrive({ session: 2, label: "fix the login" });

    await showNotices();
    const results = await screen.findByRole("region", { name: "Results" }, { timeout: 5000 });
    const go = within(results).getByRole("button", { name: "Go to chat" });
    expect(go).toHaveAttribute("data-variant", "quiet-link");
    expect(go.closest('[data-scope="message"][data-part="links"]')).not.toBeNull();
    expect(results.querySelector('[data-variant="primary"]')).toBeNull();

    await userEvent.click(go);
    await waitFor(() => expect(screen.getByTestId("pane")).toHaveTextContent("session 2"));
  });
});
