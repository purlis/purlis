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
import {
  restoredSince,
  RESTORE_LOOKS_EVERY,
  RESTORING_HAS_NO_CLOSE,
  seenFirst,
} from "./restoringChats";
import { tipOf } from "./ui/tooltip/tooltip.testkit";
import { findResult } from "./actionResult.testkit";
import { aTabForEachChat } from "./laidOut.testkit";

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
      // Each chat the launch put back has its tab, as the stored layout keeps it (#4758), and a
      // pinned chat's tab is pinned: a layout says the pin on the tab.
      if (cmd === "reopened_layout") {
        const laid = aTabForEachChat(first, () => "alpha");
        return {
          ...laid,
          tabs: laid.tabs.map((tab, at) => ({ ...tab, pinned: first[at].pinned === true })),
        };
      }
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
    /** Chat `number` started, but under `session`: the core lists that in its place, and not
     *  `number`. Said at once unless `told` is false; then only {@link tell} says it. */
    async startsAs(
      number: number,
      session: number,
      more: Partial<OpenChat> = {},
      { told = true } = {},
    ) {
      held.chats = held.chats.map((one) =>
        one.session === number
          ? chat(session, { name: one.name, in_front: one.in_front, ...more })
          : one,
      );
      if (told) await this.tell(number, session);
    },
    /** The event the core sends for chat `number`, which started as `session`. */
    async tell(number: number, session: number | null) {
      const left = held.chats.filter((one) => one.restoring != null).length;
      await act(() => emit("chat-restored", { plane: PLANE, number, session, left }));
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
    pinned: tab.closest(".tab")?.querySelector(".pinned") != null,
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

  it("one that starts under a new number keeps its tab: its pin and its place (#4962)", async () => {
    const the = core([
      restoring(4, { in_front: true }),
      restoring(5, { name: "five", pinned: true }),
      restoring(6),
    ]);
    render(<App />);
    await waitFor(() => expect(tabs()).toHaveLength(3));
    await waitFor(() => expect(tabs().filter((tab) => tab.pinned)).toHaveLength(1));
    const before = tabs();
    const five = before.findIndex((tab) => tab.pinned);

    // The core has cleared the chat's own pin by now: the pin is the tab's.
    await the.startsAs(5, 9, { pinned: false });
    await waitFor(() => expect(restoringTabs()).toBe(2));
    expect(tabs()).toEqual(
      before.map((tab, at) => (at === five ? { ...tab, restoring: false } : tab)),
    );
  });

  it("one the list says started under a new number, before its event does, keeps its tab (#4962)", async () => {
    const the = core([
      restoring(4, { in_front: true }),
      restoring(5, { name: "five", pinned: true }),
      restoring(6),
    ]);
    render(<App />);
    await waitFor(() => expect(tabs().filter((tab) => tab.pinned)).toHaveLength(1));
    const before = tabs();
    const five = before.findIndex((tab) => tab.pinned);

    // Chat 5 has started as 9, but the read that sees it is the one chat 4's event asks for.
    await the.startsAs(5, 9, { pinned: false }, { told: false });
    await the.starts(4);
    const after = before.map((tab, at) =>
      at === five || tab.name === "steward 4" ? { ...tab, restoring: false } : tab,
    );
    await waitFor(() => expect(tabs()).toEqual(after));

    // Its own event, heard last, changes nothing.
    await the.tell(5, 9);
    expect(tabs()).toEqual(after);
  });

  it("one the window's own look finds under a new number, before any event, keeps its tab (#4962)", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    try {
      const the = core([
        restoring(4, { in_front: true }),
        restoring(5, { name: "five", pinned: true }),
        restoring(6),
      ]);
      render(<App />);
      await waitFor(() => expect(tabs().filter((tab) => tab.pinned)).toHaveLength(1));
      const before = tabs();
      const five = before.findIndex((tab) => tab.pinned);

      // No event yet: the look the window takes every few seconds is the first to see it.
      await the.startsAs(5, 9, { pinned: false }, { told: false });
      await act(() => vi.advanceTimersByTimeAsync(RESTORE_LOOKS_EVERY));
      const after = before.map((tab, at) => (at === five ? { ...tab, restoring: false } : tab));
      await waitFor(() => expect(tabs()).toEqual(after));

      await the.tell(5, 9);
      expect(tabs()).toEqual(after);
    } finally {
      vi.useRealTimers();
    }
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

  it("is not closed until it has started, and the press says why", async () => {
    const the = core([restoring(4, { in_front: true })]);
    render(<App />);
    await screen.findByTestId("chat-restoring");

    // Its tab's close, which ends nothing (#4756) and still waits for the chat to start.
    const close = [...document.querySelectorAll<HTMLElement>(".tabs .tab button")].find((one) =>
      (one.getAttribute("aria-label") ?? "").startsWith("Close "),
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
    const said = restoredSince(was, open, { number: 6, session: null }, shown);
    expect(said.started.map((one) => one.session)).toEqual([4]);
    expect([...said.still]).toEqual([5]);
    expect(said.gone).toEqual([6]);
    expect(said.arrived).toEqual([]);
  });

  it("holds a chat a read finds missing, drawn as restoring, until its event says what it became", () => {
    const first = restoredSince(was, [chat(4), restoring(5)], null, shown);
    expect(first.gone).toEqual([]);
    expect([...first.still].sort()).toEqual([5, 6]);

    // Its event says it started as 9: the same chat, in its tab.
    const then = restoredSince(
      first.still,
      [chat(4), restoring(5), chat(9)],
      { number: 6, session: 9 },
      shown,
      first.seen,
    );
    expect(then.renumbered.map(({ from, chat }) => [from, chat.session])).toEqual([[6, 9]]);
    expect(then.gone).toEqual([]);
    // Even where a read listed 9 already without telling it was 6's.
    const listed9 = restoredSince(
      first.still,
      [chat(4), restoring(5), chat(9)],
      null,
      shown,
      first.seen,
    );
    expect(listed9.renumbered).toEqual([]);
    const late = restoredSince(
      listed9.still,
      [chat(4), restoring(5), chat(9)],
      { number: 6, session: 9 },
      shown,
      listed9.seen,
    );
    expect(late.renumbered.map(({ from, chat }) => [from, chat.session])).toEqual([[6, 9]]);
    // Or that it did not start.
    const not = restoredSince(
      first.still,
      [chat(4), restoring(5)],
      { number: 6, session: null },
      shown,
      first.seen,
    );
    expect(not.gone).toEqual([6]);
  });

  it("takes a session in a missing chat's place on the list as that chat, before its event", () => {
    const seen = seenFirst([restoring(4), restoring(5), restoring(6)]);
    const said = restoredSince(was, [restoring(4), chat(9), restoring(6)], null, shown, seen);
    expect(said.renumbered.map(({ from, chat }) => [from, chat.session])).toEqual([[5, 9]]);
    expect(said.gone).toEqual([]);
    // Not a session a tab already shows, nor one the read before listed.
    const shows = restoredSince(was, [restoring(4), chat(9), restoring(6)], null, () => true, seen);
    expect(shows.renumbered).toEqual([]);
    expect([...shows.still].sort()).toEqual([4, 5, 6]);
  });

  it("takes no chat by its place where two neighbours went together", () => {
    // 5 did not start and 6 started as 9: the list says only that 9 is where 5 and 6 were.
    const seen = seenFirst([chat(4), restoring(5), restoring(6)]);
    const said = restoredSince(new Set([5, 6]), [chat(4), chat(9)], null, shown, seen);
    expect(said.renumbered).toEqual([]);
    expect(said.gone).toEqual([]);
    expect([...said.still].sort()).toEqual([5, 6]);
    // Their events say which is which.
    const nine = restoredSince(
      said.still,
      [chat(4), chat(9)],
      { number: 6, session: 9 },
      shown,
      said.seen,
    );
    expect(nine.renumbered.map(({ from, chat }) => [from, chat.session])).toEqual([[6, 9]]);
  });

  it("never closes a held chat on reads another chat's event asks for", () => {
    let now = restoredSince(
      new Set([5, 6]),
      [chat(4), restoring(5)],
      { number: 4, session: 4 },
      shown,
    );
    for (let read = 0; read < 3; read += 1)
      now = restoredSince(
        now.still,
        [chat(4), chat(5)],
        { number: 5, session: 5 },
        shown,
        now.seen,
      );
    expect(now.gone).toEqual([]);
    expect([...now.still]).toEqual([6]);
  });

  it("takes a held chat whose event never comes as gone, once nothing else is restoring", () => {
    const first = restoredSince(new Set([5, 6]), [chat(4), restoring(5)], null, shown);
    expect([...first.still].sort()).toEqual([5, 6]);
    // Found missing again, but 5 is restoring still: its event may yet come.
    const second = restoredSince(first.still, [chat(4), restoring(5)], null, shown, first.seen);
    expect(second.gone).toEqual([]);
    // 5 has started and nothing else is restoring: a look of the window's own takes 6 as one
    // that did not start, where an event's read does not.
    const open = [chat(4), chat(5)];
    expect(
      restoredSince(second.still, open, { number: 5, session: 5 }, shown, second.seen).gone,
    ).toEqual([]);
    const third = restoredSince(second.still, open, null, shown, second.seen);
    expect(third.gone).toEqual([6]);
    expect([...third.still]).toEqual([]);
  });

  it("takes a chat that started under another number as the same chat, in its tab", () => {
    const said = restoredSince(was, [chat(4), chat(5), chat(9)], { number: 6, session: 9 }, shown);
    expect(said.gone).toEqual([]);
    expect(said.arrived).toEqual([]);
    expect(said.renumbered.map(({ from, chat }) => [from, chat.session])).toEqual([[6, 9]]);
  });

  it("takes one whose old tab is gone already as one that has just arrived", () => {
    // An earlier read found 6 gone and closed its tab, before the event said what became of it.
    const said = restoredSince(
      new Set([4, 5]),
      [chat(4), chat(5), chat(9)],
      { number: 6, session: 9 },
      shown,
    );
    expect(said.renumbered).toEqual([]);
    expect(said.arrived.map((one) => one.session)).toEqual([9]);
    // And not twice: a chat a tab already shows is not drawn again.
    expect(restoredSince(was, [chat(9)], { number: 6, session: 9 }, () => true).arrived).toEqual(
      [],
    );
  });

  it("never draws a chat as restoring again from an answer older than one already read", () => {
    // #4739: `opened_chats` answers off the window's thread, so two asks a launch makes close
    // together can be answered out of order. Chat 4 started in the answer read first; the
    // older one landing after it still lists 4 as restoring.
    const after = restoredSince(
      was,
      [chat(4), restoring(5), restoring(6)],
      { number: 4, session: 4 },
      shown,
    ).still;
    const older = restoredSince(after, [restoring(4), restoring(5), restoring(6)], null, shown);
    expect([...older.still]).toEqual([5, 6]);
    expect(older.started).toEqual([]);
    expect(older.gone).toEqual([]);
  });
});
