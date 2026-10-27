// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  ALLOWED_LATELY,
  Inbox,
  MOVED_JUST_NOW,
  MOVED_ON,
  NOTICE_MOVED_JUST_NOW,
  NOT_CHECKED,
  NOTHING_WAITS,
  NOTICES,
  UPDATES,
  hiddenSaid,
  landOnGroup,
  MOST_LINES,
  type UpdateRow,
  type Updates,
} from "./Inbox";
import { Notice } from "./Notice";
import { IN_ANOTHER_WINDOW, OTHER_PROJECTS, type Elsewhere } from "./InboxElsewhere";
import type { Needing, Quiet } from "./NeedsYou";
import { SETTLE_MS } from "./TaskBlocksNotice";
import { DispatchGrantNotice } from "./DispatchGrantNotice";
import { forgetInbox, replyBytes } from "./inboxRules";
import { keepInboxScope } from "./inboxScope";
import { forgetProjectViews } from "./projectViews";
import type { DispatchPending, InboxUpdate, Shown } from "./bindings";

/**
 * **The Inbox** (#1692, spec #1688): a project's asks grouped by chat, oldest first, each under
 * its chain, answered in place through the path its source's Notice answers with, Go to chat on
 * every one, and the keys of I-11. Driven as the person drives it: what they read and press.
 */

// The clock alone is the test's: an Allow waits for its row to settle (`SETTLE_MS`), and every
// test but the ones about that wait reads the list for longer than that before it presses.
beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date", "performance"] });
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
  vi.useRealTimers();
});

/** The person reads what is drawn for longer than an Allow waits. */
const read = () => act(() => vi.advanceTimersByTime(SETTLE_MS + 1));

const PLANE = "/home/dev/plane";

const permission = (session: number, ask: string, chain: string[], says = "Run cargo test") =>
  ({
    session,
    ask,
    says,
    options: [
      { id: "allow", label: "Allow", allows: true },
      { id: "deny", label: "Deny", allows: false },
    ],
    source: "permission",
    chain,
    answer: { via: "hook" },
  }) satisfies Shown;

const deny = () => ({ id: "deny", label: "Deny", allows: false });

const HOST: Shown = {
  session: 5,
  ask: "block:5:connect:host:api.example.com",
  says: "The sandbox refused api.example.com",
  options: [
    { id: "chat", label: "Allow for this chat", allows: true },
    { id: "keep", label: "Keep blocked", allows: false },
  ],
  source: "sandbox-host",
  chain: ["steward 5"],
  answer: {
    via: "sandbox-block",
    shown: { operation: "connect", kind: "host", what: "host", target: "api.example.com" },
  },
};

const DISPATCH: Shown = {
  session: 4,
  ask: "dispatch:7",
  says: "Wants to hand a task to devops",
  options: [
    { id: "chat", label: "Allow for this chat", allows: true },
    { id: "keep", label: "Keep blocked", allows: false },
  ],
  source: "dispatch",
  chain: ["steward 4"],
  answer: { via: "dispatch", id: 7, shown: "s0" },
};

const HELD: DispatchPending = {
  plane: PLANE,
  id: 7,
  session: 4,
  chat: "steward 4",
  asking: "steward",
  target: "devops",
  brief: "Check why the prod deploy is red.",
  brief_cut: false,
  brief_lines: 1,
  levels: ["chat"],
  locked: null,
  never_unread: null,
  works_in: null,
  works_in_missing: false,
  allowed_in: [],
  works_with: "devops works with its own access: vault team.",
  also: [],
  shown: "s0",
  task: null,
  task_cut: false,
  profile: null,
  mode: "task",
};

const QUESTION: Shown = {
  session: 6,
  ask: "question:6",
  says: "Waiting on your reply",
  options: [],
  source: "question",
  chain: ["steward 6"],
  answer: { via: "in-its-pane" },
};

const TERMINAL: Shown = {
  session: 8,
  ask: "terminal:8",
  says: "Waiting in its terminal: a permission prompt",
  options: [],
  source: "terminal",
  chain: ["steward 8"],
  answer: { via: "in-its-pane" },
};

/** A core that records what the window sends and answers each command from `answers`. */
function core(answers: Record<string, unknown> = {}) {
  const calls: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args: args as Record<string, unknown> });
    if (cmd in answers) {
      const answer = answers[cmd];
      if (answer instanceof Error) throw answer.message;
      return typeof answer === "function" ? (answer as (a: unknown) => unknown)(args) : answer;
    }
    return null;
  });
  return calls;
}

function draw(asks: readonly Shown[] | undefined, more: Partial<Parameters<typeof Inbox>[0]> = {}) {
  const props = {
    plane: PLANE,
    asks,
    onGo: vi.fn(),
    onLeave: vi.fn(),
    onAnswered: vi.fn(),
    ...more,
  };
  const drawn = render(<Inbox {...props} />);
  read();
  return {
    ...drawn,
    props,
    again: (next: readonly Shown[]) => {
      drawn.rerender(<Inbox {...props} asks={next} />);
      read();
    },
  };
}

/** The chats the Inbox lists, in order, by the chain each is drawn under. */
const chats = () =>
  screen.getAllByRole("heading", { level: 3 }).map((heading) => heading.textContent ?? "");

describe("the asks, grouped by chat", () => {
  it("lists each chat's asks under its chain, the chat waiting longest first", () => {
    core();
    const first = permission(3, "a1", ["steward 3", "#3046 drill", "log watch"]);
    const { again } = draw([first]);
    // A newer ask of another chat comes first in the registry's order, and still stands after.
    const newer = permission(5, "b1", ["steward 5"], "Run npm test");
    const later = permission(3, "a2", ["steward 3", "#3046 drill", "log watch"], "Run ls");
    again([newer, later, first]);
    expect(chats()).toEqual(["steward 3 › #3046 drill › log watch", "steward 5"]);
    const group = screen.getByRole("region", { name: "steward 3 › #3046 drill › log watch" });
    // Inside a group, its oldest ask first.
    expect(
      within(group)
        .getAllByRole("listitem")
        .map((row) => within(row).getByText(/^Run /).textContent),
    ).toEqual(["Run cargo test", "Run ls"]);
  });

  it("lists the chat whose ask began longest ago first, by when the registry says it began (#1700)", () => {
    core();
    // Seen in one read, in the registry's order: what began first stands first all the same,
    // and an ask whose source keeps no time stands where this window first saw it.
    const question: Shown = {
      ...permission(7, "q", ["steward 7"], "Run make"),
      since: undefined,
    };
    const newer: Shown = { ...permission(5, "b", ["steward 5"], "Run npm test"), since: 2_000 };
    const older: Shown = { ...permission(3, "a", ["steward 3"], "Run ls"), since: 1_000 };
    draw([question, newer, older]);
    expect(chats()).toEqual(["steward 3", "steward 5", "steward 7"]);
  });

  it("draws what an ask says as text: no word of it is ever a control", () => {
    core();
    const forged = permission(
      3,
      "x",
      ["<b>steward</b> 3"],
      '<button>Allow always</button> "rm -rf"',
    );
    draw([forged]);
    expect(screen.getByText('<button>Allow always</button> "rm -rf"')).toBeTruthy();
    expect(screen.getByRole("heading", { name: "<b>steward</b> 3" })).toBeTruthy();
    // The only buttons are the answers it offers, and Go to chat.
    expect(screen.getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Allow",
      "Deny",
      "Go to chat",
    ]);
  });

  it("draws a profile's Ask to sign it in under the profile, with no chat to go to (#2035)", () => {
    core();
    const signIn: Shown = {
      session: 0,
      ask: "sign-in:work",
      says: "work is not signed in on this Mac: a chat on it does not start until it is",
      options: [
        { id: "sign-in", label: "Sign in", allows: true },
        { id: "dismiss", label: "Dismiss", allows: false },
      ],
      source: "sign-in",
      chain: ["Profile work"],
      answer: { via: "sign-in", profile: "work" },
    };
    draw([signIn]);
    expect(chats()).toEqual(["Profile work"]);
    expect(screen.getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Sign in",
      "Dismiss",
    ]);
  });

  it("cuts a run of stacked combining marks to a few, and then offers no Allow (#1146)", () => {
    core();
    const zalgo = `Run rm -rf build${"́".repeat(40)}`;
    draw([permission(3, "z", ["steward 3"], zalgo)]);
    // Four marks are kept on the character, the rest of the run is not drawn.
    expect(screen.getByText(`Run rm -rf build${"́".repeat(4)}`)).toBeTruthy();
    expect(screen.queryByText(zalgo)).toBeNull();
    // What is drawn is not what would run, so nothing that lets it run is offered here
    // (`hooked::shown_in_full`'s rule): Deny, and the chat's own pane, which shows it whole.
    expect(screen.getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Deny",
      "Go to chat",
    ]);
  });

  it("marks a no-break space in a command, and says why only Deny is offered (SK-51, #3202)", () => {
    core();
    // The core offers no Allow on it (`hooked::shown_in_full`), and writes the character as its
    // mark (`asking::said_inertly`): the window draws the mark apart and says what it costs.
    const said = "Run a command: git⟦U+00A0⟧commit -m wip";
    draw([{ ...permission(3, "nb", ["steward 3"], said), options: [deny()] }]);
    const mark = screen.getByText("⟦U+00A0⟧");
    expect(mark.tagName).toBe("MARK");
    expect(mark).toHaveClass("invisible-char");
    expect(screen.getByText(hiddenSaid("⟦U+00A0⟧"))).toBeTruthy();
    expect(screen.getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Deny",
      "Go to chat",
    ]);
  });

  it("marks a line break the command holds, and says why only Deny is offered (#3470)", () => {
    core();
    // A next line, a vertical tab or a line separator is not how a shell lays a line out: the
    // core keeps it in the command, marks it, and offers no Allow. The row says what it is.
    for (const mark of ["⟦U+0085⟧", "⟦U+000B⟧", "⟦U+2028⟧"]) {
      const { unmount } = draw([
        {
          ...permission(3, "lb", ["steward 3"], `Run a command: ls${mark}rm -rf x`),
          options: [deny()],
        },
      ]);
      expect(screen.getByText(mark).tagName).toBe("MARK");
      expect(
        screen.getByText(
          `Allow is not offered. It holds ${mark}, a character that breaks the line here.`,
        ),
      ).toBeTruthy();
      unmount();
    }
    expect(hiddenSaid("⟦U+00A0⟧")).toBe(
      "Allow is not offered. It holds ⟦U+00A0⟧, a character that looks like another one here.",
    );
  });

  it("names the mark that misleads most, and what each kind does (#3470)", () => {
    core();
    // A no-break space and a line separator: the separator, which can hide a second command.
    const said = "Run a command: git⟦U+00A0⟧commit⟦U+2028⟧rm -rf x";
    draw([{ ...permission(3, "lb", ["steward 3"], said), options: [deny()] }]);
    expect(
      screen.getByText(
        "Allow is not offered. It holds ⟦U+2028⟧, a character that breaks the line here.",
      ),
    ).toBeTruthy();
    expect(hiddenSaid("⟦U+000D⟧")).toBe(
      "Allow is not offered. It holds ⟦U+000D⟧, a character that breaks the line here.",
    );
    for (const mark of ["⟦U+202E⟧", "⟦U+001B⟧", "⟦U+200B⟧"]) {
      expect(hiddenSaid(mark)).toBe(
        `Allow is not offered. It holds ${mark}, a character a reader does not see here.`,
      );
    }
  });

  it("draws a command over several lines on them, and says why only Deny is offered (#3502)", () => {
    core();
    // The core puts `ls` and `rm -rf x` on one line, offers no Allow (`hooked::shown_in_full`),
    // and gives the lines as that line has them (`asking::lines_of`).
    const said = "Run a command: ls rm -rf x";
    draw([
      {
        ...permission(3, "ml", ["steward 3"], said),
        options: [deny()],
        lines: { count: 2, shown: ["Run a command: ls", "rm -rf x"] },
      },
    ]);
    const lines = Array.from(document.querySelectorAll(".ask-line"), (one) => one.textContent);
    expect(lines).toEqual(["Run a command: ls", "rm -rf x"]);
    expect(document.querySelector(".ask-lines")).not.toHaveClass("inbox-clamped");
    expect(screen.getByText("Allow is not offered. It runs over 2 lines.")).toBeTruthy();
    expect(screen.getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Deny",
      "Go to chat",
    ]);
  });

  it("draws the first few lines of a long command, and counts the rest (#3502)", () => {
    core();
    const shown = Array.from({ length: 9 }, (_, at) => `echo ${at}`);
    draw([
      {
        ...permission(3, "ml", ["steward 3"], shown.join(" ")),
        options: [deny()],
        lines: { count: 40, shown },
      },
    ]);
    const lines = Array.from(document.querySelectorAll(".ask-line"), (one) => one.textContent);
    expect(lines).toEqual([...shown.slice(0, MOST_LINES), `and ${40 - MOST_LINES} more lines`]);
    expect(screen.getByText("Allow is not offered. It runs over 40 lines.")).toBeTruthy();
  });

  it("says how many lines a command runs over where it cannot draw them (#3502)", () => {
    core();
    // A line masked whole: the core gives the count and no lines.
    draw([
      {
        ...permission(3, "ml", ["steward 3"], "[vault value]"),
        options: [deny()],
        lines: { count: 3, shown: [] },
      },
    ]);
    expect(document.querySelector(".ask-line")).toBeNull();
    expect(screen.getByText("[vault value]")).toBeTruthy();
    expect(
      screen.getByText("Allow is not offered. It runs over 3 lines, put on one here."),
    ).toBeTruthy();
  });

  it("says the lines over a mark, and never offers Allow it was not given (#3502)", () => {
    core();
    const said = "Run a command: git⟦U+00A0⟧commit rm -rf x";
    draw([
      {
        ...permission(3, "ml", ["steward 3"], said),
        lines: { count: 2, shown: ["Run a command: git⟦U+00A0⟧commit", "rm -rf x"] },
      },
    ]);
    // Where the core offered Allow, the row draws its line as it is and says no reason.
    expect(screen.queryByText(/Allow is not offered/)).toBeNull();
    cleanup();
    draw([
      {
        ...permission(3, "ml", ["steward 3"], said),
        options: [deny()],
        lines: { count: 2, shown: ["Run a command: git⟦U+00A0⟧commit", "rm -rf x"] },
      },
    ]);
    expect(screen.getByText("⟦U+00A0⟧").tagName).toBe("MARK");
    expect(screen.getByText("Allow is not offered. It runs over 2 lines.")).toBeTruthy();
  });

  it("says nothing of a ⟦ the command wrote itself, which keeps its Allow (SK-51, #3202)", () => {
    core();
    draw([permission(3, "lb", ["steward 3"], "Run a command: echo ⟦U+27E6⟧U+00A0⟧")]);
    expect(screen.queryByText(/Allow is not offered/)).toBeNull();
    expect(screen.getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Allow",
      "Deny",
      "Go to chat",
    ]);
  });

  it("draws a path in NFD as it is, with its Allow (#1146)", () => {
    core();
    const nfd = "Run cat /Users/dev/Tiếng Việt/café.md".normalize("NFD");
    draw([permission(3, "n", ["steward 3"], nfd)]);
    expect(screen.getByText(nfd)).toBeTruthy();
    expect(screen.getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Allow",
      "Deny",
      "Go to chat",
    ]);
  });

  it("says it is reading before the project's asks first arrive", () => {
    core();
    draw(undefined);
    expect(screen.getByText("Reading what waits on you…")).toBeTruthy();
    expect(screen.queryByText(NOTHING_WAITS)).toBeNull();
  });
});

describe("answering in place", () => {
  it("answers a permission on its chat's own hook, and lists it as recently answered once it goes", async () => {
    const calls = core();
    const ask = permission(3, "01J0A", ["steward 3"]);
    const { again } = draw([ask]);
    await userEvent.click(screen.getByRole("button", { name: "Allow" }));
    await waitFor(() =>
      expect(calls).toContainEqual({
        cmd: "answer_ask",
        args: { plane: PLANE, session: 3, ask: "01J0A", option: "allow" },
      }),
    );
    // The hook carried it out, so its source stops waiting and the registry drops it.
    again([]);
    expect(screen.getByText(NOTHING_WAITS)).toBeTruthy();
    const recent = screen.getByRole("region", { name: "Recently answered" });
    expect(within(recent).getByText("steward 3")).toBeTruthy();
    expect(within(recent).getByText("Run cargo test")).toBeTruthy();
    expect(within(recent).getByText("Allow")).toBeTruthy();
    // Read-only: nothing in it can be pressed.
    expect(within(recent).queryAllByRole("button")).toEqual([]);
  });

  it("opens Delete workspace from a workspace's removal, removes nothing, and notes no answer (qw356)", async () => {
    // A removal is for good, and a chat raised it: no press on a row among Allows removes
    // anything. Remove… opens the window's own dialog, and the ask waits on.
    const calls = core();
    const removal: Shown = {
      session: 6,
      ask: "removal:6:alpha",
      says: "Removing workspace alpha deletes its memory, its todos and its clones",
      options: [
        { id: "remove", label: "Remove…", allows: true },
        { id: "keep", label: "Keep", allows: false },
      ],
      source: "workspace-removal",
      chain: ["steward 6"],
      answer: { via: "workspace-removal", workspace: "alpha", memories: 2, todos: 0 },
    };
    const opened: unknown[] = [];
    const heard = (event: Event) => opened.push((event as CustomEvent).detail);
    window.addEventListener("purlis-remove-workspace-ask", heard);
    try {
      const { props, again } = draw([removal]);
      await userEvent.click(screen.getByRole("button", { name: "Remove…" }));
      await waitFor(() => expect(opened).toEqual([{ plane: PLANE, workspace: "alpha" }]));
      expect(calls.filter((one) => one.cmd !== "asks_waiting")).toEqual([]);
      expect(props.onAnswered).not.toHaveBeenCalled();
      // The person closed the dialog, and the question was answered elsewhere: nothing says
      // they answered Remove… here.
      again([]);
      expect(screen.queryByRole("region", { name: "Recently answered" })).toBeNull();
    } finally {
      window.removeEventListener("purlis-remove-workspace-ask", heard);
    }
  });

  it("offers every choice a harness gave, a question of several answers included", async () => {
    const calls = core();
    const choosing: Shown = {
      ...permission(3, "q1", ["steward 3"], "Which database should it use?"),
      options: [
        { id: "0", label: "Postgres", allows: true },
        { id: "1", label: "SQLite", allows: true },
        { id: "2", label: "Neither", allows: false },
      ],
    };
    draw([choosing]);
    await userEvent.click(screen.getByRole("button", { name: "SQLite" }));
    await waitFor(() =>
      expect(calls.find((one) => one.cmd === "answer_ask")?.args.option).toBe("1"),
    );
  });

  it("draws each Allow as a grant and its refusal plain, neither primary (ADR 0097 §6)", async () => {
    core();
    draw([permission(3, "p1", ["steward 3"]), HOST]);
    const answers = (who: string) =>
      within(screen.getByRole("group", { name: `Answers to ${who}` })).getAllByRole("button");
    for (const [who, allow, keep] of [
      ["steward 3", "Allow", "Deny"],
      ["steward 5", "Allow for this chat", "Keep blocked"],
    ]) {
      const [first, second] = answers(who);
      expect(first).toHaveAccessibleName(allow);
      expect(first).toHaveAttribute("data-variant", "grants");
      expect(first).not.toHaveAttribute("data-variant", "primary");
      expect(second).toHaveAccessibleName(keep);
      expect(second).toHaveAttribute("data-scope", "button");
      expect(second).not.toHaveAttribute("data-variant", "grants");
      expect(second).not.toHaveAttribute("data-variant", "primary");
    }
  });

  it("marks a persona's declared browser in its place, and answers only on a press (#2241)", async () => {
    const calls = core();
    const browser: Shown = {
      session: 8,
      ask: "browser:8",
      says: "Wants a browser: what may it reach? Its persona qa declares the open web too",
      options: [
        { id: "project", label: "This project's sites", allows: true },
        {
          id: "open-web",
          label: "Also the open web (qa uses this)",
          allows: true,
          preselected: true,
        },
        { id: "project-always", label: "This project's sites, always for qa", allows: true },
        { id: "open-web-always", label: "Also the open web, always for qa", allows: true },
        { id: "keep", label: "Keep off", allows: false },
      ],
      source: "browser",
      chain: ["qa 8"],
      answer: { via: "browser" },
    };
    draw([browser]);
    const answers = within(screen.getByRole("group", { name: "Answers to qa 8" })).getAllByRole(
      "button",
    );
    expect(answers.map((one) => one.textContent)).toEqual([
      "This project's sites",
      "Also the open web (qa uses this)",
      "This project's sites, always for qa",
      "Also the open web, always for qa",
      "Keep off",
    ]);
    expect(answers[1]).toHaveAttribute("data-variant", "primary");
    expect(answers[3]).not.toHaveAttribute("data-variant", "primary");
    expect(calls.some((one) => one.cmd === "allow_chat_browser")).toBe(false);
    await userEvent.click(answers[0]);
    await waitFor(() =>
      expect(calls.find((one) => one.cmd === "allow_chat_browser")?.args).toEqual({
        plane: PLANE,
        session: 8,
        openWeb: false,
        always: false,
      }),
    );
  });

  it("allows a refused host by the block Notice's own command, and owes the chat its restart", async () => {
    const calls = core({ allow_sandbox_block: { said: "Allowed." } });
    const { props } = draw([HOST]);
    await userEvent.click(screen.getByRole("button", { name: "Allow for this chat" }));
    await waitFor(() =>
      expect(calls).toContainEqual({
        cmd: "allow_sandbox_block",
        args: {
          plane: PLANE,
          session: 5,
          shown: HOST.answer.via === "sandbox-block" ? HOST.answer.shown : null,
          level: "chat",
        },
      }),
    );
    // What the block Notice does after an Allow, the window does too: its pane's copy is put
    // away and the chat is owed a restart to take the grant.
    await waitFor(() =>
      expect(props.onAnswered).toHaveBeenCalledWith(HOST, HOST.options[0], false),
    );
  });

  it("keeps a refused host blocked with the reason typed beside it, which its chat is told (#1987)", async () => {
    const calls = core();
    draw([HOST, permission(3, "a1", ["steward 3"])]);
    // Only a sandbox ask has the box: a permission's own answers go to its harness.
    const boxes = screen.getAllByRole("textbox", { name: /Why you keep it blocked/ });
    expect(boxes).toHaveLength(1);
    await userEvent.type(boxes[0], "use the internal mirror");
    await userEvent.click(screen.getByRole("button", { name: "Keep blocked" }));
    await waitFor(() =>
      expect(calls).toContainEqual({
        cmd: "keep_sandbox_block",
        args: {
          plane: PLANE,
          session: 5,
          shown: HOST.answer.via === "sandbox-block" ? HOST.answer.shown : null,
          reason: "use the internal mirror",
        },
      }),
    );
  });

  it("keeps a refused host blocked with no reason where none is typed (#1987)", async () => {
    const calls = core();
    draw([HOST]);
    await userEvent.click(screen.getByRole("button", { name: "Keep blocked" }));
    await waitFor(() =>
      expect(calls).toContainEqual({
        cmd: "keep_sandbox_block",
        args: {
          plane: PLANE,
          session: 5,
          shown: HOST.answer.via === "sandbox-block" ? HOST.answer.shown : null,
          reason: null,
        },
      }),
    );
  });

  it("says the source's own sentence when it refuses, and lists the ask still", async () => {
    core({ answer_ask: new Error("This ask was answered elsewhere.") });
    const { props } = draw([permission(3, "late", ["steward 3"])]);
    await userEvent.click(screen.getByRole("button", { name: "Deny" }));
    expect(await screen.findByText("This ask was answered elsewhere.")).toBeTruthy();
    expect(props.onAnswered).not.toHaveBeenCalled();
  });

  it("draws a dispatch grant as its own Notice, brief and all, and allows it by its digest", async () => {
    let held = [HELD];
    const calls = core({
      dispatch_grants_needed: () => held,
      allow_dispatch: () => {
        held = [];
        return { said: "Allowed for this chat." };
      },
    });
    draw([DISPATCH]);
    // What the Allow is bound to is on screen before it: the brief as the chat wrote it.
    expect(await screen.findByText("Check why the prod deploy is red.")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Allow for this chat" }));
    await waitFor(() =>
      expect(calls).toContainEqual({
        cmd: "allow_dispatch",
        args: { plane: PLANE, id: 7, level: "chat", also: [], shown: "s0" },
      }),
    );
  });

  it("clears the dispatch's Notice on its chat's pane when it is answered here, and the other way", async () => {
    let held = [HELD];
    const calls = core({
      dispatch_grants_needed: () => held,
      allow_dispatch: () => {
        held = [];
        return { said: "Allowed for this chat." };
      },
      keep_dispatch_blocked: () => {
        held = [];
        return true;
      },
    });
    const props = { plane: PLANE, onGo: vi.fn(), onLeave: vi.fn() };
    const both = (asks: readonly Shown[]) => (
      <>
        <section aria-label="pane">
          <DispatchGrantNotice plane={PLANE} session={4} asks={asks} />
        </section>
        <Inbox {...props} asks={asks} />
      </>
    );
    const { rerender } = render(both([DISPATCH]));
    const pane = screen.getByRole("region", { name: "pane" });
    const inbox = screen.getByRole("region", { name: "Inbox" });
    await within(pane).findByText("Check why the prod deploy is red.");
    await within(inbox).findByText("Check why the prod deploy is red.");

    // An Allow on the row as it is drawn waits for it to settle (#1695): nothing is sent.
    await userEvent.click(within(inbox).getByRole("button", { name: "Allow for this chat" }));
    expect(within(inbox).getByText(MOVED_JUST_NOW)).toBeTruthy();
    expect(calls.filter((one) => one.cmd === "allow_dispatch")).toEqual([]);
    read();
    await userEvent.click(within(inbox).getByRole("button", { name: "Allow for this chat" }));

    // Both are drawn from the one list the core holds: the pane's question is gone with it.
    await waitFor(() =>
      expect(within(pane).queryByText("Check why the prod deploy is red.")).toBeNull(),
    );
    rerender(both([]));
    // And the Inbox keeps what it answered.
    expect(within(inbox).getByText(NOTHING_WAITS)).toBeTruthy();
    const recent = within(inbox).getByRole("region", { name: "Recently answered" });
    // With the words of the way out that was pressed.
    expect(within(recent).getByText("Allow for this chat")).toBeTruthy();
  });

  it("holds a dispatch's Allow from the keyboard until the row settles, as from the pointer", async () => {
    let held = [HELD];
    const calls = core({
      dispatch_grants_needed: () => held,
      allow_dispatch: () => {
        held = [];
        return { said: "Allowed for this chat." };
      },
    });
    render(<Inbox plane={PLANE} onGo={vi.fn()} onLeave={vi.fn()} asks={[DISPATCH]} />);
    const inbox = screen.getByRole("region", { name: "Inbox" });
    await within(inbox).findByText("Check why the prod deploy is red.");
    const allow = within(inbox).getByRole("button", { name: "Allow for this chat" });
    allow.focus();
    await userEvent.keyboard("{Enter}");
    await userEvent.keyboard(" ");
    expect(within(inbox).getByText(MOVED_JUST_NOW)).toBeTruthy();
    expect(calls.filter((one) => one.cmd === "allow_dispatch")).toEqual([]);
    read();
    allow.focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(calls.filter((one) => one.cmd === "allow_dispatch")).toHaveLength(1),
    );
  });

  it("gives a chat waiting on a reply a box, and sends what is typed as the person's message", async () => {
    const calls = core({ asks_waiting: { asks: [QUESTION] } });
    draw([QUESTION]);
    await userEvent.type(
      screen.getByRole("textbox", { name: "Reply to steward 6" }),
      "Yes, ship it",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send reply" }));
    await waitFor(() =>
      expect(calls).toContainEqual({
        cmd: "send_input",
        args: { plane: PLANE, session: 6, text: replyBytes("Yes, ship it") },
      }),
    );
    expect((screen.getByRole("textbox") as HTMLInputElement).value).toBe("");
  });

  it("says why a chat waits where it asked nothing, and offers it no reply box", () => {
    core();
    draw([QUESTION], { whyOf: (session) => (session === 6 ? "deep failed" : undefined) });
    expect(screen.getByText("deep failed")).toBeTruthy();
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(screen.getByRole("button", { name: "Go to chat steward 6" })).toBeTruthy();
  });

  it("sends nothing for an empty reply", () => {
    core();
    draw([QUESTION]);
    expect(screen.getByRole("button", { name: "Send reply" })).toHaveProperty("disabled", true);
  });

  it("names a prompt waiting in a harness's terminal, with only the way to it", () => {
    core();
    draw([TERMINAL]);
    expect(screen.getByText("Waiting in its terminal: a permission prompt")).toBeTruthy();
    expect(screen.getAllByRole("button").map((one) => one.textContent)).toEqual(["Go to chat"]);
  });
});

describe("the train 43 review (#1700)", () => {
  it("sends no reply when the chat no longer waits on one, and says so", async () => {
    // The chat moved on between the last read and the press: its Enter would land on whatever
    // is in front, so the asks are read again first.
    const calls = core({ asks_waiting: { asks: [] } });
    draw([QUESTION]);
    await userEvent.type(screen.getByRole("textbox", { name: "Reply to steward 6" }), "Yes");
    await userEvent.click(screen.getByRole("button", { name: "Send reply" }));

    expect(await screen.findByText(MOVED_ON)).toBeTruthy();
    expect(calls.map((one) => one.cmd)).not.toContain("send_input");
    // What was typed is kept, to be sent in the chat itself.
    expect((screen.getByRole("textbox") as HTMLInputElement).value).toBe("Yes");
  });

  it("keeps Home and End in the reply box: they move its cursor, not the list", async () => {
    core();
    draw([permission(3, "01J0A", ["steward 3"]), QUESTION]);
    const box = screen.getByRole("textbox", { name: "Reply to steward 6" });
    await userEvent.type(box, "abc");

    await userEvent.keyboard("{Home}X");
    expect(box).toHaveFocus();
    expect((box as HTMLInputElement).value).toBe("Xabc");
    await userEvent.keyboard("{End}Y");
    expect((box as HTMLInputElement).value).toBe("XabcY");
  });

  it("offers the reply box to a chat waiting on a reply that also has a reason, beside it", () => {
    core();
    draw([QUESTION], {
      whyOf: (session) => (session === 6 ? "its report has nowhere to go" : undefined),
      asked: (session) => session === 6,
    });

    expect(screen.getByText("its report has nowhere to go")).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "Reply to steward 6" })).toBeTruthy();
  });

  it("keeps the keyboard in the list after an answer takes its ask away", async () => {
    core();
    const first = permission(3, "01J0A", ["steward 3"]);
    const second = permission(4, "01J0B", ["steward 4"], "Run cargo build");
    const { again } = draw([first, second]);
    const allow = within(screen.getByRole("region", { name: "steward 3" })).getByRole("button", {
      name: "Allow",
    });
    await userEvent.click(allow);
    // Pressed and busy, it keeps the keyboard: a disabled button would drop it to the page.
    expect(allow).toHaveFocus();

    again([second]);

    expect(document.activeElement).not.toBe(document.body);
    expect(document.activeElement?.closest(".inbox-list")).not.toBeNull();
  });
});

describe("the keyboard after an answer (#1700)", () => {
  it("is never taken back for a press on nothing, while what it was on is still drawn", () => {
    core();
    const first = permission(3, "01J0A", ["steward 3"]);
    const second = permission(4, "01J0B", ["steward 4"], "Run cargo build");
    const { again } = draw([first, second]);
    const allow = within(screen.getByRole("region", { name: "steward 3" })).getByRole("button", {
      name: "Allow",
    });
    allow.focus();
    act(() => allow.blur());

    again([first, second, permission(5, "01J0C", ["steward 5"])]);

    expect(document.activeElement).toBe(document.body);
  });

  it("keeps the keyboard in the reply box once Send reply is pressed", async () => {
    // Send reply is disabled while it sends and once the box is empty: the keyboard goes back
    // to the box rather than to the page.
    const calls = core({ asks_waiting: { asks: [QUESTION] } });
    draw([QUESTION]);
    const box = screen.getByRole("textbox", { name: "Reply to steward 6" });
    await userEvent.type(box, "Yes");
    await userEvent.click(screen.getByRole("button", { name: "Send reply" }));

    await waitFor(() => expect(calls.map((one) => one.cmd)).toContain("send_input"));
    expect(box).toHaveFocus();
  });

  it("says so and sends nothing where the asks could not be read before a reply", async () => {
    const calls = core({ asks_waiting: new Error("the project is not open") });
    draw([QUESTION]);
    await userEvent.type(screen.getByRole("textbox", { name: "Reply to steward 6" }), "Yes");
    await userEvent.click(screen.getByRole("button", { name: "Send reply" }));

    expect(await screen.findByText(NOT_CHECKED)).toBeTruthy();
    expect(calls.map((one) => one.cmd)).not.toContain("send_input");
  });
});

describe("the project's Notices (#1695)", () => {
  const line = (cause: string, words: string) => (
    <Notice cause={cause} onDismiss={() => undefined}>
      {words}
    </Notice>
  );

  it("lists them after the asks and before the updates, each with its ways out", () => {
    core();
    draw([permission(3, "01J0A", ["steward 3"])], {
      notices: <>{line("pin-dormant:able", "able is gone, kept dormant")}</>,
      updates: {
        rows: [
          {
            update: {
              key: "task:1",
              kind: "task-done",
              at: 1,
              session: 3,
              chain: ["steward 3"],
              says: "deep: done",
              read: false,
            },
            dismiss: vi.fn(),
          },
        ],
        onMarkAllRead: vi.fn(),
        onDismissAll: vi.fn(),
      },
    });

    expect(chats()).toEqual(["steward 3", NOTICES, UPDATES]);
    const notices = screen.getByRole("region", { name: NOTICES });
    expect(within(notices).getByText("able is gone, kept dormant")).toBeTruthy();
    expect(within(notices).getByRole("button", { name: "Dismiss" })).toBeTruthy();
  });

  it("says nothing of Notices while there are none, and counts the ones it lists", () => {
    core();
    const onNotices = vi.fn();
    const { props } = draw([], { notices: <></>, onNotices });

    expect(screen.queryByRole("region", { name: NOTICES })).toBeNull();
    expect(onNotices).toHaveBeenLastCalledWith(0);
    cleanup();

    render(
      <Inbox
        {...props}
        notices={<>{line("pin-dormant:able", "able is gone")}</>}
        onNotices={onNotices}
      />,
    );
    expect(screen.getByRole("region", { name: NOTICES })).toBeTruthy();
    expect(onNotices).toHaveBeenLastCalledWith(1);
  });
});

describe("what was allowed lately, in an empty Inbox (D-1700-7)", () => {
  it("lists the grants of the last day the sources keep, read-only, under what was answered here", async () => {
    core({
      sandbox_grants: [
        {
          id: "g1",
          what: "host",
          target: "api.example.com",
          persona: null,
          level: "you",
          by: null,
          at: Math.floor(Date.now() / 1000) - 600,
          chat: "steward 3",
          locked: null,
          waiting: null,
          for_no_persona: false,
          expected: false,
        },
      ],
      dispatch_grants: { grants: [], all_locked: null, locked_pairs: [], locked_by: null },
    });
    draw([]);
    const allowed = await screen.findByRole("region", { name: ALLOWED_LATELY });
    expect(within(allowed).getByText("Reach api.example.com")).toBeTruthy();
    expect(within(allowed).getByText("for you on this machine")).toBeTruthy();
    // Read-only: revoked where the grants are listed whole.
    expect(within(allowed).queryAllByRole("button")).toEqual([]);
  });

  it("lists a Keep blocked the person answered beside what was allowed (#1700)", async () => {
    core({
      sandbox_grants: [],
      dispatch_grants: { grants: [], all_locked: null, locked_pairs: [], locked_by: null },
      sandbox_kept_blocked: [
        { target: "pypi.org:443", chat: "steward 3", at: Math.floor(Date.now() / 1000) - 60 },
      ],
    });
    draw([]);
    const allowed = await screen.findByRole("region", { name: ALLOWED_LATELY });
    expect(within(allowed).getByText("Kept pypi.org:443 blocked")).toBeTruthy();
    expect(within(allowed).getByText("for that chat")).toBeTruthy();
    expect(within(allowed).queryAllByRole("button")).toEqual([]);
  });

  it("reads no grants while something waits", async () => {
    const calls = core();
    draw([permission(3, "a1", ["steward 3"])]);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
    expect(calls.some(({ cmd }) => cmd === "sandbox_grants")).toBe(false);
  });
});

describe("the window's own lines, with a project in front (D-LB-1)", () => {
  it("gives the window a place at the top of the Notices, shown while it lists any", () => {
    core();
    let place: HTMLDivElement | null = null;
    const at = (element: HTMLDivElement | null) => {
      place = element;
    };
    const { rerender, props } = draw([], { windowLines: { at, count: 0 } });
    // Nothing listed yet: the section is there and hidden, so it is no region a reader meets.
    expect(screen.queryByRole("region", { name: NOTICES })).toBeNull();
    const notices = document.querySelector<HTMLElement>("section.inbox-notices") as HTMLElement;
    expect(notices.hidden).toBe(true);
    expect(place).not.toBeNull();
    expect(notices.contains(place)).toBe(true);

    // The window lists a line there: the section is drawn, and the line is in it.
    rerender(<Inbox {...props} windowLines={{ at, count: 1 }} />);
    expect(screen.getByRole("region", { name: NOTICES }).hidden).toBe(false);
    expect(
      (screen.getByRole("region", { name: NOTICES }).firstElementChild as HTMLElement).tagName,
    ).toBe("H3");
  });
});

describe("a Notice that grants something waits to settle (#1695)", () => {
  const granting = (onPress: () => void) => (
    <Notice
      cause="sandbox-hosts:persona:devops"
      fixes={[{ label: "Allow devops's hosts", onPress }]}
      onDismiss={() => undefined}
    >
      devops reaches 2 hosts
    </Notice>
  );

  it("does nothing on a press as it is drawn, says why, and acts once the person has read it", async () => {
    core();
    const allowed = vi.fn();
    render(
      <Inbox
        plane={PLANE}
        asks={[]}
        onGo={vi.fn()}
        onLeave={vi.fn()}
        notices={granting(allowed)}
      />,
    );
    const notices = screen.getByRole("region", { name: NOTICES });
    await userEvent.click(within(notices).getByRole("button", { name: "Allow devops's hosts" }));
    expect(allowed).not.toHaveBeenCalled();
    expect(within(notices).getByText(NOTICE_MOVED_JUST_NOW)).toBeTruthy();
    read();
    await userEvent.click(within(notices).getByRole("button", { name: "Allow devops's hosts" }));
    expect(allowed).toHaveBeenCalledOnce();
  });

  it("holds a press from the keyboard as it holds one from the pointer", async () => {
    core();
    const allowed = vi.fn();
    render(
      <Inbox
        plane={PLANE}
        asks={[]}
        onGo={vi.fn()}
        onLeave={vi.fn()}
        notices={granting(allowed)}
      />,
    );
    const notices = screen.getByRole("region", { name: NOTICES });
    const allow = within(notices).getByRole("button", { name: "Allow devops's hosts" });
    allow.focus();
    await userEvent.keyboard("{Enter}");
    await userEvent.keyboard(" ");
    expect(allowed).not.toHaveBeenCalled();
    expect(within(notices).getByText(NOTICE_MOVED_JUST_NOW)).toBeTruthy();
    read();
    await userEvent.keyboard("{Enter}");
    expect(allowed).toHaveBeenCalledOnce();
  });

  it("leaves a Notice that grants nothing to answer at once", async () => {
    core();
    const fixed = vi.fn();
    render(
      <Inbox
        plane={PLANE}
        asks={[]}
        onGo={vi.fn()}
        onLeave={vi.fn()}
        notices={
          <Notice cause="pin-dormant:able" fixes={[{ label: "Forget", onPress: fixed }]}>
            able is gone
          </Notice>
        }
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Forget" }));
    expect(fixed).toHaveBeenCalledOnce();
  });
});

describe("ignoring a chat that waits on a reply", () => {
  it("offers the queue's own Ignore on a chat waiting on a reply, and on nothing that asks a decision", async () => {
    core();
    const ignored: number[] = [];
    draw([QUESTION, TERMINAL, permission(3, "p", ["steward 3"])], {
      onIgnore: (session) => ignored.push(session),
    });
    const ignores = screen.getAllByRole("button", { name: /^Ignore .+ until it asks again$/ });
    // A question and a prompt in its terminal: the chat asks again at its next stop. A
    // permission is a decision, and is never put away unanswered.
    expect(ignores.map((one) => one.getAttribute("aria-label"))).toEqual([
      "Ignore steward 6 until it asks again",
      "Ignore steward 8 until it asks again",
    ]);
    await userEvent.click(ignores[0]);
    expect(ignored).toEqual([6]);
  });
});

describe("updates, after the asks (#1693)", () => {
  const update = (key: string, at: number, more: Partial<InboxUpdate> = {}): InboxUpdate => ({
    key,
    kind: "task-done",
    at,
    session: 3,
    chain: ["steward 3"],
    says: `${key}: done`,
    read: false,
    ...more,
  });
  const rows = (list: InboxUpdate[], more: Partial<UpdateRow> = {}): UpdateRow[] =>
    list.map((one) => ({ update: one, dismiss: vi.fn(), ...more }));
  const updates = (list: UpdateRow[]): Updates => ({
    rows: list,
    onMarkAllRead: vi.fn(),
    onDismissAll: vi.fn(),
  });

  it("lists them after the asks, in the order given, newest first, each with its kind and words", () => {
    core();
    draw([permission(3, "a1", ["steward 3"])], {
      updates: updates(
        rows([
          update("task:b", 2_000, { kind: "task-failed", says: "drill: failed" }),
          update("doctor:git:fail", 1_000, { kind: "doctor", chain: [], says: "git: no identity" }),
        ]),
      ),
    });
    const list = screen.getByRole("region", { name: UPDATES });
    // After the asks: the ask's group comes first in the list.
    expect(chats()).toEqual(["steward 3", UPDATES]);
    expect(
      within(list)
        .getAllByRole("listitem")
        .map((row) => row.getAttribute("data-kind")),
    ).toEqual(["task-failed", "doctor"]);
    expect(within(list).getByText("steward 3: drill: failed")).toBeTruthy();
    expect(within(list).getByText("git: no identity")).toBeTruthy();
    expect(within(list).getAllByText(/Task failed|Doctor/).length).toBe(2);
  });

  it("draws an update's words as text, never as a control", () => {
    core();
    draw([], {
      updates: updates(rows([update("x", 1, { says: "<button>Allow</button>" })])),
    });
    expect(screen.getByText("steward 3: <button>Allow</button>")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Allow" })).toBeNull();
  });

  it("stands under the empty Inbox's sentence: an update is nothing that waits", () => {
    core();
    draw([], { updates: updates(rows([update("task:a", 1)])) });
    expect(screen.getByText(NOTHING_WAITS)).toBeTruthy();
    expect(screen.getByRole("region", { name: UPDATES })).toBeTruthy();
  });

  it("marks every update read and dismisses every one, and answers no ask doing either", async () => {
    const calls = core();
    const these = updates(rows([update("task:a", 1)]));
    draw([permission(3, "a1", ["steward 3"])], { updates: these });
    await userEvent.click(screen.getByRole("button", { name: "Mark all read" }));
    await userEvent.click(screen.getByRole("button", { name: "Dismiss all" }));
    expect(these.onMarkAllRead).toHaveBeenCalledOnce();
    expect(these.onDismissAll).toHaveBeenCalledOnce();
    // No ask was answered in bulk: nothing went to any source.
    expect(calls.filter((one) => one.cmd === "answer_ask")).toEqual([]);
    expect(screen.getByRole("button", { name: "Allow" })).toBeTruthy();
  });

  it("offers Mark all read only while something is unread", () => {
    core();
    draw([], { updates: updates(rows([update("task:a", 1, { read: true })])) });
    expect(screen.getByRole("button", { name: "Mark all read" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    expect(screen.queryByText(/· new/)).toBeNull();
  });

  it("says nothing of updates while there are none", () => {
    core();
    draw([], { updates: updates([]) });
    expect(screen.queryByRole("region", { name: UPDATES })).toBeNull();
    expect(screen.queryByRole("button", { name: "Dismiss all" })).toBeNull();
  });

  it("dismisses one, goes to its chat, and presses a source's own answers", async () => {
    core();
    const go = vi.fn();
    const never = vi.fn();
    const [row] = rows([update("away:steward#devops#", 1, { kind: "refused-away", chain: [] })], {
      go,
      more: "devops works with its own access.",
      answers: [
        {
          label: "Never for this pair",
          name: "Never for this pair: steward",
          title: "",
          press: never,
        },
      ],
    });
    draw([], { updates: updates([row]) });
    expect(screen.getByText("devops works with its own access.")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Never for this pair: steward" }));
    await userEvent.click(screen.getByRole("button", { name: /^Go to chat/ }));
    await userEvent.click(screen.getByRole("button", { name: /^Dismiss: / }));
    expect(never).toHaveBeenCalledOnce();
    expect(go).toHaveBeenCalledOnce();
    expect(row.dismiss).toHaveBeenCalledOnce();
  });

  it("draws an away refusal's Allow from now on as a grant and Never for this pair plain (ADR 0097 §6)", () => {
    core();
    const [row] = rows([update("away:steward#devops#", 1, { kind: "refused-away", chain: [] })], {
      answers: [
        {
          label: "Never for this pair",
          name: "Never for this pair: steward",
          title: "",
          press: vi.fn(),
        },
        {
          label: "Allow from now on",
          name: "Allow from now on: steward to devops",
          title: "",
          allows: true,
          press: vi.fn(),
        },
      ],
    });
    draw([], { updates: updates([row]) });
    const allow = screen.getByRole("button", { name: "Allow from now on: steward to devops" });
    const never = screen.getByRole("button", { name: "Never for this pair: steward" });
    expect(allow).toHaveAttribute("data-variant", "grants");
    expect(allow).toHaveClass("inbox-answer", "allows");
    expect(allow.querySelector('[data-part="icon"] svg.lucide-lock-open')).toHaveAttribute(
      "aria-hidden",
      "true",
    );
    expect(never).not.toHaveAttribute("data-variant", "grants");
    expect(never).not.toHaveAttribute("data-variant", "primary");
  });

  it("is reached by ↓ from the last ask, its two buttons first, and Enter presses one", async () => {
    core();
    const these = updates(rows([update("task:a", 1)]));
    draw([permission(3, "a1", ["steward 3"])], { updates: these });
    const ask = screen.getAllByRole("listitem")[0];
    act(() => ask.focus());
    // The ask, its two answers and Go to chat; then the updates' two buttons.
    for (let step = 0; step < 4; step += 1) await userEvent.keyboard("{ArrowDown}");
    expect(document.activeElement?.textContent).toBe("Mark all read");
    await userEvent.keyboard("{ArrowDown}");
    await userEvent.keyboard("{Enter}");
    expect(these.onDismissAll).toHaveBeenCalledOnce();
  });
});

describe("going to the chat", () => {
  it("has Go to chat on every ask, a task's too, which goes to the chat that asked", async () => {
    core({ dispatch_grants_needed: [HELD] });
    const task = permission(9, "t1", ["steward 3", "#3046 drill"]);
    const { props } = draw([task, HOST, DISPATCH, QUESTION, TERMINAL]);
    const goes = screen.getAllByRole("button", { name: /^Go to chat / });
    expect(goes).toHaveLength(5);
    await userEvent.click(
      screen.getByRole("button", { name: "Go to chat steward 3 › #3046 drill" }),
    );
    expect(props.onGo).toHaveBeenCalledWith(9);
  });
});

describe("the keyboard (I-11)", () => {
  it("moves with ↑ and ↓ from the ask to its buttons and on, and Enter presses the one focused", async () => {
    const calls = core();
    draw([permission(3, "k1", ["steward 3"]), permission(5, "k2", ["steward 5"], "Run ls")]);
    const [first, second] = screen.getAllByRole("listitem");
    act(() => first.focus());
    await userEvent.keyboard("{ArrowDown}");
    expect(document.activeElement?.textContent).toBe("Allow");
    await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}");
    expect(document.activeElement).toBe(second);
    await userEvent.keyboard("{ArrowUp}");
    expect(document.activeElement?.textContent).toBe("Go to chat");
    await userEvent.keyboard("{ArrowUp}{ArrowUp}{Enter}");
    await waitFor(() =>
      expect(calls.find((one) => one.cmd === "answer_ask")?.args).toMatchObject({
        ask: "k1",
        option: "allow",
      }),
    );
  });

  it("is one Tab stop, on the first ask, and Home and End go to the ends", async () => {
    core();
    draw([permission(3, "h1", ["steward 3"]), permission(5, "h2", ["steward 5"], "Run ls")]);
    const list = screen.getByRole("region", { name: "Inbox" });
    const stops = [...list.querySelectorAll('[tabindex="0"]')];
    expect(stops).toEqual([screen.getAllByRole("listitem")[0]]);
    act(() => (stops[0] as HTMLElement).focus());
    await userEvent.keyboard("{End}");
    expect(document.activeElement).toHaveAccessibleName("Go to chat steward 5");
    await userEvent.keyboard("{Home}");
    expect(document.activeElement).toBe(screen.getAllByRole("listitem")[0]);
  });

  it("leaves on Escape, for the chat in front", async () => {
    core();
    const { props } = draw([permission(3, "e1", ["steward 3"])]);
    act(() => screen.getByRole("listitem").focus());
    await userEvent.keyboard("{Escape}");
    expect(props.onLeave).toHaveBeenCalledOnce();
  });

  it("answers nothing on a single letter, wherever the keyboard is", async () => {
    const calls = core();
    draw([permission(3, "l1", ["steward 3"]), HOST]);
    for (const row of screen.getAllByRole("listitem")) {
      act(() => row.focus());
      await userEvent.keyboard("aydnk1 ");
    }
    act(() => screen.getAllByRole("button", { name: "Allow" })[0].focus());
    await userEvent.keyboard("ydn");
    expect(calls.filter((one) => one.cmd !== "dispatch_grants_needed")).toEqual([]);
  });
});

describe("where a notification lands (#1694)", () => {
  it("is the chat's group, its first ask given the keyboard, and the list keeps one Tab stop", async () => {
    core();
    draw([
      permission(3, "n1", ["steward 3"]),
      permission(5, "n2", ["steward 5"], "Run ls"),
      permission(5, "n3", ["steward 5"], "Run pwd"),
    ]);
    act(() => {
      expect(landOnGroup(5)).toBe(true);
    });
    const group = screen.getByRole("region", { name: "steward 5" });
    expect(document.activeElement).toBe(within(group).getAllByRole("listitem")[0]);
    // From there the keys go on as from any ask.
    await userEvent.keyboard("{ArrowDown}");
    expect(document.activeElement?.textContent).toBe("Allow");
  });

  it("is nowhere for a chat with nothing left in the Inbox", () => {
    core();
    draw([permission(3, "n1", ["steward 3"])]);
    expect(landOnGroup(9)).toBe(false);
  });
});

describe("the bytes a reply is typed as", () => {
  it("are one paste and Enter, with nothing typed able to end the paste early", () => {
    expect(replyBytes("yes")).toBe("\u001b[200~yes\u001b[201~\r");
    expect(replyBytes("go\u001b[201~\rrm -rf /")).toBe("\u001b[200~go[201~\nrm -rf /\u001b[201~\r");
    expect(replyBytes("  ")).toBeUndefined();
  });
});

describe("an Allow waits for its row to settle (#1695)", () => {
  it("allows nothing pressed just after the ask was drawn, says so, and allows once it was read", async () => {
    const calls = core();
    render(
      <Inbox
        plane={PLANE}
        asks={[permission(3, "01J0A", ["steward 3"])]}
        onGo={vi.fn()}
        onLeave={vi.fn()}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Allow" }));
    expect(screen.getByRole("status")).toHaveTextContent(MOVED_JUST_NOW);
    expect(calls.some(({ cmd }) => cmd === "answer_ask")).toBe(false);

    read();
    await userEvent.click(screen.getByRole("button", { name: "Allow" }));
    await waitFor(() => expect(calls.some(({ cmd }) => cmd === "answer_ask")).toBe(true));
  });

  it("allows nothing on a row an ask joining above it just moved", async () => {
    const calls = core();
    const first = permission(3, "01J0A", ["steward 3"]);
    const later = permission(4, "01J0B", ["steward 4"], "Run cargo build");
    const { rerender, props } = draw([first, later]);
    // The chat above asks a second thing: its group grows, and steward 4's row moves down
    // under the pointer, with no time to read it.
    rerender(
      <Inbox
        {...props}
        asks={[first, permission(3, "01J0C", ["steward 3"], "Run cargo fmt"), later]}
      />,
    );
    const group = screen.getByRole("region", { name: "steward 4" });
    await userEvent.click(within(group).getByRole("button", { name: "Allow" }));
    expect(within(group).getByRole("status")).toHaveTextContent(MOVED_JUST_NOW);
    expect(calls.some(({ cmd }) => cmd === "answer_ask")).toBe(false);
  });

  it("allows nothing on an ask whose words changed under the pointer, though it kept its place", async () => {
    const calls = core();
    const { rerender, props } = draw([HOST]);
    // The same ask, under the same key, now saying something else: read anew before an Allow.
    rerender(
      <Inbox
        {...props}
        asks={[{ ...HOST, says: "A connection to api.example.com waits on your answer" }]}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Allow for this chat" }));
    expect(screen.getByRole("status")).toHaveTextContent(MOVED_JUST_NOW);
    expect(calls.some(({ cmd }) => cmd === "allow_sandbox_block")).toBe(false);
  });

  it("never holds a Deny: only what allows waits", async () => {
    const calls = core();
    render(
      <Inbox
        plane={PLANE}
        asks={[permission(3, "01J0A", ["steward 3"])]}
        onGo={vi.fn()}
        onLeave={vi.fn()}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Deny" }));
    await waitFor(() => expect(calls.some(({ cmd }) => cmd === "answer_ask")).toBe(true));
  });
});

describe("what the hand's list named, listed in the Inbox (#1695)", () => {
  const away: Needing = {
    plane: "/home/dev/ops",
    project: "ops",
    session: 7,
    name: "steward 7",
    workspace: "infra",
    go: {
      id: "needs.show:7",
      title: "Show steward 7",
      available: true,
      reason: "",
      does: { verb: "showChat", session: 7 },
      name: "steward 7",
    },
    ignore: {
      id: "needs.ignore:7",
      title: "Ignore steward 7 until it asks again",
      available: true,
      reason: "",
      does: { verb: "ignoreNeedsYou", session: 7 },
      name: "steward 7",
    },
  };
  const shell: Quiet = { name: "shell 2", project: "plane", plane: PLANE, session: 2 };
  const codex: Quiet = { name: "codex 4", project: "ops", plane: "/home/dev/ops", session: 4 };
  const elsewhere = (over: Partial<Elsewhere> = {}): Elsewhere => ({
    asking: [],
    quiet: [],
    onPress: vi.fn(),
    ...over,
  });

  // What waits in other projects is listed under All projects (qw359): the scope these read.
  beforeEach(() => keepInboxScope(PLANE, "all"));

  it("lists a chat waiting in another window as a group of its own, with Go to chat and Ignore", async () => {
    core();
    const there = elsewhere({ asking: [away] });
    draw([permission(3, "01J0A", ["steward 3"])], { elsewhere: there });

    expect(chats()).toEqual(["steward 3", "steward 7"]);
    const group = screen.getByRole("region", { name: "steward 7 · infra · ops" });
    expect(within(group).getByText(IN_ANOTHER_WINDOW)).toBeTruthy();
    await userEvent.click(within(group).getByRole("button", { name: /^Go to chat steward 7/ }));
    expect(there.onPress).toHaveBeenLastCalledWith("/home/dev/ops", away.go);
    await userEvent.click(
      within(group).getByRole("button", { name: "Ignore steward 7 until it asks again" }),
    );
    expect(there.onPress).toHaveBeenLastCalledWith("/home/dev/ops", away.ignore);
  });

  it("is not empty while a chat waits in another window", () => {
    core();
    draw([], { elsewhere: elsewhere({ asking: [away] }) });
    expect(screen.queryByText(NOTHING_WAITS)).toBeNull();
    expect(screen.getByRole("region", { name: "steward 7 · infra · ops" })).toBeTruthy();
  });

  it("names each chat that cannot say it waits as a Notice, in this project and in any other, with Go to chat", async () => {
    core();
    const onNotices = vi.fn();
    const there = elsewhere({ quiet: [shell, codex] });
    draw([], { elsewhere: there, notices: <></>, onNotices });

    // Never "nothing" over a chat purlis cannot see.
    expect(screen.queryByText(NOTHING_WAITS)).toBeNull();
    expect(
      screen.getByText("Nothing has asked for you, but 2 chats can't tell purlis they're waiting"),
    ).toBeTruthy();
    const notices = screen.getByRole("region", { name: NOTICES });
    // This project's own is named alone; another project's says which.
    expect(notices.querySelector("[data-cause='chat-quiet:/home/dev/plane#2']")).toHaveTextContent(
      "shell 2 can't tell purlis it's waiting.",
    );
    expect(notices.querySelector("[data-cause='chat-quiet:/home/dev/ops#4']")).toHaveTextContent(
      "codex 4 in ops can't tell purlis it's waiting.",
    );
    // The faint hand stands for them; the status line does not count them a second time.
    expect(onNotices).toHaveBeenLastCalledWith(0);

    const codexNotice = notices.querySelector<HTMLElement>(
      "[data-cause='chat-quiet:/home/dev/ops#4']",
    );
    await userEvent.click(
      within(codexNotice as HTMLElement).getByRole("button", { name: "Go to chat" }),
    );
    expect(there.onPress).toHaveBeenLastCalledWith(
      "/home/dev/ops",
      expect.objectContaining({ does: { verb: "showChat", session: 4 } }),
    );
  });

  it("names each other project of this window with something waiting, one line each, with Go to project (#1700)", async () => {
    core();
    const onOpen = vi.fn();
    const there = elsewhere({
      waitingHere: [
        { plane: PLANE, project: "plane", count: 1 },
        { plane: "/home/dev/ops", project: "ops", count: 2 },
        { plane: "/home/dev/web", project: "web", count: 1 },
      ],
      onOpen,
    });
    draw([permission(3, "01J0A", ["steward 3"])], { elsewhere: there });

    const lines = screen.getByRole("region", { name: OTHER_PROJECTS });
    // Not this project's own: its asks are the list above.
    expect(
      within(lines)
        .getAllByRole("listitem")
        .map((one) => one.textContent),
    ).toEqual(["ops · 2 waiting on youGo to project", "web · 1 waiting on youGo to project"]);
    // A line answers nothing: its one control goes to the project, whose Inbox answers it.
    expect(
      within(lines)
        .getAllByRole("button")
        .map((one) => one.textContent),
    ).toEqual(["Go to project", "Go to project"]);
    await userEvent.click(within(lines).getByRole("button", { name: "Go to project ops" }));
    expect(onOpen).toHaveBeenLastCalledWith("/home/dev/ops");
  });

  it("is not empty while another project of this window waits, and draws no line where none does", () => {
    core();
    const { unmount } = draw([], {
      elsewhere: elsewhere({
        waitingHere: [{ plane: "/home/dev/ops", project: "ops", count: 1 }],
        onOpen: vi.fn(),
      }),
    });
    expect(screen.queryByText(NOTHING_WAITS)).toBeNull();
    expect(screen.getByRole("region", { name: OTHER_PROJECTS })).toBeTruthy();
    unmount();

    draw([], {
      elsewhere: elsewhere({
        waitingHere: [{ plane: PLANE, project: "plane", count: 2 }],
        onOpen: vi.fn(),
      }),
    });
    expect(screen.getByText(NOTHING_WAITS)).toBeTruthy();
    expect(screen.queryByRole("region", { name: OTHER_PROJECTS })).toBeNull();
  });

  it("draws the Notices while it lists only what the status line leaves out", () => {
    core();
    draw([], {
      notices: (
        <Notice cause="doctor-finding:hooks" onDismiss={() => undefined}>
          hooks are not installed
        </Notice>
      ),
      onNotices: vi.fn(),
    });
    expect(
      within(screen.getByRole("region", { name: NOTICES })).getByText("hooks are not installed"),
    ).toBeTruthy();
  });
});

describe("a folder's ask in the Inbox (#1986)", () => {
  const FOLDER: Shown = {
    session: 3,
    ask: "block:3:write:outside:/work/notes",
    says: "The sandbox refused a write in /work/notes",
    options: [
      { id: "chat", label: "Allow for this chat", allows: true },
      { id: "keep", label: "Keep blocked", allows: false },
    ],
    source: "sandbox-write",
    chain: ["steward 3"],
    answer: {
      via: "sandbox-block",
      shown: { operation: "write", kind: "outside", what: "write", target: "/work/notes" },
    },
  };

  it("says before its Allow which chat restarts and what will stop", async () => {
    core({ what_a_restart_ends: { listening: [{ program: "node", port: 5173 }], others: 0 } });
    draw([FOLDER]);

    await waitFor(() =>
      expect(
        screen.getByText(/^Allowing restarts steward 3 /, { selector: "p" }),
      ).toHaveTextContent(
        "Allowing restarts steward 3 on its conversation at its next pause between turns; its node on port 5173 will stop.",
      ),
    );
  });

  it("says nothing of a restart for a host's ask", () => {
    const calls = core();
    draw([HOST]);
    expect(screen.queryByText(/^Allowing restarts/)).toBeNull();
    expect(calls.map((one) => one.cmd)).not.toContain("what_a_restart_ends");
  });
});
