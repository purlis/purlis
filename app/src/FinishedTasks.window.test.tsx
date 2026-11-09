// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { FinishedTask, OpenChat } from "./bindings";
import { forgetThisLaunch } from "./regions";
import { stripNamed } from "./test-strips";
import { aTabForEachChat } from "./laidOut.testkit";
import { tipOf } from "./ui/tooltip/tooltip.testkit";
import { findTree, heard, rowNamed, rowsOf } from "./ui/tree/tree.testkit";

/**
 * **A chat's finished tasks, against the whole window** (#1485): a task ends at its report, and
 * its row stays under the chat that asked. Done and cancelled fold into one "Finished (n)" line
 * with Clear finished; a failure stays a row of its own. A row shows its report as text, and
 * Reopen makes it an ordinary chat with a tab.
 *
 * The core here is a pretend one that holds the finished rows as the real one reads them from
 * its dispatch records: `finished_tasks` lists them, `clear_finished_tasks` takes rows away,
 * and `reopen_finished_task` starts a chat and takes the row.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

const STEWARD: OpenChat = {
  session: 4,
  name: "4",
  cwd: ALPHA,
  harness: "claude",
  in_front: true,
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
};

/** A task the steward chat asked for that is still working. */
const WORKING: OpenChat = {
  ...STEWARD,
  session: 9,
  name: "9",
  in_front: false,
  persona: "devops",
  label: "read the logs",
  from: {
    name: "steward 4",
    workspace: "alpha",
    chat: 4,
    task: true,
    tab: false,
    reported: false,
    unreported: false,
  },
};

function finished(id: string, name: string, more: Partial<FinishedTask> = {}): FinishedTask {
  return {
    id,
    asker: 4,
    name,
    chat: null,
    persona: "devops",
    how: "done",
    outcome: "done",
    folds: true,
    report: `${name}: all good.`,
    changed: null,
    ended: "2026-10-08T12:04:30+00:00",
    place: "alpha",
    branch: null,
    reopens: true,
    not_reopened: null,
    did_not_start: false,
    attempts: 0,
    waits: null,
    ...more,
  };
}

/** Five that came out done, as the five "live check" tasks did, and one that failed. */
const FIVE_DONE = ["talk", "listen", "read", "write", "count"].map((name, at) =>
  finished(`01K6DONE${at}`, `live check ${name}`),
);
const FAILED = finished("01K6FAILED", "check staging", {
  how: "failed",
  outcome: "failed",
  folds: false,
  report: "The cluster refused the login.\nNothing was changed.",
});

type Asked = { cmd: string; args: Record<string, unknown> };

/** The core, with `open` chats in workspace alpha and `rows` finished under them. */
function core(rows: FinishedTask[], open: OpenChat[] = [STEWARD], refuses?: string) {
  const asked: Asked[] = [];
  let listed = [...rows];
  const chats = [...open];
  /** While set, a read of the finished rows is not answered until it is let go. */
  let unanswered: Promise<void> | undefined;
  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, unknown>;
      asked.push({ cmd, args: a });
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return chats;
      // The layout stored gives each chat open at the launch a tab on alpha (#4758).
      if (cmd === "reopened_layout") return aTabForEachChat(open, () => "alpha");
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: ["devops", "steward"],
          persona: "steward",
          unfiled: [],
          // A list of its own each time, as an answer over the wire is.
          workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats: [...chats] }],
        };
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "stopping_chats") return [];
      if (cmd === "finished_tasks")
        return unanswered === undefined ? listed : unanswered.then(() => listed);
      if (cmd === "clear_finished_tasks") {
        const ids = a.ids as string[];
        const before = listed.length;
        listed = listed.filter((row) => !ids.includes(row.id));
        return before - listed.length;
      }
      if (cmd === "tasks_used")
        // What each ended task's record kept (#1500): the failure kept a figure, the rest none.
        return {
          chats: [],
          finished: ((a.finished as string[]) ?? []).map((id) =>
            id === FAILED.id
              ? { id, tokens: "12k in, 3k out", unsaid: null }
              : { id, tokens: null, unsaid: "nothing" },
          ),
          total: null,
        };
      if (cmd === "reopen_finished_task") {
        if (refuses !== undefined) throw new Error(refuses);
        const row = listed.find((one) => one.id === a.id);
        listed = listed.filter((one) => one.id !== a.id);
        // An ordinary chat: nobody asked for it.
        const chat: OpenChat = {
          ...STEWARD,
          session: 21,
          name: "21",
          in_front: false,
          persona: row?.persona ?? null,
          label: row?.name ?? null,
          resumed: "9f2c-the-conversation",
          from: null,
        };
        chats.push(chat);
        return chat;
      }
      return null;
    },
    { shouldMockEvents: true },
  );
  return {
    /** Holds every read of the finished rows from now, and answers the way to let them go. */
    holdFinished: () => {
      let letGo = () => {};
      unanswered = new Promise((resolve) => {
        letGo = resolve;
      });
      return () => {
        unanswered = undefined;
        letGo();
      };
    },
    /** purlis ends task chat `session`: it is no longer listed, and `row` is its finished row. */
    end: (session: number, row: FinishedTask) => {
      chats.splice(
        chats.findIndex((chat) => chat.session === session),
        1,
      );
      listed = [...listed, row];
    },
    /** The core says chat `session`'s end is done. */
    stopped: (session: number) =>
      act(() => emit("chat-stop", { plane: PLANE, session, phase: "stopped" })),
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map(({ args }) => args),
    /** Every command that ends, closes or stops a chat, in the order it was asked. */
    ended: () =>
      asked
        .map((one) => one.cmd)
        .filter((cmd) =>
          ["close_session", "close_chat_stopping", "smart_close", "stop_chat"].includes(cmd),
        ),
  };
}

const section = () => findTree("Chats of this project");
/**
 * The Chats tree, once steward 4's finished tasks are rows of it (#3198): they are rows under
 * the chat that asked, no group of their own.
 */
const theirs = async () => {
  const tree = await section();
  await waitFor(() => expect(finishedRows(tree).length + foldRows(tree).length).toBeGreaterThan(0));
  return tree;
};
/** Every finished task's row drawn in `tree`. */
const finishedRows = (tree: HTMLElement) =>
  rowsOf(tree).filter((one) => one.classList.contains("finished-task"));
/** The "Finished (n)" rows drawn in `tree`. */
const foldRows = (tree: HTMLElement) =>
  rowsOf(tree).filter((one) => one.classList.contains("finished-count"));
/** The finished row named `name`, its report opened by a press or Enter on it, or none. */
const finishedRow = (tree: HTMLElement, name: string) =>
  finishedRows(tree).find((one) => one.getAttribute("aria-label") === name);

/** The same, where it must be there. */
const theRow = (tree: HTMLElement, name: string) => {
  const found = finishedRow(tree, name);
  if (found === undefined) throw new Error(`no finished row is named ${name}`);
  return found;
};

const tabNames = () =>
  within(stripNamed("Tabs"))
    .queryAllByRole("tab")
    .map((tab) => tab.querySelector(".tab-name")?.textContent);

beforeEach(() => {
  globalThis.localStorage.clear();
  forgetThisLaunch();
});
afterEach(() => {
  cleanup();
  clearMocks();
});

describe("a chat's finished tasks", () => {
  it("shows five done tasks as one Finished (5) line and a failed one as a row of its own", async () => {
    core([...FIVE_DONE, FAILED]);
    render(<App />);
    const group = await theirs();

    // One line for the five, and none of them drawn until it is opened.
    const fold = rowNamed(group, "Finished (5)");
    expect(fold).toHaveAttribute("aria-expanded", "false");
    expect(finishedRow(group, "live check talk")).toBeUndefined();
    // The failure is never behind the count: its own row, with its word.
    const failed = theRow(group, "check staging");
    expect(failed).toHaveTextContent("failed");
    expect(within(group).getByRole("button", { name: "Clear finished" })).toBeInTheDocument();

    // Opened, the five are there, each with how it ended.
    await userEvent.click(fold);
    expect(fold).toHaveAttribute("aria-expanded", "true");
    for (const task of FIVE_DONE) expect(heard(theRow(group, task.name))).toContain("done");
    // None of them is a chat: the tree lists the one that is.
    expect(rowsOf(group).filter((one) => one.classList.contains("chat"))).toHaveLength(1);
  });

  it("folds a cancelled task with the done ones, and keeps every other end out of the fold", async () => {
    core([
      finished("01K6A", "counted", { how: "done", outcome: "done" }),
      finished("01K6B", "called off", { how: "cancelled", outcome: "cancelled" }),
      finished("01K6C", "stuck", { how: "blocked", outcome: "blocked", folds: false }),
      finished("01K6D", "died", {
        how: "unreported",
        outcome: "ended without a report",
        folds: false,
      }),
      finished("01K6E", "shut", {
        how: "closed_by_person",
        outcome: "closed by you",
        folds: false,
      }),
      finished("01K6H", "halted", {
        how: "stopped_by_person",
        outcome: "stopped by you",
        folds: false,
      }),
    ]);
    render(<App />);
    const group = await theirs();

    expect(rowNamed(group, "Finished (2)")).toBeInTheDocument();
    expect(heard(theRow(group, "stuck"))).toContain("blocked");
    expect(heard(theRow(group, "died"))).toContain("ended without a report");
    // The two ends the person caused are rows of their own, never in the fold (V100-9).
    expect(heard(theRow(group, "shut"))).toContain("closed by you");
    expect(heard(theRow(group, "halted"))).toContain("stopped by you");
    expect(finishedRow(group, "counted")).toBeUndefined();
    expect(finishedRow(group, "called off")).toBeUndefined();
  });

  it("says how a finished task ended in the word and the shape its chat's row said it in", async () => {
    // One task that has reported and is open for a moment yet, and the same ends as finished
    // rows: both are drawn from the one function (#1484), so one vocabulary is on screen.
    const reported: OpenChat = {
      ...WORKING,
      label: "probe",
      from: { ...WORKING.from, reported: true, outcome: "blocked" } as OpenChat["from"],
    };
    core(
      [
        finished("01K6A", "counted", { how: "done", outcome: "done" }),
        finished("01K6B", "called off", { how: "cancelled", outcome: "cancelled" }),
        finished("01K6C", "stuck", { how: "blocked", outcome: "blocked", folds: false }),
        finished("01K6D", "died", {
          how: "unreported",
          outcome: "ended without a report",
          folds: false,
        }),
        finished("01K6E", "shut", {
          how: "closed_by_person",
          outcome: "closed by you",
          folds: false,
        }),
        finished("01K6H", "halted", {
          how: "stopped_by_person",
          outcome: "stopped by you",
          folds: false,
        }),
        finished("01K6F", "broke", { how: "failed", outcome: "failed", folds: false }),
        // A value from a later purlis, which this window's types do not know.
        finished("01K6G", "odd", {
          how: "paused" as FinishedTask["how"],
          outcome: "paused by a rule",
          folds: false,
        }),
      ],
      [STEWARD, reported],
    );
    render(<App />);
    const group = await theirs();
    await userEvent.click(rowNamed(group, "Finished (2)"));

    /** The word and the shape on a row, and the core's own word where it is drawn too. */
    const says = (on: HTMLElement) => ({
      word: on.querySelector(".shown-state .word")?.textContent,
      shape: on.querySelector(".shown-state .shape")?.getAttribute("data-shape"),
      // The core's own word, after the state's in what the row says beside its name (#3198).
      more: on.getAttribute("aria-description")?.split(", ")[1],
    });
    expect(says(theRow(group, "counted"))).toEqual({ word: "done", shape: "tick" });
    expect(says(theRow(group, "called off"))).toEqual({ word: "cancelled", shape: "dash" });
    expect(says(theRow(group, "broke"))).toEqual({ word: "failed", shape: "cross" });
    expect(says(theRow(group, "died"))).toEqual({
      word: "ended without a report",
      shape: "triangle",
    });
    // The one end the vocabulary has no word of its own for keeps the core's word beside
    // the state's, so it is not said less exactly than before.
    expect(says(theRow(group, "stuck"))).toEqual({
      word: "failed",
      shape: "cross",
      more: "blocked",
    });
    // **The two ends the person caused have their own words and shapes** (#1488), said once:
    // never "cancelled", which is what the asking chat does, and with no second word beside.
    expect(says(theRow(group, "shut"))).toEqual({ word: "closed by you", shape: "octagon" });
    expect(says(theRow(group, "halted"))).toEqual({ word: "stopped by you", shape: "square" });
    // An end this window does not know is not guessed at: it is drawn as the core said it.
    expect(says(theRow(group, "odd"))).toEqual({
      word: "reported",
      shape: "dot",
      more: "paused by a rule",
    });

    // The task still open says the same of the same end as the finished row does.
    const open = rowsOf(await section()).find(
      (one) => one.querySelector(".session")?.textContent === "probe",
    );
    if (open === undefined) throw new Error("the open task has no row");
    // Its mark is the chat's one state (#5340, #6012): Failed, and its word says so.
    expect(open.querySelector('[data-scope="chat-mark"][data-part="word"]')?.textContent).toBe(
      "failed",
    );
    expect(open.querySelector('[data-scope="chat-mark"][data-part="root"]')).toHaveAttribute(
      "data-variant",
      "failed",
    );
    // **One line, as a chat's row is** (#1687): the state is the mark, and its word, with the
    // core's own beside it, is still the row's to a screen reader, out of sight where it stands,
    // and whole in the row's tooltip.
    for (const name of ["counted", "stuck"]) {
      const one = theRow(group, name);
      for (const said of one.querySelectorAll(".shown-state .word"))
        expect(said.classList.contains("hidden-words")).toBe(true);
    }
    // Named by its task (#3198), and how it ended said after the name.
    expect(heard(theRow(group, "counted"))).toBe("counted done");
    expect(heard(theRow(group, "stuck"))).toBe("stuck failed, blocked");
    expect(tipOf(theRow(group, "stuck"))).toMatch(/^stuck\nfailed · blocked/);
    expect(within(theRow(group, "counted")).queryByRole("img")).toBeNull();
  });

  it("keeps a task's row until its finished row is read, so the rows below move once", async () => {
    const reported: OpenChat = {
      ...WORKING,
      label: "probe",
      // Failed, so its session stays open over it and its row is on screen.
      from: { ...WORKING.from, reported: true, outcome: "failed" } as OpenChat["from"],
    };
    const other: OpenChat = { ...STEWARD, session: 12, name: "12", in_front: false };
    const held = core([], [STEWARD, reported, other]);
    render(<App />);
    const tree = await section();
    const names = () =>
      rowsOf(tree)
        .filter((one) => one.classList.contains("chat"))
        .map((one) => one.querySelector(".session")?.textContent);
    await waitFor(() => expect(names()).toEqual(["steward 4", "probe", "steward 12"]));

    // purlis ends the task: it leaves the list of chats, and its finished row is one read
    // later. That read is not answered yet.
    const answer = held.holdFinished();
    held.end(
      9,
      finished("01K6PROBE", "probe", { chat: 9, how: "failed", outcome: "failed", folds: false }),
    );
    await held.stopped(9);

    await waitFor(() => expect(held.asked("plane_sidebar").length).toBeGreaterThan(1));
    expect(names()).toEqual(["steward 4", "probe", "steward 12"]);
    expect(finishedRows(tree)).toEqual([]);

    // The finished rows are read: the chat's row goes and the row that replaces it comes,
    // together.
    answer();

    const group = await theirs();
    expect(heard(theRow(group, "probe"))).toContain("failed");
    expect(names()).toEqual(["steward 4", "steward 12"]);
  });

  it("clears the finished rows and nothing else: the failure stays, and no chat is touched", async () => {
    const said = core([...FIVE_DONE, FAILED]);
    render(<App />);
    const group = await theirs();

    await userEvent.click(within(group).getByRole("button", { name: "Clear finished" }));

    // The five rows, by their records' ids, in one ask.
    await waitFor(() =>
      expect(said.asked("clear_finished_tasks")).toEqual([
        { plane: PLANE, ids: FIVE_DONE.map((task) => task.id) },
      ]),
    );
    await waitFor(() => expect(foldRows(group)).toEqual([]));
    expect(within(group).queryByRole("button", { name: "Clear finished" })).toBeNull();
    // The failed row is still its own row, until it is cleared itself.
    expect(theRow(group, "check staging")).toBeInTheDocument();
    // Rows only: nothing was closed, stopped or reopened, and the steward chat is as it was.
    expect(said.ended()).toEqual([]);
    expect(said.asked("reopen_finished_task")).toEqual([]);
    expect(tabNames()).toEqual(["steward 4"]);

    await userEvent.click(within(group).getByRole("button", { name: "Clear check staging" }));

    await waitFor(() =>
      expect(said.asked("clear_finished_tasks")[1]).toEqual({ plane: PLANE, ids: [FAILED.id] }),
    );
    await waitFor(() => expect(finishedRows(group)).toEqual([]));
  });

  it("says what a finished task used on its name's hover, read once the pointer rests (#1500)", async () => {
    const { asked } = core([
      FAILED,
      finished("01K6WAITS", "start again", {
        how: "failed",
        outcome: "failed",
        folds: false,
        report: "It could not start.",
        waits: { approval: null },
      }),
    ]);
    render(<App />);
    const group = await theirs();
    const name = theRow(group, "check staging");

    fireEvent.pointerEnter(name);
    // Its whole name first, cut short in a narrow sidebar, then how it ended and what it used.
    await waitFor(() => expect(asked("tasks_used")).toHaveLength(1));
    await act(async () => {});
    expect(tipOf(name)).toBe(
      "check staging\nfailed\nThe cluster refused the login.\nTokens: 12k in, 3k out",
    );
    expect(asked("tasks_used")).toEqual([
      { plane: PLANE, scope: "hover", own: null, chats: [], finished: [FAILED.id] },
    ]);

    // A task still waiting to start has no record to read, and nothing is asked for it.
    const waiting = theRow(group, "start again");
    fireEvent.pointerEnter(waiting);
    await new Promise((resolve) => setTimeout(resolve, 400));
    expect(asked("tasks_used")).toHaveLength(1);
    expect(tipOf(waiting)).toBe("start again\nfailed\nIt could not start.");
  });

  it("shows a finished row's report on a press, as text and never as markup", async () => {
    core([
      finished("01K6X", "check prod", {
        how: "failed",
        outcome: "failed",
        folds: false,
        report:
          '<b>Healthy</b> <img src=x onerror="alert(1)">\n# Not a heading\n[a link](https://example.com)',
        changed: "values.yaml: **replicas** 2 to 3",
        branch: "check-prod-b5rc0def",
      }),
    ]);
    render(<App />);
    const group = await theirs();
    const name = theRow(group, "check prod");
    // Its first line on hover, after its name and how it ended; nothing of it drawn until it is
    // opened.
    expect(tipOf(name)).toEqual(
      'check prod\nfailed\n<b>Healthy</b> <img src=x onerror="alert(1)">',
    );
    expect(screen.queryByRole("region", { name: "Report of check prod" })).toBeNull();

    await userEvent.click(name);

    const report = screen.getByRole("region", { name: "Report of check prod" });
    // Every character as written, line breaks and all.
    expect(report.querySelector(".report-text")?.textContent).toBe(
      '<b>Healthy</b> <img src=x onerror="alert(1)">\n# Not a heading\n[a link](https://example.com)',
    );
    expect(report).toHaveTextContent("Changed: values.yaml: **replicas** 2 to 3");
    expect(report).toHaveTextContent("alpha · own branch check-prod-b5rc0def");
    // And nothing in it became an element.
    expect(report.querySelector("b, img, h1, a, strong")).toBeNull();

    await userEvent.click(name);
    expect(screen.queryByRole("region", { name: "Report of check prod" })).toBeNull();
  });

  it("reopens a finished task as an ordinary chat in the focused pane, and its row goes", async () => {
    const said = core([FAILED]);
    render(<App />);
    const group = await theirs();
    expect(tabNames()).toEqual(["steward 4"]);

    await userEvent.click(within(group).getByRole("button", { name: "Reopen check staging" }));

    await waitFor(() =>
      expect(said.asked("reopen_finished_task")).toEqual([
        { plane: PLANE, id: FAILED.id, columns: expect.any(Number), rows: expect.any(Number) },
      ]),
    );
    // It takes the focused pane, as any chat started there does (#4758), and its tab is named
    // as the task was; the chat that asked runs on in the sidebar.
    await waitFor(() => expect(tabNames()).toEqual(["check staging"]));
    // Its row has gone: it is a chat now, and no finished task.
    await waitFor(() => expect(finishedRows(group)).toEqual([]));
    // And nothing was ended to make it.
    expect(said.ended()).toEqual([]);
    expect(said.asked("clear_finished_tasks")).toEqual([]);
  });

  it("says why on the row when a task cannot be reopened, and offers no Reopen where there is no conversation", async () => {
    const said = core(
      [FAILED, finished("01K6N", "no conversation", { folds: false, reopens: false })],
      [STEWARD],
      "'check staging' cannot be reopened: the folder it worked in is gone (workspaces/alpha). Its report is still here to read.",
    );
    render(<App />);
    const group = await theirs();

    // Reopen where there is nothing to resume is still a button the keyboard reaches, and
    // says why to whoever lands on it. Pressing it asks the core nothing.
    const cannot = within(group).getByRole("button", { name: "Reopen no conversation" });
    expect(cannot).not.toBeDisabled();
    expect(cannot).toHaveAttribute("aria-disabled", "true");
    // Its tooltip, which the keyboard's focus opens and which describes it while it is shown:
    // read as the keyboard reaches it, along its row (#3198).
    act(() => theRow(group, "no conversation").focus());
    expect(tipOf(cannot)).toBe(
      "It cannot be reopened: its harness named no conversation to resume.",
    );
    cannot.focus();
    expect(cannot).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    expect(said.asked("reopen_finished_task")).toEqual([]);
    await userEvent.click(within(group).getByRole("button", { name: "Reopen check staging" }));

    expect(await within(group).findByRole("alert")).toHaveTextContent(
      "'check staging' cannot be reopened: the folder it worked in is gone (workspaces/alpha). Its report is still here to read.",
    );
    // The row is still there, and no tab was opened.
    expect(theRow(group, "check staging")).toBeInTheDocument();
    expect(tabNames()).toEqual(["steward 4"]);
  });

  it("draws them under the chat that asked, after its running tasks, and folds them away with it", async () => {
    core([...FIVE_DONE, FAILED], [STEWARD, WORKING]);
    render(<App />);
    const tree = await theirs();

    // Rows of the tree under the steward chat, after the task still working under it.
    const working = rowNamed(tree, /read the logs/);
    const first = theRow(tree, "check staging");
    expect(first).toHaveAttribute("aria-level", working.getAttribute("aria-level"));
    expect(working.compareDocumentPosition(first) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

    // Folding the steward chat's row hides everything under it.
    const steward = rowNamed(tree, /steward 4/);
    steward.focus();
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() => expect(finishedRows(tree)).toEqual([]));
    await userEvent.keyboard("{ArrowRight}");
    await waitFor(() => expect(finishedRow(tree, "check staging")).toBeDefined());
  });

  // **Decided with the move (#3198, decision 1)**: a finished task is a row of the tree, and →
  // reaches its buttons, as a row's controls are reached in the tree family's grid; the arrows
  // walk the rows. Before, each was a Tab stop of its own.
  it("is worked by the keyboard: the arrows reach the rows, → their buttons, Enter opens the fold and its report, and its menu clears it", async () => {
    const said = core([...FIVE_DONE, FAILED]);
    render(<App />);
    const group = await theirs();
    const fold = rowNamed(group, "Finished (5)");
    const failed = theRow(group, "check staging");

    // From the row that stands alone, → goes through its buttons in the order they are drawn:
    // its Brief (#1494), Reopen, Changes (#1511) and Clear.
    act(() => failed.focus());
    await userEvent.keyboard("{ArrowRight}");
    expect(within(group).getByRole("button", { name: "Brief of check staging" })).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    expect(within(group).getByRole("button", { name: "Reopen check staging" })).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    expect(within(group).getByRole("button", { name: "Changes of check staging" })).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    expect(within(group).getByRole("button", { name: "Clear check staging" })).toHaveFocus();
    // ↓ goes on to the next row, the fold.
    await userEvent.keyboard("{ArrowDown}");
    expect(fold).toHaveFocus();

    // Enter opens it, ← shuts it, and the keyboard stays on it.
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(fold).toHaveAttribute("aria-expanded", "true"));
    expect(theRow(group, "live check talk")).toBeInTheDocument();
    expect(fold).toHaveFocus();
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() => expect(fold).toHaveAttribute("aria-expanded", "false"));
    expect(finishedRow(group, "live check talk")).toBeUndefined();
    expect(fold).toHaveFocus();

    // Clear finished is in its menu, which Shift+F10 opens, as at its end for the pointer: →
    // on a row with rows under it opens it or goes into it, and never reaches its buttons.
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    await userEvent.click(await screen.findByRole("menuitem", { name: "Clear finished" }));
    await waitFor(() =>
      expect(said.asked("clear_finished_tasks")).toEqual([
        { plane: PLANE, ids: FIVE_DONE.map((task) => task.id) },
      ]),
    );
    await waitFor(() => expect(foldRows(group)).toEqual([]));
    // The rows have gone, and the keyboard is not stranded: the row that stays is still
    // there, and its report opens on Enter.
    const stays = theRow(group, "check staging");
    act(() => stays.focus());
    await userEvent.keyboard("{Enter}");
    expect(screen.getByRole("region", { name: "Report of check staging" })).toBeInTheDocument();
  });

  it("says on the row why the last Reopen did not hold, in the core's sentence", async () => {
    const why =
      "It could not be reopened: its harness ended at once, without bringing the conversation back (it may have been removed). Its report is still here.";
    core([{ ...FAILED, not_reopened: why }]);
    render(<App />);
    const group = await theirs();

    expect(within(group).getByText(why)).toBeInTheDocument();
    // And Reopen is still offered: the next try may hold.
    expect(within(group).getByRole("button", { name: "Reopen check staging" })).not.toHaveAttribute(
      "aria-disabled",
    );
  });

  it("draws nothing for a chat with no finished tasks", async () => {
    core([]);
    render(<App />);
    await section();

    await waitFor(() => expect(screen.getAllByTestId("pane").length).toBeGreaterThan(0));
    expect(finishedRows(await section())).toEqual([]);
    expect(foldRows(await section())).toEqual([]);
  });
});
