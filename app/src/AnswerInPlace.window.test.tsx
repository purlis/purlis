// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { askRows } from "./test-inbox";
import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App from "./App";
import type { FinishedTask, InboxUpdate, Shown } from "./bindings";
import { aTabForEachChat } from "./laidOut.testkit";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";
import { forgetThisLaunch } from "./regions";
import { SETTLE_MS } from "./TaskBlocksNotice";
import { showUpdates } from "./test-strips";

/**
 * **The Answer link on an ask, and a result read in place** (#5651, spec #5638, ADR 0119
 * decisions 7 and 8), on the real window with the core mocked. An ask the window answers by what
 * is typed shows Answer (a decision's options stand on its row, #5998), which opens its form in the row with the keyboard in it and reads Cancel while
 * it is open; an ask only its own place can answer shows Go to chat alone. Answering clears
 * the ask through its source, and the ✋ drops with it. A line under Mine opens in place to
 * show its report or why it failed, and reading it, or going to its chat, marks it read.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const PLANE = "/home/dev/plane";
const AT = Math.floor(Date.now() / 1000) - 60;

const chat = (session: number) => ({
  session,
  name: `steward ${session}`,
  cwd: `${PLANE}/workspaces/alpha`,
  harness: null,
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
});

const PERMISSION: Shown = {
  session: 1,
  ask: "ask-1",
  says: "Run cargo test",
  options: [
    { id: "allow", label: "Allow once", allows: true },
    { id: "always", label: "Allow always", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: ["steward 1"],
  answer: { via: "hook" },
};

const QUESTION: Shown = {
  session: 2,
  ask: "question:2",
  says: "Waiting on your reply",
  options: [],
  source: "question",
  chain: ["steward 2"],
  answer: { via: "in-its-pane" },
  last_message: "Which branch?",
};

const TERMINAL: Shown = {
  session: 3,
  ask: "terminal:3",
  says: "Waiting in its terminal: a permission prompt",
  options: [],
  source: "terminal",
  chain: ["steward 3"],
  answer: { via: "in-its-pane" },
};

const BROWSER: Shown = {
  session: 4,
  ask: "browser-needs-you:4",
  says: "Its browser waits for you to sign in to github.com",
  options: [{ id: "take-over", label: "Take over", allows: false }],
  source: "browser-needs-you",
  chain: ["steward 4"],
  answer: { via: "take-over", program: "browser-1" },
};

const TASK_QUESTION: Shown = {
  session: 5,
  ask: "task-question:5:4",
  says: "Which host?",
  options: [],
  source: "task-question",
  chain: ["steward 1", "check prod"],
  answer: { via: "task-question", why: "no-reply", from: "steward 1", note: null },
};

const FAILED: FinishedTask = {
  id: "t1",
  asker: 1,
  name: "ship the docs",
  chat: null,
  persona: null,
  how: "failed",
  outcome: "failed",
  folds: false,
  report: "The docs build stopped: mdbook is not installed.",
  changed: null,
  ended: null,
  place: "alpha",
  branch: null,
  reopens: false,
  not_reopened: null,
  did_not_start: false,
  attempts: 0,
  waits: null,
};

const update = (key: string, says: string): InboxUpdate => ({
  key,
  kind: "task-failed",
  at: AT,
  session: null,
  chain: [],
  says,
  read: false,
});

type Asked = { cmd: string; args: Record<string, unknown> };

/** Chats 1 to 5 in alpha, holding `asks`; Mine lists one failed task, unread. */
function core(asks: readonly Shown[], events = false) {
  const chats = [1, 2, 3, 4, 5].map(chat);
  const asked: Asked[] = [];
  let held = [...asks];
  const updates = [update("task:t1", "ship the docs: failed")];
  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, unknown>;
      asked.push({ cmd, args: a });
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "asks_waiting") return { plane: PLANE, asks: held, with_asking_chat: [] };
      // An answer clears its ask at its source: the next read of the asks no longer lists it.
      if (cmd === "answer_ask") {
        held = held.filter((one) => one.ask !== a.ask);
        return null;
      }
      if (cmd === "send_input") {
        held = held.filter((one) => one.session !== a.session);
        return null;
      }
      if (cmd === "task_question") return { number: 4, question: "Which host?" };
      if (cmd === "answer_task_question") {
        held = held.filter((one) => one.session !== a.session);
        return null;
      }
      if (cmd === "inbox_updates" || cmd === "note_inbox_updates") return updates;
      if (cmd === "settle_inbox_updates") {
        const keys = a.keys as string[] | null;
        for (const one of updates) if (keys === null || keys.includes(one.key)) one.read = true;
        return updates;
      }
      if (cmd === "finished_tasks") return [FAILED];
      if (cmd === "opened_chats") return chats;
      if (cmd === "reopened_layout") return aTabForEachChat(chats, () => "alpha");
      if (cmd === "plane_pins")
        return { project: false, workspaces: ["alpha"], missing: [], order: ["alpha"] };
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: ["steward"],
          persona: "steward",
          unfiled: [],
          workspaces: [
            { name: "alpha", path: `${PLANE}/workspaces/alpha`, vision: "", todos: [], chats },
          ],
        };
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
      if (cmd === "extensions_on") return [];
      return null;
    },
    { shouldMockEvents: events },
  );
  return {
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args),
  };
}

beforeEach(() => {
  forgetThisLaunch();
  vi.useFakeTimers({ shouldAdvanceTime: true, toFake: ["setTimeout", "clearTimeout"] });
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
  vi.useRealTimers();
});

const press = userEvent.setup({ advanceTimers: (ms) => vi.advanceTimersByTime(ms) });

/** The person reads what was drawn for longer than an Allow waits for its row to settle. */
const read = () => new Promise((done) => setTimeout(done, SETTLE_MS + 50));

/** The row of chat `who`'s ask in Asks. */
const rowOf = (view: HTMLElement, who: string) =>
  askRows()
    .filter((one) => view.contains(one))
    .find(
      (one) =>
        one.classList.contains("inbox-ask") && one.querySelector(".inbox-who")?.textContent === who,
    ) as HTMLElement;

/** The hand on the title bar, by what it counts. */
const hand = (count: number) =>
  screen.queryAllByRole("button", {
    name: count === 1 ? "1 thing waits on you" : `${count} things wait on you`,
  });

async function opened(asks: readonly Shown[], events = false) {
  const one = core(asks, events);
  render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
  const view = await screen.findByRole("tabpanel", { name: "Asks" });
  await waitFor(() =>
    expect(view.querySelectorAll('[role="row"].inbox-ask')).toHaveLength(asks.length),
  );
  return { ...one, view };
}

describe("the Answer link on an ask (#5651)", () => {
  it("shows Answer, closed, on an ask answered by typing, and no form until it is pressed", async () => {
    const { view } = await opened([QUESTION]);

    const row = rowOf(view, "steward 2");
    const answer = within(row).getByRole("button", { name: "Answer steward 2" });
    expect(answer).toHaveTextContent("Answer");
    expect(answer).toHaveAttribute("aria-expanded", "false");
    expect(within(row).queryByRole("textbox")).toBeNull();
  });

  it("draws a permission's options on its row, with no Answer (#5998)", async () => {
    const { view } = await opened([PERMISSION]);
    const row = rowOf(view, "steward 1");

    const answers = within(row).getByRole("group", { name: "Answers to steward 1" });
    expect(
      within(answers)
        .getAllByRole("button")
        .map((one) => one.textContent),
    ).toEqual(["Allow once", "Other scopes…", "Deny"]);
    expect(within(row).queryByRole("button", { name: /^Answer/ })).toBeNull();
  });

  it("answers a permission from its options, and the hand's count drops at once", async () => {
    const { view, asked } = await opened([PERMISSION, QUESTION]);
    await waitFor(() => expect(hand(2).length).toBeGreaterThan(0));
    const row = rowOf(view, "steward 1");

    await read();
    await press.click(within(row).getByRole("button", { name: "Deny" }));

    await waitFor(() =>
      expect(asked("answer_ask")).toEqual([expect.objectContaining({ option: "deny" })]),
    );
    await waitFor(() => expect(hand(1).length).toBeGreaterThan(0));
    expect(view.querySelectorAll('[role="row"].inbox-ask')).toHaveLength(1);
  });

  it("opens a reply box with the keyboard in it, sends on Enter, and the hand's count drops", async () => {
    const { view, asked } = await opened([PERMISSION, QUESTION]);
    await waitFor(() => expect(hand(2).length).toBeGreaterThan(0));
    const row = rowOf(view, "steward 2");

    await press.click(within(row).getByRole("button", { name: "Answer steward 2" }));
    const box = within(row).getByRole("textbox", { name: "Reply to steward 2" });
    await waitFor(() => expect(box).toHaveFocus());
    await press.keyboard("main{Enter}");

    await waitFor(() =>
      expect(asked("send_input")).toEqual([expect.objectContaining({ session: 2 })]),
    );
    await waitFor(() => expect(hand(1).length).toBeGreaterThan(0));
  });

  it("opens a task's question in the row, with the keyboard in its box, and Enter sends the answer", async () => {
    const { view, asked } = await opened([TASK_QUESTION]);
    const row = rowOf(view, "steward 1 › check prod");

    await press.click(within(row).getByRole("button", { name: "Answer steward 1 › check prod" }));
    const box = await within(row).findByRole("textbox", { name: "Your answer to check prod" });
    await waitFor(() => expect(box).toHaveFocus());
    expect(screen.queryByRole("dialog")).toBeNull();
    await press.keyboard("prod-2{Enter}");

    await waitFor(() =>
      expect(asked("answer_task_question")).toEqual([
        expect.objectContaining({ session: 5, number: 4, text: "prod-2" }),
      ]),
    );
  });

  it("shows no Answer on a terminal's prompt and a browser's sign-in, only the chat's own actions (#6013)", async () => {
    const { view } = await opened([TERMINAL, BROWSER]);

    for (const who of ["steward 3", "steward 4"]) {
      const row = rowOf(view, who);
      expect(
        within(row)
          .getAllByRole("button")
          .map((one) => one.textContent)
          .filter((said) => said !== ""),
      ).toEqual(["Go to chat", "Snooze", "Stop"]);
    }
  });

  it("works by keyboard alone: → to Answer, Enter opens, Enter in the box sends, and Cancel closes (#3123)", async () => {
    const { view, asked } = await opened([QUESTION]);
    const row = rowOf(view, "steward 2");
    row.focus();

    // Ruling B (#3123): a row's own controls are reached by →.
    await press.keyboard("{ArrowRight}");
    const answer = within(row).getByRole("button", { name: "Answer steward 2" });
    expect(answer).toHaveFocus();
    await press.keyboard("{Enter}");
    const box = within(row).getByRole("textbox", { name: "Reply to steward 2" });
    await waitFor(() => expect(box).toHaveFocus());

    // Back to the link, which now reads Cancel, and Enter closes the form: Esc gives the
    // keyboard back to the row, whose first control it is.
    await press.keyboard("{Escape}");
    await waitFor(() => expect(row).toHaveFocus());
    await press.keyboard("{ArrowRight}");
    const cancel = within(row).getByRole("button", { name: "Cancel answering steward 2" });
    expect(cancel).toHaveFocus();
    await press.keyboard("{Enter}");
    expect(within(row).queryByRole("textbox")).toBeNull();
    expect(within(row).getByRole("button", { name: "Answer steward 2" })).toHaveFocus();

    // Opened again, a reply typed and sent with Enter alone.
    await press.keyboard("{Enter}");
    await waitFor(() =>
      expect(within(row).getByRole("textbox", { name: "Reply to steward 2" })).toHaveFocus(),
    );
    await press.keyboard("main{Enter}");
    await waitFor(() => expect(asked("send_input")).toHaveLength(1));
  });
});

describe("a Mine line read in place (#5651)", () => {
  /** The bell's count on the Updates icon, by what it says. */
  const bell = () => document.body.textContent?.includes("1 unread update from your chats");

  it("opens in place to show why it failed, marks it read and lowers the bell, and Hide closes it", async () => {
    const { asked } = await opened([]);
    const view = await showUpdates();
    const row = await within(view).findByRole("row", { name: "ship the docs: failed" });
    await waitFor(() => expect(bell()).toBe(true));
    expect(within(row).queryByText(FAILED.report)).toBeNull();

    const open = within(row).getByRole("button", { name: "Read ship the docs: failed" });
    expect(open).toHaveAttribute("aria-expanded", "false");
    await press.click(open);

    expect(within(row).getByText(FAILED.report)).toBeVisible();
    expect(open).toHaveAttribute("aria-expanded", "true");
    expect(open).toHaveTextContent("Hide");
    await waitFor(() =>
      expect(asked("settle_inbox_updates")).toEqual([
        expect.objectContaining({ keys: ["task:t1"], how: "read" }),
      ]),
    );
    await waitFor(() => expect(bell()).toBe(false));

    await press.click(open);
    expect(within(row).queryByText(FAILED.report)).toBeNull();
    // Read once is read: closing it settles nothing more.
    expect(asked("settle_inbox_updates")).toHaveLength(1);
  });

  it("opens by keyboard alone, from the line to Read by → and Enter (#3123)", async () => {
    await opened([]);
    const view = await showUpdates();
    const row = await within(view).findByRole("row", { name: "ship the docs: failed" });
    row.focus();

    await press.keyboard("{ArrowRight}");
    expect(within(row).getByRole("button", { name: "Read ship the docs: failed" })).toHaveFocus();
    await press.keyboard("{Enter}");

    expect(within(row).getByText(FAILED.report)).toBeVisible();
  });

  it("marks it read on Go to chat, and lowers the bell", async () => {
    const { asked } = await opened([]);
    const view = await showUpdates();
    const row = await within(view).findByRole("row", { name: "ship the docs: failed" });
    await waitFor(() => expect(bell()).toBe(true));

    await act(() => press.click(within(row).getByRole("button", { name: /^Go to chat/ })));

    await waitFor(() =>
      expect(asked("settle_inbox_updates")).toEqual([
        expect.objectContaining({ keys: ["task:t1"], how: "read" }),
      ]),
    );
    await waitFor(() => expect(bell()).toBe(false));
  });
});

describe("a notification about a task, clicked (#6005)", () => {
  it("opens Updates with the task's line read in place, and marks it read", async () => {
    const { asked } = await opened([], true);
    // The window is behind another app when its notification is clicked.
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    await act(async () => {
      await emit("asks-notified", { plane: PLANE, session: 1, sessions: [1], update: "task:t1" });
    });
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });

    const view = await screen.findByRole("tabpanel", { name: "Updates" });
    const row = await within(view).findByRole("row", { name: "ship the docs: failed" });
    await waitFor(() => expect(within(row).getByText(FAILED.report)).toBeVisible());
    expect(within(row).getByRole("button", { name: "Hide ship the docs: failed" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    await waitFor(() =>
      expect(asked("settle_inbox_updates")).toEqual([
        expect.objectContaining({ keys: ["task:t1"], how: "read" }),
      ]),
    );
  });
});
