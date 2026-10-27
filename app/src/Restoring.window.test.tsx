// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render as renderBare, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { NotStarted, OpenChat } from "./bindings";
import { forgetThisLaunch } from "./regions";
import { restoredSince, RESTORING_HAS_NO_CLOSE } from "./restoringChats";
import { tipOf } from "./ui/tooltip/tooltip.testkit";
import { findResult } from "./actionResult.testkit";

/**
 * **A launch's chats come back behind the window** (`restoring.rs`), against the whole window:
 * every chat the core has registered has its tab in its place at once, drawn as restoring, and
 * each becomes the chat as the core says it has started. One that does not start loses its tab
 * and is listed with the chats that did not, and holds up no other.
 *
 * The core here is a pretend one: `opened_chats` answers with the chats as they stand, some
 * marked `restoring`, and the test moves them on and sends `chat-restored` as the real one
 * does.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

function chat(session: number, more: Partial<OpenChat> = {}): OpenChat {
  return {
    session,
    name: String(session),
    cwd: ALPHA,
    harness: "claude",
    in_front: false,
    resumed: null,
    fresh: null,
    profile: "claude",
    persona: "steward",
    unreported: null,
    card: null,
    guessed: null,
    pinned: false,
    label: null,
    from: null,
    ...more,
  };
}

/** The chat as the core says it before it has started: what only a start settles is unsaid. */
const restoring = (session: number, more: Partial<OpenChat> = {}): OpenChat =>
  chat(session, { harness: null, restoring: { workspace: "alpha" }, ...more });

/** The core: `chats` as they stand, which the test moves on, and what did not start. */
function core(first: OpenChat[]) {
  const held = { chats: first, waiting: [] as NotStarted[] };
  const asked: string[] = [];
  mockIPC(
    (cmd) => {
      asked.push(cmd);
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return held.chats;
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: ["steward"],
          persona: "steward",
          unfiled: [],
          workspaces: [
            {
              name: "alpha",
              path: ALPHA,
              vision: "",
              todos: [],
              // The sidebar lists the chats that are open, never one still to start.
              chats: held.chats.filter((one) => one.restoring == null),
            },
          ],
        };
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return held.waiting;
      if (cmd === "running_sessions") return [];
      if (cmd === "stopping_chats") return [];
      if (cmd === "finished_tasks") return [];
      return null;
    },
    { shouldMockEvents: true },
  );
  return {
    /** Chat `number` started: the core lists it as open, and says so. */
    async starts(number: number, more: Partial<OpenChat> = {}) {
      held.chats = held.chats.map((one) =>
        one.session === number ? chat(number, { in_front: one.in_front, ...more }) : one,
      );
      const left = held.chats.filter((one) => one.restoring != null).length;
      await act(() => emit("chat-restored", { plane: PLANE, number, session: number, left }));
    },
    /** Chat `number` did not start: it is no longer listed, and waits with the reason. */
    async doesNotStart(number: number, why: string) {
      const gone = held.chats.find((one) => one.session === number);
      held.chats = held.chats.filter((one) => one.session !== number);
      held.waiting = [
        ...held.waiting,
        { id: `01K6CHAT${number}`, name: gone?.name ?? String(number), why, approval: null },
      ];
      const left = held.chats.filter((one) => one.restoring != null).length;
      await act(() => emit("chat-restored", { plane: PLANE, number, session: null, left }));
    },
    asked: (cmd: string) => asked.filter((one) => one === cmd).length,
  };
}

const tabs = () =>
  [...document.querySelectorAll<HTMLElement>('.tabs [role="tab"]')].map((tab) => ({
    name: tab.textContent,
    restoring: tab.closest(".tab")?.hasAttribute("data-restoring") ?? false,
  }));
const restoringTabs = () => tabs().filter((tab) => tab.restoring).length;

beforeEach(() => {
  globalThis.localStorage.clear();
  forgetThisLaunch();
});
afterEach(() => {
  cleanup();
  clearMocks();
});

describe("a launch's chats, coming back behind the window", () => {
  it("each has its tab in its place at once, and its pane says it is restoring", async () => {
    core([restoring(4, { in_front: true }), restoring(5), restoring(6)]);
    render(<App />);

    // Three tabs, in the record's order, before any chat has started.
    await waitFor(() => expect(tabs()).toHaveLength(3));
    expect(restoringTabs()).toBe(3);
    // The one in front draws no terminal: there is no program yet. It says what is happening.
    const pane = await screen.findByTestId("chat-restoring");
    expect(pane).toHaveAttribute("role", "status");
    expect(pane).toHaveTextContent("Restoring");
    expect(screen.queryByTestId("pane")).toBeNull();
    // Its tab says so too, where a pointer rests on it.
    expect(tipOf(document.querySelector('.tab[data-restoring] [role="tab"]'))).toEqual(
      "Restoring…",
    );
  });

  it("each becomes the chat as it starts, in the place its tab had", async () => {
    const the = core([restoring(4, { in_front: true }), restoring(5), restoring(6)]);
    render(<App />);
    await screen.findByTestId("chat-restoring");
    const before = tabs().map((tab) => tab.name);

    // The chat in front starts first: its terminal takes the place of the notice.
    await the.starts(4, { resumed: "0b2f6c1e-1111-4222-8333-444444444444" });
    expect(await screen.findByTestId("pane")).toHaveTextContent("session 4");
    expect(screen.queryByTestId("chat-restoring")).toBeNull();
    expect(restoringTabs()).toBe(2);

    await the.starts(6);
    await the.starts(5);
    await waitFor(() => expect(restoringTabs()).toBe(0));
    expect(tabs().map((tab) => tab.name)).toEqual(before);
  });

  it("one that does not start loses its tab, is listed with its reason, and holds up no other", async () => {
    const the = core([restoring(4, { in_front: true }), restoring(5), restoring(6)]);
    render(<App />);
    await waitFor(() => expect(tabs()).toHaveLength(3));

    await the.doesNotStart(5, "its folder is gone");
    await waitFor(() => expect(tabs()).toHaveLength(2));
    const notice = await waitFor(() => {
      const found = document.querySelector<HTMLElement>(
        '[data-cause="chat-did-not-start:01K6CHAT5"]',
      );
      if (found === null) throw new Error("no Notice says chat 5 did not start");
      return found;
    });
    expect(notice).toHaveTextContent("did not start (its folder is gone)");

    // The others come back as they would have.
    await the.starts(4);
    await the.starts(6);
    await waitFor(() => expect(restoringTabs()).toBe(0));
    expect(tabs()).toHaveLength(2);
  });

  it("an event about another project's chat reads nothing of this one", async () => {
    const the = core([restoring(4, { in_front: true })]);
    render(<App />);
    await screen.findByTestId("chat-restoring");
    const reads = the.asked("opened_chats");

    await act(() =>
      emit("chat-restored", { plane: "/home/dev/other", number: 4, session: 4, left: 0 }),
    );
    expect(the.asked("opened_chats")).toBe(reads);
    expect(restoringTabs()).toBe(1);
  });

  it("is not ended by its close until it has started, and the press says why", async () => {
    const the = core([restoring(4, { in_front: true })]);
    render(<App />);
    await screen.findByTestId("chat-restoring");

    const close = [...document.querySelectorAll<HTMLElement>(".tabs .tab button")].find((one) =>
      (one.getAttribute("aria-label") ?? "").startsWith("End chat"),
    );
    if (close === undefined) throw new Error("the restoring tab draws no close");
    await userEvent.click(close);

    // Nothing was ended and nothing was asked: the tab is there, and the line says why.
    // The line, and not the close's own reason, kept for a screen reader as its description.
    await findResult(RESTORING_HAS_NO_CLOSE);
    expect(tabs()).toHaveLength(1);
    expect(the.asked("close_session")).toBe(0);
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });
});

describe("what a read says has become of the restoring chats", () => {
  const was = new Set([4, 5, 6]);
  const shown = () => false;

  it("tells the started from the ones still restoring and the ones that are gone", () => {
    const open = [chat(4), restoring(5)];
    const said = restoredSince(was, open, 4, shown);
    expect(said.started.map((one) => one.session)).toEqual([4]);
    expect([...said.still]).toEqual([5]);
    expect(said.gone).toEqual([6]);
    expect(said.arrived).toEqual([]);
  });

  it("takes a chat that started under another number as one that has just arrived", () => {
    const said = restoredSince(was, [chat(4), chat(5), chat(9)], 9, shown);
    expect(said.gone).toEqual([6]);
    expect(said.arrived.map((one) => one.session)).toEqual([9]);
    // And not twice: a chat a tab already shows is not drawn again.
    expect(restoredSince(was, [chat(9)], 9, () => true).arrived).toEqual([]);
  });
});
