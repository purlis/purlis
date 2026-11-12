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
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { FinishedTask, Moved, OpenChat } from "./bindings";
import { setChatsListPrefs } from "./chatsListPrefs";
import { cardOf, theCard } from "./chatCard.testkit";
import type { State } from "./chatState";
import { forgetThisLaunch } from "./regions";
import { tipOf } from "./ui/tooltip/tooltip.testkit";
import { heard } from "./frame.testkit";
import { aBatchOf } from "./chatEvents.testkit";
import { CHAT_EVENTS } from "./chatEvents";
import { aTabForEachChat } from "./laidOut.testkit";
import { measuring } from "./stripRoom.testkit";
import { findTree, keysOn, queryTree, rowsOf, treeNamed } from "./ui/tree/tree.testkit";

/**
 * **The Chats list at fifty chats, against the whole window** (#1499, V100-47 to V100-50): the
 * order its sessions stand in and when it is held still, the folds it makes by itself, the
 * filter, a row's one line and its card, its keys, and what a chat moving redraws.
 *
 * The harness is `ChatsSection.window.test.tsx`'s: the core's answers as fixtures, and its
 * events sent by hand.
 */

/** Every draw of a row, by its chat: a row asks for its task's tokens each time it is drawn
 *  (`useTokensOnHover`, which reads nothing until the card is up). */
const drawn = vi.hoisted(() => ({ rows: [] as number[] }));

vi.mock("./tasksUsed", async (original) => {
  const real = await original<typeof import("./tasksUsed")>();
  return {
    ...real,
    useTokensOnHover: (what: Parameters<typeof real.useTokensOnHover>[0]) => {
      if ("chat" in what) drawn.rows.push(what.chat);
      return real.useTokensOnHover(what);
    },
  };
});

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const PLANE = "/home/dev/plane";

type Lineage = NonNullable<OpenChat["from"]>;
type Filed = OpenChat & { workspace: string };

/** A chat working in `workspace`, as the core lists it. */
function chat(session: number, workspace: string, more: Partial<OpenChat> = {}): Filed {
  return {
    session,
    name: String(session),
    cwd: `${PLANE}/workspaces/${workspace}`,
    harness: "claude",
    in_front: session === 1,
    resumed: null,
    fresh: null,
    profile: null,
    persona: "steward",
    unreported: null,
    card: null,
    guessed: null,
    pinned: false,
    label: null,
    from: null,
    ...more,
    workspace,
  };
}

/** A task chat `asker` asked for, which the person saw as `name` in `workspace`. */
function taskOf(asker: number, workspace = "alpha", name = `steward ${asker}`): Lineage {
  return {
    chat: asker,
    name,
    workspace,
    task: true,
    tab: false,
    reported: false,
    unreported: false,
  };
}

/** The core's typed end for each of its words for one (`FinishedTask.how`, #1485). */
const HOW: Record<string, FinishedTask["how"]> = {
  done: "done",
  cancelled: "cancelled",
  blocked: "blocked",
  failed: "failed",
  "ended without a report": "unreported",
  "closed by you": "closed_by_person",
  "stopped by you": "stopped_by_person",
};

function finished(id: string, asker: number, more: Partial<FinishedTask> = {}): FinishedTask {
  return {
    how: HOW[more.outcome ?? "done"],
    chat: null,
    not_reopened: null,
    did_not_start: false,
    attempts: 0,
    waits: null,
    id,
    asker,
    name: `check ${id}`,
    persona: "devops",
    outcome: "done",
    folds: true,
    report: "All good.",
    changed: null,
    ended: "2026-10-08T12:04:30+00:00",
    place: "alpha",
    branch: null,
    reopens: true,
    ...more,
  };
}

/** A fixture's chat as the core sends it: the workspace is where it is filed, not a field. */
function asListed(one: Filed): OpenChat {
  const sent: OpenChat & { workspace?: string } = { ...one };
  delete sent.workspace;
  return sent;
}

type Asked = { cmd: string; args: Record<string, unknown> };

/** The core, holding `open` chats in two workspaces and `ended` finished tasks. */
function core(open: Filed[], ended: FinishedTask[] = []) {
  const asked: Asked[] = [];
  const listeners = new Map<string, number>();
  /** Every listener for the chat events: each chat store, and the asks, listen for them. */
  const batches: number[] = [];
  let sequence = 0;
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: a });
    if (cmd === "plugin:event|listen") {
      const { event, handler } = args as { event: string; handler: number };
      listeners.set(event, handler);
      if (event === CHAT_EVENTS) batches.push(handler);
      return 1;
    }
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "opened_chats") return open.map(asListed);
    // The layout stored gives a tab to each chat its lineage says has one (#4758): a task has
    // none until the person opens it.
    if (cmd === "reopened_layout") {
      const tabbed = open.filter((one) => one.from?.tab !== false);
      const laid = aTabForEachChat(
        tabbed,
        (session) => open.find((one) => one.session === session)?.workspace ?? null,
      );
      return {
        ...laid,
        tabs: laid.tabs.map((tab, at) => ({ ...tab, pinned: tabbed[at]?.pinned ?? false })),
      };
    }
    if (cmd === "open_chat_tab") {
      const one = open.find((chat) => chat.session === a.session);
      if (one?.from) one.from = { ...one.from, tab: true };
      return null;
    }
    if (cmd === "stopping_chats") return [];
    // What ending a task would do (#1488): here every task is in the middle of a turn.
    if (cmd === "task_ending")
      return {
        name: "devops 2",
        working: true,
        no_report: null,
        below: [],
        stopping: false,
        reported: false,
      };
    if (cmd === "finished_tasks") return ended;
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        personas: ["steward", "devops"],
        persona: "steward",
        unfiled: [],
        workspaces: ["alpha", "beta"].map((name) => ({
          name,
          path: `${PLANE}/workspaces/${name}`,
          vision: "",
          todos: [],
          colour: null,
          live: false,
          chats: open.filter((chat) => chat.workspace === name).map(asListed),
        })),
      };
    if (cmd === "workspace_panels")
      return {
        workspace: a.workspace,
        repos: [],
        paths: {},
        absent: [],
        refused: [],
        todos: [],
        todos_refused: null,
        personas: ["steward", "devops"],
        persona: "steward",
      };
    if (cmd === "workspace_repos")
      return { workspace: a.workspace, repos: [], cache_refused: null };
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
    return null;
  });
  const send = (event: string, payload: unknown) => {
    const handler = listeners.get(event);
    if (handler === undefined) throw new Error(`the window is not listening for ${event}`);
    act(() => {
      heard(handler, { event, id: 1, payload });
    });
  };
  return {
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args),
    /** The core says a chat another chat started has arrived. */
    arrive: (one: Filed) => {
      open.push(one);
      send("handoff-arrived", {
        plane: PLANE,
        session: one.session,
        name: one.name,
        label: one.label,
        from: one.from,
        workspace: one.workspace,
        persona: one.persona,
        harness: one.harness,
      });
    },
    /** The core says chat `session` was stopped and has ended: it is listed no more. */
    end: (session: number) => {
      open.splice(
        open.findIndex((chat) => chat.session === session),
        1,
      );
      send("chat-stop", { plane: PLANE, session, phase: "stopped" });
    },
    /** The core says chat `session` moved to `state`, with `queue` asking for the person. */
    move: (session: number, state: State, queue: number[] = [], more: Partial<Moved> = {}) => {
      const handlers = batches;
      if (handlers.length === 0) throw new Error("the window is not listening for moves");
      sequence += 1;
      const moved: Moved = {
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
        ...more,
      };
      act(() => {
        for (const handler of handlers)
          heard(handler, { event: CHAT_EVENTS, id: 1, payload: aBatchOf("moved", moved) });
      });
    },
  };
}

/** The list, with every workspace's chats on it: these read chats in alpha and beta at once,
 *  which the list shows on the person's word (#1655). */
const section = async () => {
  const tree = await findTree("Chats of this project");
  const every = screen.getByRole("radio", { name: "All" });
  if (!(every as HTMLInputElement).checked) fireEvent.click(every);
  return tree;
};
const theTree = () => treeNamed("Chats of this project");
/** The list the pointer rests over: the tree and what holds it. */
const list = () => {
  const found = theTree().closest<HTMLElement>(".chats-list");
  if (found === null) throw new Error("the tree is not in a list");
  return found;
};

/** Each row of the section as `level name`, top to bottom. */
const shape = () =>
  rowsOf(theTree()).map(
    (row) => `${row.getAttribute("aria-level")} ${row.querySelector(".session")?.textContent}`,
  );

const row = (name: string) => {
  const found = rowsOf(theTree()).find(
    (one) => one.querySelector(".session")?.textContent === name,
  );
  if (found === undefined) throw new Error(`no row is named ${name}`);
  return found;
};

/** Where a row says it stands among its siblings: `posinset/setsize`. */
const place = (name: string) =>
  `${row(name).getAttribute("aria-posinset")}/${row(name).getAttribute("aria-setsize")}`;

const box = () =>
  screen.getByRole("searchbox", { name: "Filter chats by name, persona, workspace or state" });
/**
 * Types into the filter's box, with the keyboard brought to it and no pointer: jsdom lays
 * nothing out, so a pointer pressed anywhere lands on the regions' divider, which takes the
 * keyboard as it would for a drag.
 */
const typed = async (text: string) => {
  act(() => box().focus());
  await userEvent.keyboard(text);
};
/** An animation on `mark` ends, by whichever name this engine gives the event. */
const over = (mark: Element | null) => {
  if (mark === null) throw new Error("no mark to end the animation of");
  act(() => {
    for (const name of ["animationend", "webkitAnimationEnd"])
      mark.dispatchEvent(new Event(name, { bubbles: true }));
  });
};
/** The chat drawn in the pane in front: a task is shown in the pane it returns to (#1486). */
const shownInFront = () => screen.getByTestId("pane").textContent;
/** The word a row says its state in. */
const word = (name: string) =>
  row(name).querySelector('[data-scope="chat-mark"][data-part="word"]')?.textContent;
const hiddenSaid = () => document.querySelector(".chats-hidden")?.textContent;
/** The small count a folded row says of its tasks (#1675), as it is drawn. */
const folded = (name: string) =>
  row(name).querySelector('.task-count [aria-hidden="true"]')?.textContent;

/** Three sessions, the first with a task, and each made to do what a test says. */
const threeSessions = () => [
  chat(1, "alpha"),
  chat(2, "alpha"),
  chat(3, "beta"),
  chat(4, "alpha", { persona: "devops", from: taskOf(1) }),
];

beforeEach(() => {
  globalThis.localStorage.clear();
  forgetThisLaunch();
});
afterEach(() => {
  cleanup();
  clearMocks();
  drawn.rows.length = 0;
});

describe("the order of the Chats list (V100-47)", () => {
  /** Session 3 needs the person, 2 is at work, 1 and its task are idle. */
  const settled = async () => {
    const held = core(threeSessions());
    render(<App />);
    await section();
    await waitFor(() => expect(shape()).toHaveLength(4));
    held.move(1, "waiting");
    held.move(4, "waiting");
    held.move(2, "running");
    held.move(3, "waiting", [3]);
    return held;
  };

  it("puts the sessions that need the person first, then the ones at work, then the idle", async () => {
    await settled();

    expect(shape()).toEqual(["1 steward 3", "1 steward 2", "1 steward 1", "2 devops 4"]);
    // Each row says where it now stands, to a screen reader.
    expect(place("steward 3")).toBe("1/3");
    expect(place("steward 2")).toBe("2/3");
    expect(place("steward 1")).toBe("3/3");
    expect(place("devops 4")).toBe("1/1");
  });

  it("puts a session whose task needs the person with the ones that need the person", async () => {
    const { move } = await settled();

    move(4, "waiting", [3, 4]);

    expect(shape()).toEqual(["1 steward 1", "2 devops 4", "1 steward 3", "1 steward 2"]);
  });

  it("does not move a row while the pointer rests over the list, and does when it leaves", async () => {
    const { move } = await settled();
    const before = shape();

    await userEvent.hover(list());
    move(1, "waiting", [3, 1]);
    move(2, "waiting", [3, 1]);

    expect(shape()).toEqual(before);
    // The state itself is said at once: only the order waits.
    expect(word("steward 1")).toBe("needs you");
    // No row moved under the click: what is pressed is what was under the pointer.
    await userEvent.click(row("devops 4"));
    await waitFor(() => expect(shownInFront()).toBe("session 4"));
    expect(shape()).toEqual(before);

    act(() => row("devops 4").blur());
    await userEvent.unhover(list());

    expect(shape()).toEqual(["1 steward 1", "2 devops 4", "1 steward 3", "1 steward 2"]);
  });

  it("does not move a row while the keyboard is in the list, and does when it leaves", async () => {
    const { move } = await settled();
    const before = shape();

    // The keyboard comes in: an arrow, from the first row to the second.
    act(() => row("steward 3").focus());
    await userEvent.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(row("steward 2"));
    move(1, "waiting", [3, 1]);

    expect(shape()).toEqual(before);
    // The arrows walk the rows as they stand.
    await userEvent.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(row("steward 1"));

    act(() => row("steward 1").blur());

    expect(shape()).toEqual(["1 steward 1", "2 devops 4", "1 steward 3", "1 steward 2"]);
  });

  it("lets the keyboard's hold go when the row the keyboard was on is taken away", async () => {
    const { move, end } = await settled();

    // The keyboard is on the task's row, which holds the list.
    act(() => row("steward 1").focus());
    await userEvent.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(row("devops 4"));
    // The task is stopped and ends: its row goes, and no blur says the keyboard left.
    end(4);
    await waitFor(() => expect(shape()).toHaveLength(3));

    move(1, "waiting", [3, 1]);

    // Not held for good: the order follows the chats again.
    expect(shape()).toEqual(["1 steward 1", "1 steward 3", "1 steward 2"]);
  });

  it("does not move a row while the keyboard is in a row's menu (#3594)", async () => {
    const { move } = await settled();
    const before = shape();

    act(() => row("steward 3").focus());
    await userEvent.keyboard("{ArrowDown}");
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const menu = await screen.findByRole("menu");
    await waitFor(() => expect(menu.contains(document.activeElement)).toBe(true));
    move(1, "waiting", [3, 1]);

    expect(shape()).toEqual(before);

    // The menu closed and gave the keyboard back to its row; once that leaves, the rows move.
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    act(() => (document.activeElement as HTMLElement).blur());

    expect(shape()).toEqual(["1 steward 1", "2 devops 4", "1 steward 3", "1 steward 2"]);
  });

  it("does not move a row while the pointer is on a row's card, which stands beside the list", async () => {
    const { move } = await settled();
    const before = shape();

    await userEvent.hover(row("steward 3"));
    const shown = await theCard();
    // Off the row and straight onto its card, as a pointer reading it goes (WCAG 1.4.13).
    await userEvent.hover(shown);
    move(1, "waiting", [3, 1]);

    expect(shape()).toEqual(before);
  });

  it("is not held by a row a pointer's press focused, once the pointer has left", async () => {
    const { move } = await settled();

    // Focused, and not by the keyboard: what a click does on the WebViews that focus a button.
    act(() => row("steward 2").focus());
    move(1, "waiting", [3, 1]);

    expect(shape()).toEqual(["1 steward 1", "2 devops 4", "1 steward 3", "1 steward 2"]);
  });

  it("lets go of what it holds when the window stops being the one in use", async () => {
    const { move } = await settled();
    const before = shape();
    await userEvent.hover(list());
    move(1, "waiting", [3, 1]);
    expect(shape()).toEqual(before);

    // The person went to another app with the pointer parked over the list: no leave is sent.
    act(() => {
      window.dispatchEvent(new Event("blur"));
    });

    expect(shape()).toEqual(["1 steward 1", "2 devops 4", "1 steward 3", "1 steward 2"]);
  });

  it("does not draw a task that arrives above a resting pointer until the pointer leaves", async () => {
    const { arrive } = await settled();
    const before = shape();

    await userEvent.hover(list());
    arrive(chat(5, "alpha", { persona: "devops", from: taskOf(3, "beta", "steward 3") }));
    await act(async () => {});

    // Steward 3 is the first row: a row drawn under it would move every row below.
    expect(shape()).toEqual(before);

    await userEvent.unhover(list());

    await waitFor(() =>
      expect(shape()).toEqual([
        "1 steward 3",
        "2 devops 5",
        "1 steward 2",
        "1 steward 1",
        "2 devops 4",
      ]),
    );
  });

  it("knocks once for a chat that starts needing the person, and not again when rows move past it", async () => {
    const { move } = await settled();
    const state = (name: string) => row(name).querySelector('[data-part="badge"]');
    move(2, "waiting", [3, 2]);
    expect(state("steward 2")).toHaveAttribute("data-arrived");

    // The knock plays, and is over.
    over(state("steward 2"));
    expect(state("steward 2")).not.toHaveAttribute("data-arrived");

    // Another session comes to need the person and is put above it: the row is moved in the
    // document, which would play a class left on it a second time.
    move(1, "waiting", [3, 2, 1]);
    expect(shape()).toEqual(["1 steward 1", "2 devops 4", "1 steward 2", "1 steward 3"]);
    expect(state("steward 2")).not.toHaveAttribute("data-arrived");
    expect(row("steward 2").isConnected).toBe(true);

    // It knocks again only when it starts needing the person again.
    move(2, "running", [3, 1]);
    move(2, "waiting", [3, 1, 2]);
    expect(state("steward 2")).toHaveAttribute("data-arrived");
  });

  it("groups the sessions by workspace where the person asked for that", async () => {
    await settled();

    act(() => setChatsListPrefs({ grouped: true }));

    expect(shape()).toEqual(["1 steward 2", "1 steward 1", "2 devops 4", "1 steward 3"]);
    expect(
      [...theTree().querySelectorAll(".chats-group-name")].map((one) => one.textContent),
    ).toEqual(["alpha", "beta"]);
    // The names over them are each a group's (#3198): a session is counted among its group's.
    expect(place("steward 2")).toBe("1/2");
    expect(place("steward 3")).toBe("1/1");
  });
});

describe("the folds the Chats list makes by itself (V100-48)", () => {
  /** A task of chat 1 that has reported done: working while its turn runs, done once it is
   *  over, and an open chat until purlis ends it. */
  const reportedDone = () => ({ ...taskOf(1), reported: true, outcome: "done" });
  const twoTasks = () => [
    chat(1, "alpha"),
    chat(2, "alpha", { persona: "devops", from: reportedDone() }),
    chat(3, "alpha", { persona: "devops", from: reportedDone() }),
    chat(4, "alpha"),
  ];
  const up = async (ended: FinishedTask[] = []) => {
    const held = core(twoTasks(), ended);
    render(<App />);
    await section();
    // Both tasks have reported: until their turns are heard to run they read done, and the
    // session is folded over them.
    await waitFor(() => expect(shape()).toHaveLength(2));
    held.move(2, "running");
    held.move(3, "running");
    await waitFor(() => expect(shape()).toHaveLength(4));
    return held;
  };

  it("keeps a session open while a task under it works, and folds it when all are over", async () => {
    const { move } = await up();
    expect(row("steward 1")).toHaveAttribute("aria-expanded", "true");

    // One of them is over, its report sent: the other still works.
    move(2, "done");
    expect(row("steward 1")).toHaveAttribute("aria-expanded", "true");
    move(3, "done");

    expect(row("steward 1")).toHaveAttribute("aria-expanded", "false");
    expect(shape()).toEqual(["1 steward 1", "1 steward 4"]);
  });

  it("opens a session it folded when a task under it needs the person", async () => {
    const { move } = await up();
    move(2, "done");
    move(3, "done");
    expect(shape()).toHaveLength(2);

    move(3, "waiting", [3]);

    expect(row("steward 1")).toHaveAttribute("aria-expanded", "true");
    expect(shape()).toEqual(["1 steward 1", "2 devops 2", "2 devops 3", "1 steward 4"]);
  });

  it("knocks for a task that started needing the person while its session was folded over it", async () => {
    const { move } = await up();
    move(2, "done");
    move(3, "done");
    expect(shape()).toHaveLength(2);

    move(3, "waiting", [3]);

    // Its row is drawn for the first time in that state, and it is still a change: the hand
    // knocks, as it would have on a row that was on screen.
    expect(row("devops 3").querySelector('[data-part="badge"]')).toHaveAttribute("data-arrived");
    expect(word("devops 3")).toBe("needs you");
    // The task beside it is drawn again too, in the state it was folded away in: no motion
    // for a state nothing changed to.
    expect(row("devops 2").querySelector('[data-part="badge"]')).not.toHaveAttribute(
      "data-arrived",
    );
  });

  it("keeps a session open over an open task that is blocked, and counts it when folded by hand", async () => {
    // A blocked task is not ended (#1485): it stays an open chat that reads failed, waiting
    // on something. The list never folds it away by itself.
    const { move } = core([
      chat(1, "alpha"),
      chat(2, "alpha", {
        persona: "devops",
        from: { ...taskOf(1), reported: true, outcome: "blocked" },
      }),
      chat(3, "alpha", { persona: "devops", from: reportedDone() }),
    ]);
    render(<App />);
    await section();
    await waitFor(() => expect(shape()).toHaveLength(3));
    move(2, "waiting");
    move(3, "waiting");

    expect(word("devops 2")).toBe("failed");
    expect(word("devops 3")).toBe("done");
    expect(row("steward 1")).toHaveAttribute("aria-expanded", "true");
    expect(shape()).toEqual(["1 steward 1", "2 devops 2", "2 devops 3"]);

    // Folded by the person, the row counts what it hides, and its card says how each ended.
    await keysOn(row("steward 1"), "{ArrowLeft}");
    expect(row("steward 1")).toHaveAttribute("aria-expanded", "false");
    expect(folded("steward 1")).toBe("2");
    expect((await cardOf(row("steward 1"))).facts.Tasks).toBe("1 failed · 1 done");
  });

  it("counts the open tasks that are done on the row it folds over them by itself", async () => {
    // Done and not ended yet (the person is reading one): in no finished row, and still in
    // the count.
    const { move } = await up();
    move(2, "done");
    move(3, "done");

    expect(row("steward 1")).toHaveAttribute("aria-expanded", "false");
    expect(folded("steward 1")).toBe("2");
    expect((await cardOf(row("steward 1"))).facts.Tasks).toBe("2 done");
  });

  it("keeps a fold set by hand when it would have opened the session by itself", async () => {
    const { move } = await up();

    await keysOn(row("steward 1"), "{ArrowLeft}");
    expect(row("steward 1")).toHaveAttribute("aria-expanded", "false");
    move(2, "done");
    move(3, "done");
    move(3, "running");

    expect(row("steward 1")).toHaveAttribute("aria-expanded", "false");
    expect(shape()).toEqual(["1 steward 1", "1 steward 4"]);
  });

  it("takes a fold set by hand off once, when a task under it comes to need the person (#1675)", async () => {
    const { move } = await up();
    await keysOn(row("steward 1"), "{ArrowLeft}");
    move(2, "done");

    move(3, "waiting", [3]);

    expect(row("steward 1")).toHaveAttribute("aria-expanded", "true");
    expect(shape()).toEqual(["1 steward 1", "2 devops 2", "2 devops 3", "1 steward 4"]);
  });

  it("keeps a session opened by hand open when it would have folded it by itself", async () => {
    const { move } = await up();
    await keysOn(row("steward 1"), "{ArrowLeft}");
    await keysOn(row("steward 1"), "{ArrowRight}");

    move(2, "done");
    move(3, "done");

    expect(row("steward 1")).toHaveAttribute("aria-expanded", "true");
    expect(shape()).toHaveLength(4);
  });

  it("says on a folded session's row how its finished tasks ended", async () => {
    core(
      [chat(1, "alpha")],
      ["a", "b", "c", "d", "e"].map((id) => finished(id, 1)),
    );
    render(<App />);
    await section();

    // Five done and nothing at work: folded by itself, and its row counts them, and its card
    // says how they ended (#1491: the one count, which its tab's chip and the explorer's line
    // share).
    await waitFor(() => expect(folded("steward 1")).toBe("5"));
    expect(row("steward 1")).toHaveAttribute("aria-expanded", "false");
    expect((await cardOf(row("steward 1"))).facts.Tasks).toBe("5 done");
    expect(shape()).toEqual(["1 steward 1"]);

    // Opened by hand, the finished tasks are under it, and the row counts nothing: they are
    // its rows now, folded into one (#3198). Its card says the same as it did folded.
    await keysOn(row("steward 1"), "{ArrowRight}");
    await waitFor(() => expect(shape()).toEqual(["1 steward 1", "2 Finished (5)"]));
    expect(row("steward 1").querySelector(".task-count")).toBeNull();
    expect((await cardOf(row("steward 1"))).facts.Tasks).toBe("5 done");
    // One source: the folded marks the row drew before are gone.
    expect(theTree().querySelector(".below-summary")).toBeNull();
  });

  it("does not fold a failure away: a session with a task that did not come out done stays open", async () => {
    core(
      [chat(1, "alpha")],
      [finished("a", 1), finished("b", 1, { outcome: "failed", folds: false })],
    );
    render(<App />);
    await section();

    await waitFor(() => expect(shape()).toEqual(["1 steward 1", "2 check b", "2 Finished (1)"]));
    expect(row("steward 1")).toHaveAttribute("aria-expanded", "true");
    // Folded by hand, it counts both, and its card says both ends, the failure first.
    await keysOn(row("steward 1"), "{ArrowLeft}");
    expect(folded("steward 1")).toBe("2");
    expect((await cardOf(row("steward 1"))).facts.Tasks).toBe("1 failed · 1 done");
  });
});

describe("the filter over the Chats list (V100-49)", () => {
  /** `reported`: the task has sent its report, done, so it is over once its turn is. */
  const four = (reported = false) => [
    chat(1, "alpha"),
    chat(2, "beta", {
      persona: "devops",
      label: "live check talk",
      from: reported ? { ...taskOf(1), reported: true, outcome: "done" } : taskOf(1),
    }),
    chat(3, "alpha", { label: "release notes" }),
    chat(4, "alpha"),
  ];
  const up = async (reported = false) => {
    const held = core(four(reported));
    render(<App />);
    await section();
    await waitFor(() => expect(shape()).toHaveLength(reported ? 3 : 4));
    for (const session of [1, 2, 3, 4]) held.move(session, reported ? "running" : "waiting");
    await waitFor(() => expect(shape()).toHaveLength(4));
    return held;
  };

  it("finds a task by its name, and keeps the session that asked for it above it", async () => {
    await up();

    await typed("talk");
    expect(shape()).toEqual(["1 steward 1", "2 live check talk"]);
    expect(hiddenSaid()).toBe("The filter hides 2 of 4 chats.");
    // What is drawn is what a screen reader counts.
    expect(place("steward 1")).toBe("1/1");
    expect(place("live check talk")).toBe("1/1");
  });

  // #6213: the number a coordinator named, `3` or `#3`, finds that chat alone, and a task
  // under the session that asked for it, as a name does.
  it("finds a chat by its number, typed bare or with #", async () => {
    await up();

    await typed("#3");
    expect(shape()).toEqual(["1 release notes"]);

    await userEvent.clear(box());
    await typed("2");
    expect(shape()).toEqual(["1 steward 1", "2 live check talk"]);
  });

  it("finds a task by its persona, its workspace and its state's word", async () => {
    const { move } = await up();

    await typed("devops");
    expect(shape()).toEqual(["1 steward 1", "2 live check talk"]);

    await userEvent.clear(box());
    await typed("beta");
    expect(shape()).toEqual(["1 steward 1", "2 live check talk"]);

    move(2, "running");
    await userEvent.clear(box());
    await typed("working");
    expect(shape()).toEqual(["1 steward 1", "2 live check talk"]);
  });

  it("has a chip for the chats that need the person and one for the ones at work", async () => {
    const { move } = await up();
    move(3, "waiting", [3]);
    move(2, "running", [3]);

    await userEvent.click(screen.getByRole("checkbox", { name: "needs you" }));
    expect(shape()).toEqual(["1 release notes"]);

    await userEvent.click(screen.getByRole("checkbox", { name: "working" }));
    expect(shape()).toEqual(["1 release notes", "1 steward 1", "2 live check talk"]);

    await userEvent.click(screen.getByRole("checkbox", { name: "needs you" }));
    expect(shape()).toEqual(["1 steward 1", "2 live check talk"]);
  });

  it("finds, with the working chip, what the order puts with the working", async () => {
    // Nothing has been heard from any of the four: each reads "running (no detail from …)",
    // which the order counts as at work.
    core(four());
    render(<App />);
    await section();
    await waitFor(() => expect(shape()).toHaveLength(4));

    await userEvent.click(screen.getByRole("checkbox", { name: "working" }));

    expect(shape()).toHaveLength(4);
    expect(hiddenSaid()).toBe("The filter hides no chat.");
  });

  it("has its chips as one stop of the keyboard, named as a group, with the arrows between them", async () => {
    await up();
    const chips = screen.getByRole("group", { name: "Show only" });
    const [needs, working] = within(chips).getAllByRole("checkbox");

    // One of them is the stop; the other is reached by an arrow and not by Tab.
    expect([needs.tabIndex, working.tabIndex]).toEqual([0, -1]);
    act(() => needs.focus());
    await userEvent.keyboard("{ArrowRight}");
    expect(document.activeElement).toBe(working);

    await userEvent.keyboard(" ");
    expect(working).toBeChecked();
  });

  it("says a chat it hides needs the person, and goes to it", async () => {
    const { move } = await up();
    move(2, "running");
    await userEvent.click(screen.getByRole("checkbox", { name: "working" }));
    expect(shape()).toEqual(["1 steward 1", "2 live check talk"]);
    expect(screen.queryByRole("button", { name: /the filter hides$/ })).toBeNull();

    // The task stops working and asks for the person: the chip no longer finds it, and the
    // session above it is hidden with it. No hand for it is drawn anywhere in the list.
    move(2, "waiting", [2]);

    expect(queryTree("Chats of this project")).toBeNull();
    expect(hiddenSaid()).toBe(
      "live check talk needs you, and the filter hides it. No chat matches the filter.",
    );
    await userEvent.click(
      screen.getByRole("button", {
        name: "Go to live check talk, which needs you and the filter hides",
      }),
    );
    await waitFor(() => expect(shownInFront()).toBe("session 2"));
  });

  it("finds a finished task by how it ended, under the chat that asked for it", async () => {
    core(four(), [
      finished("a", 3, { name: "check staging", outcome: "failed", folds: false }),
      finished("b", 3, { name: "count rows" }),
      finished("c", 4, { name: "tidy up" }),
    ]);
    render(<App />);
    await section();
    // Four chats, and the rows of what each folded or left open finished (#3198).
    await waitFor(() => expect(shape()).toHaveLength(6));

    await typed("failed");

    // Only what was asked for, under the chat that asked: the done one beside it is not drawn,
    // nor is its fold, nor steward 4's.
    await waitFor(() => expect(shape()).toEqual(["1 release notes", "2 check staging"]));
  });

  it("opens every row above what it found, whatever fold was set by hand, and puts the fold back", async () => {
    await up();
    await keysOn(row("steward 1"), "{ArrowLeft}");
    expect(shape()).toEqual(["1 steward 1", "1 release notes", "1 steward 4"]);

    await typed("talk");

    // Found means drawn: the fold set by hand does not hide it, and the count is of what is
    // not drawn.
    expect(shape()).toEqual(["1 steward 1", "2 live check talk"]);
    expect(hiddenSaid()).toBe("The filter hides 2 of 4 chats.");
    // A fold set while the filter is on is obeyed, for as long as the filter is.
    await keysOn(row("steward 1"), "{ArrowLeft}");
    expect(shape()).toEqual(["1 steward 1"]);
    await keysOn(row("steward 1"), "{ArrowRight}");

    await userEvent.clear(box());

    // The fold set before the filter is back.
    expect(shape()).toEqual(["1 steward 1", "1 release notes", "1 steward 4"]);
    expect(row("steward 1")).toHaveAttribute("aria-expanded", "false");
  });

  it("is cleared by Escape, in its box and in the list", async () => {
    await up();
    await typed("talk");
    expect(shape()).toHaveLength(2);

    await userEvent.keyboard("{Escape}");

    expect(box()).toHaveValue("");
    expect(shape()).toHaveLength(4);
    expect(hiddenSaid()).toBe("");

    await userEvent.click(screen.getByRole("checkbox", { name: "working" }));
    expect(hiddenSaid()).toBe("No chat matches the filter.");
    await userEvent.click(screen.getByRole("checkbox", { name: "working" }));
    await typed("notes");
    fireEvent.keyDown(row("release notes"), { key: "Escape" });
    expect(shape()).toHaveLength(4);
  });

  it("opens a session it folded by itself over a task the filter asks for", async () => {
    const { move } = await up(true);
    for (const session of [1, 3, 4]) move(session, "waiting");
    move(2, "done");
    expect(shape()).toEqual(["1 steward 1", "1 release notes", "1 steward 4"]);

    await typed("talk");

    expect(shape()).toEqual(["1 steward 1", "2 live check talk"]);
  });

  it("is in the palette too: a task is found there, and shown from there", async () => {
    await up();

    await userEvent.keyboard("{F2}");
    await userEvent.type(screen.getByRole("combobox"), "live check");
    await userEvent.click(
      await screen.findByRole("option", {
        name: /Show task live check talk in beta, asked by steward 1/,
      }),
    );

    await waitFor(() => expect(shownInFront()).toBe("session 2"));
  });
});

describe("a row of the Chats list (V100-19, V100-50)", () => {
  const rows = () => [
    chat(1, "alpha"),
    chat(2, "alpha", { persona: "devops", from: taskOf(1) }),
    chat(3, "beta", { persona: "devops", from: taskOf(1) }),
    chat(4, "alpha", {
      persona: "devops",
      from: taskOf(1),
      cwd: `${PLANE}/workspaces/alpha/.worktrees/app/fix-login`,
    }),
  ];
  const up = async () => {
    const held = core(rows());
    render(<App />);
    await section();
    await waitFor(() => expect(shape()).toHaveLength(4));
    return held;
  };

  it("is one line: the persona's badge with the state on it, the name, and where it works is in its card", async () => {
    const { move } = await up();
    move(1, "running");

    expect(theTree().querySelector(".line.two")).toBeNull();
    const first = row("steward 1");
    // The persona's mark leads the row, with the state on it (#3342).
    expect(
      first.querySelector('[data-part="lead"] [data-part="who"] .persona-mark'),
    ).not.toBeNull();
    expect(first.querySelector('[data-part="label"]')?.textContent).toBe("steward 1");
    expect(first.querySelector('[data-part="lead"] [data-part="word"]')?.textContent).toBe(
      "working",
    );
    expect((await cardOf(row("steward 1"))).facts.Workspace).toBe("alpha");
  });

  it("says the whole name in its card and to a screen reader, whatever the row cuts short", async () => {
    await up();

    expect(row("steward 1")).toHaveAccessibleName(/^steward 1/);
    expect((await cardOf(row("steward 1"))).text).toMatch(/^steward 1/);
  });

  it("says a task's workspace and its own branch in its card, and on no row", async () => {
    await up();

    expect((await cardOf(row("devops 2"))).facts.Workspace).toBe("alpha");
    expect((await cardOf(row("devops 3"))).facts.Workspace).toBe("beta");
    expect((await cardOf(row("devops 4"))).facts.Branch).toBe("fix-login");
    expect(theTree().textContent).not.toContain("fix-login");
  });

  it("says how long a chat has been in its state, once the window has seen it come into it", async () => {
    const { move } = await up();

    // The first the window hears of it: how long it has been so is not known, and not said.
    move(2, "running");
    expect((await cardOf(row("devops 2"))).facts["In this state"]).toBeUndefined();

    move(2, "waiting");

    expect((await cardOf(row("devops 2"))).facts["In this state"]).toBe("just now");
    expect((await cardOf(row("devops 3"))).facts["In this state"]).toBeUndefined();
  });

  it("keeps its two status lines in the tree while they have nothing to say", async () => {
    await up();

    const lines = [...document.querySelectorAll('.chats-notes [role="status"]')];
    expect(lines.map((line) => line.textContent)).toEqual(["", ""]);
    // Out of the rows' own box: saying something there pushes no row.
    expect(list().contains(lines[0])).toBe(false);
  });

  it("wears no badge for having no tab, and says nothing of tabs", async () => {
    await up();

    expect(theTree().querySelector(".no-tab")).toBeNull();
    expect(theTree().textContent).not.toMatch(/no tab/i);
    for (const one of rowsOf(theTree())) expect(tipOf(one) ?? "").not.toMatch(/tab/i);
  });
});

describe("a chat's tab and its row wear one mark (#5340)", () => {
  /** The tab on the strip whose name starts with `name`. */
  const tabOf = (name: string) => {
    const found = [...document.querySelectorAll<HTMLElement>('[data-strip="Tabs"] [role="tab"]')]
      .filter((tab) => tab.querySelector(".tab-name")?.textContent === name)
      .at(0);
    if (found === undefined) throw new Error(`no tab named ${name}`);
    return found;
  };
  /** What a mark draws: its state, and the glyph of its corner. */
  const markIn = (holder: Element) => {
    const root = holder.querySelector('[data-scope="chat-mark"][data-part="root"]');
    return {
      state: root?.getAttribute("data-variant") ?? null,
      glyph:
        root
          ?.querySelector('[data-part="badge"] svg')
          ?.getAttribute("class")
          ?.split(" ")
          .filter((one) => one.startsWith("lucide-"))
          .join(" ") ?? null,
    };
  };

  const twoChats = () => [
    chat(1, "alpha", { in_front: false }),
    chat(2, "alpha", { in_front: true }),
  ];
  const pending: [string, State, number[], Partial<Moved>, string][] = [
    ["needs you", "waiting", [1], {}, "needs"],
    ["working", "running", [], {}, "working"],
    ["idle", "waiting", [], {}, "idle"],
    ["ended", "done", [], {}, "ended"],
    [
      "waiting on its own work",
      "waiting",
      [],
      { own_work: { helpers: 2, workflows: 0, monitors: 0, shells: 0 } },
      // Its own work wakes it: Working until that work ends (ADR 0076, #6012).
      "working",
    ],
    [
      "a task failed, in the queue for that alone",
      "running",
      [1],
      {
        only_failures: true,
        needs: [{ kind: "task_failed", id: "r-1", task: "build", how: "failed", why: "" }],
      } as Partial<Moved>,
      "failed",
    ],
  ];

  it.each(pending)("wears the same mark on both: %s", async (_, state, queue, more, expected) => {
    const { move } = core(twoChats());
    render(<App />);
    await section();
    await waitFor(() => expect(tabOf("steward 1")).toBeTruthy());

    move(1, state, queue, more);

    await waitFor(() => expect(markIn(row("steward 1")).state).toBe(expected));
    expect(markIn(tabOf("steward 1"))).toEqual(markIn(row("steward 1")));
  });

  it("wears the hourglass on both for a chat waiting on its tasks, never the board's dot", async () => {
    const { move } = core([
      chat(1, "alpha"),
      chat(2, "alpha", { in_front: true }),
      chat(5, "alpha", { persona: "devops", from: taskOf(1) }),
    ]);
    render(<App />);
    await section();
    await waitFor(() => expect(tabOf("steward 1")).toBeTruthy());

    move(5, "running");
    move(1, "waiting");

    await waitFor(() => expect(markIn(row("steward 1")).state).toBe("waiting"));
    expect(markIn(tabOf("steward 1"))).toEqual({ state: "waiting", glyph: "lucide-hourglass" });
    expect(markIn(row("steward 1"))).toEqual({ state: "waiting", glyph: "lucide-hourglass" });
  });

  it("keeps the mark on a pinned tab", async () => {
    const { move } = core([
      chat(1, "alpha", { pinned: true }),
      chat(2, "alpha", { in_front: true }),
    ]);
    render(<App />);
    await section();
    await waitFor(() => expect(tabOf("steward 1").querySelector(".pinned")).not.toBeNull());

    move(1, "waiting", [1]);

    await waitFor(() => expect(markIn(tabOf("steward 1")).state).toBe("needs"));
    expect(markIn(tabOf("steward 1"))).toEqual(markIn(row("steward 1")));
  });

  it("keeps its row's mark on a pinned tab drawn as its icon alone (#5448)", async () => {
    const room = measuring();
    try {
      const { move } = core([
        chat(1, "alpha", { pinned: true }),
        chat(2, "alpha", { pinned: true }),
        chat(3, "alpha"),
        chat(4, "alpha", { in_front: true }),
      ]);
      render(<App />);
      await section();
      await waitFor(() => expect(tabOf("steward 1").querySelector(".pinned")).not.toBeNull());

      // Room for three tabs: the two pins alone would take over half, so they are icons only.
      room.roomFor(3);
      await waitFor(() => expect(tabOf("steward 1").closest("[data-icon-only]")).not.toBeNull());

      for (const [state, queue, expected] of [
        ["waiting", [1], "needs"],
        ["running", [], "working"],
        ["waiting", [], "idle"],
      ] as [State, number[], string][]) {
        move(1, state, queue);
        await waitFor(() => expect(markIn(row("steward 1")).state).toBe(expected));
        expect(markIn(tabOf("steward 1"))).toEqual(markIn(row("steward 1")));
      }
    } finally {
      room.stop();
    }
  });

  it("wears no state on a view's tab", async () => {
    core([chat(1, "alpha")]);
    render(<App />);
    await section();
    await waitFor(() => expect(tabOf("steward 1")).toBeTruthy());

    for (const tab of document.querySelectorAll('[data-strip="Tabs"] [role="tab"]'))
      if (tab.querySelector('[data-mark="view"]'))
        expect(tab.querySelector('[data-scope="chat-mark"]')).toBeNull();
  });
});

describe("a chat's tab and its row say one state word (#4935)", () => {
  /** The tab on the strip whose name starts with `name`. */
  const tabOf = (name: string) => {
    const found = [...document.querySelectorAll<HTMLElement>('[data-strip="Tabs"] [role="tab"]')]
      .filter((tab) => tab.querySelector(".tab-name")?.textContent === name)
      .at(0);
    if (found === undefined) throw new Error(`no tab named ${name}`);
    return found;
  };
  /** What the tab says its chat is doing: its one word, as the scenario specs read it
   *  (`chat.state.e2e.ts`). */
  const tabSays = (name: string) =>
    tabOf(name).querySelector(".tab-said")?.getAttribute("aria-label");

  // Every state the board says, and the queue's word on the one that waits.
  const states: [string, State, number[]][] = [
    ["at work", "running", []],
    ["asking for the person", "waiting", [1]],
    ["waiting, not asking", "waiting", []],
    ["done", "done", []],
    ["failed", "failed", []],
    ["not heard from", "unknown", []],
  ];

  it.each(states)("says the same word on both, %s", async (_, state, queue) => {
    const { move } = core(threeSessions());
    render(<App />);
    await section();
    await waitFor(() => expect(shape()).toHaveLength(4));
    await waitFor(() => expect(tabOf("steward 1")).toBeTruthy());

    move(1, state, queue);

    await waitFor(() => expect(word("steward 1")).toBeTruthy());
    expect(tabSays("steward 1")).toBe(word("steward 1"));
    expect(tabOf("steward 1").querySelectorAll(".tab-said")).toHaveLength(1);
  });

  it("says the same word on both for a session waiting on its own work", async () => {
    const { move } = core(threeSessions());
    render(<App />);
    await section();
    await waitFor(() => expect(shape()).toHaveLength(4));
    move(4, "done");

    move(1, "waiting", [], { own_work: { helpers: 2, workflows: 0, monitors: 0, shells: 0 } });

    await waitFor(() => expect(word("steward 1")).toMatch(/^waiting on /));
    expect(tabSays("steward 1")).toBe(word("steward 1"));
  });

  it("says the same word on both for a shell", async () => {
    const { move } = core([
      chat(1, "alpha", { harness: null, profile: null, persona: null, name: "zsh" }),
    ]);
    render(<App />);
    await section();
    await waitFor(() => expect(shape()).toHaveLength(1));

    move(1, "running");

    const name = rowsOf(theTree())[0].querySelector(".session")?.textContent as string;
    await waitFor(() => expect(word(name)).toBeTruthy());
    expect(tabSays(name)).toBe(word(name));
  });

  it("says the same word on both for a task in a tab of its own that has reported", async () => {
    const { move } = core([
      chat(1, "alpha"),
      chat(5, "alpha", {
        persona: "devops",
        in_front: true,
        from: { ...taskOf(1), tab: true, reported: true, outcome: "blocked" },
      }),
    ]);
    render(<App />);
    await section();
    await waitFor(() => expect(tabOf("devops 5")).toBeTruthy());

    move(5, "waiting");

    await waitFor(() => expect(word("devops 5")).toBeTruthy());
    expect(tabSays("devops 5")).toBe(word("devops 5"));
  });

  it("says the same word on both for a session waiting on its task", async () => {
    const { move } = core(threeSessions());
    render(<App />);
    await section();
    await waitFor(() => expect(shape()).toHaveLength(4));

    move(4, "running");
    move(1, "waiting");

    await waitFor(() => expect(word("steward 1")).toBe("waiting on 1 task"));
    expect(tabSays("steward 1")).toBe("waiting on 1 task");
  });
});

describe("the keys of a row (#1499)", () => {
  const up = async () => {
    const held = core([
      chat(1, "alpha"),
      chat(2, "alpha", { persona: "devops", from: taskOf(1) }),
      chat(3, "alpha"),
    ]);
    render(<App />);
    await section();
    await waitFor(() => expect(shape()).toHaveLength(3));
    return held;
  };
  const said = () => document.querySelector(".chats-said")?.textContent;

  it("opens a row on Enter", async () => {
    await up();

    act(() => row("devops 2").focus());
    await userEvent.keyboard("{Enter}");

    await waitFor(() => expect(shownInFront()).toBe("session 2"));
  });

  it("opens a task beside the session that asked for it on Space (#1489)", async () => {
    await up();

    act(() => row("devops 2").focus());
    await userEvent.keyboard(" ");

    // In the tab it returns to: its chat on one side, the task on the other.
    await waitFor(() =>
      expect(screen.getAllByTestId("pane").map((pane) => pane.textContent)).toEqual([
        "session 1",
        "session 2",
      ]),
    );
    expect(said()).toBe("");
  });

  it("says why Space does nothing on a chat that is not a task", async () => {
    await up();

    act(() => row("steward 1").focus());
    await userEvent.keyboard(" ");

    expect(said()).toBe(
      "Only a task opens beside the chat that asked for it. Press Enter to show this chat.",
    );
    // Space did not press the row: nothing was opened.
    expect(shownInFront()).toBe("session 1");
  });

  it("asks to stop a task on Delete, and stops nothing until the person says so", async () => {
    const { asked } = await up();

    act(() => row("devops 2").focus());
    await userEvent.keyboard("{Delete}");

    // The task's own question (#1488), never a chat's Stop and never the close dialog.
    expect(await screen.findByRole("alertdialog", { name: "Stop task 'devops 2'?" })).toBeTruthy();
    expect(asked("stop_chat")).toEqual([]);
    expect(asked("end_task")).toEqual([]);
  });

  it("asks to stop a task on the key a Mac marks delete, which sends Backspace", async () => {
    const platform = vi.spyOn(navigator, "platform", "get").mockReturnValue("MacIntel");
    try {
      await up();

      act(() => row("devops 2").focus());
      await userEvent.keyboard("{Backspace}");

      expect(
        await screen.findByRole("alertdialog", { name: "Stop task 'devops 2'?" }),
      ).toBeTruthy();
    } finally {
      platform.mockRestore();
    }
  });

  it("does not stop a task on Delete with a modifier held, or on Backspace off a Mac", async () => {
    await up();

    act(() => row("devops 2").focus());
    await userEvent.keyboard("{Shift>}{Delete}{/Shift}");
    await userEvent.keyboard("{Backspace}");

    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("takes down what a key said at the next key that is not Space", async () => {
    await up();
    act(() => row("steward 1").focus());
    await userEvent.keyboard(" ");
    expect(said()).not.toBe("");

    await userEvent.keyboard(" ");
    expect(said()).not.toBe("");
    await userEvent.keyboard("{ArrowDown}");

    expect(said()).toBe("");
  });

  it("does nothing on Delete for a chat that is not a task", async () => {
    await up();

    act(() => row("steward 3").focus());
    await userEvent.keyboard("{Delete}");

    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("walks the rows with the arrows, through a fold and a filter", async () => {
    await up();

    act(() => row("steward 1").focus());
    await userEvent.keyboard("{ArrowLeft}");
    expect(row("steward 1")).toHaveAttribute("aria-expanded", "false");
    await userEvent.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(row("steward 3"));
    expect(place("steward 3")).toBe("2/2");

    await typed("devops");
    // The filter opens the row above what it found: the arrows walk into it.
    expect(shape()).toEqual(["1 steward 1", "2 devops 2"]);
    act(() => row("steward 1").focus());
    await userEvent.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(row("devops 2"));
    expect(row("devops 2")).toHaveAttribute("aria-level", "2");
  });
});

describe("a chat moving, with fifty sessions and their tasks listed (SC-3)", () => {
  it("draws no row for a move, even one that changes the order (#3198)", async () => {
    const { move } = core([
      ...Array.from({ length: 50 }, (_, at) => chat(at + 1, at % 2 === 0 ? "alpha" : "beta")),
      ...Array.from({ length: 50 }, (_, at) =>
        chat(at + 51, "alpha", { persona: "devops", from: taskOf(at + 1) }),
      ),
    ]);
    // Not under StrictMode, which draws everything twice: what is counted is draws.
    renderBare(<App />);
    await section();
    await waitFor(() => expect(shape()).toHaveLength(100));
    for (let turn = 0; turn < 10; turn += 1) await act(async () => {});
    drawn.rows.length = 0;

    // Tasks start and end their turns, and a session too: each says so on its own mark.
    move(51, "running");
    move(52, "running");
    move(51, "waiting");
    move(7, "running");

    expect(word("devops 52")).toBe("working");
    // A task's turn ended owing its report: it waits on the chat that asked (#6012).
    expect(word("devops 51")).toBe("waiting on the chat that asked");
    expect(drawn.rows).toEqual([]);

    // Session 40 and its task go idle: it goes below the ones at work, which is the one
    // thing that moves rows. Its task going idle first moves nothing.
    move(90, "waiting");
    expect(drawn.rows).toEqual([]);
    move(40, "waiting");

    expect(shape().at(-2)).toBe("1 steward 40");
    // No row is drawn again: the tree moves session 40 and its task below the ten it moved
    // past, each row as it was (#3198). A row's place is the tree's to say, not the row's.
    expect(drawn.rows).toEqual([]);
  });

  it("draws no row for a word typed in the filter: the rows it leaves stand as they were", async () => {
    core([
      ...Array.from({ length: 50 }, (_, at) => chat(at + 1, at % 2 === 0 ? "alpha" : "beta")),
      ...Array.from({ length: 50 }, (_, at) =>
        chat(at + 51, "alpha", { persona: "devops", from: taskOf(at + 1) }),
      ),
    ]);
    renderBare(<App />);
    await section();
    await waitFor(() => expect(shape()).toHaveLength(100));
    for (let turn = 0; turn < 10; turn += 1) await act(async () => {});
    drawn.rows.length = 0;

    act(() => box().focus());
    fireEvent.change(box(), { target: { value: "devops 60" } });

    expect(shape()).toEqual(["1 steward 10", "2 devops 60"]);
    // No row is drawn again (#3198): the two it leaves are as they were, and the ninety-eight
    // that left draw nothing.
    expect(drawn.rows).toEqual([]);
  });
});

describe("a failed task's mark lasts until its chat has been seen (#5340)", () => {
  const FAILED = {
    only_failures: true,
    needs: [{ kind: "task_failed", id: "r-1", task: "build", how: "failed", why: "" }],
  } as Partial<Moved>;
  type Ask = (cmd: string) => Record<string, unknown>[];
  const seen = (asked: Ask) => asked("task_failure_seen");

  it("stays on a chat in a background tab, and the core is told nothing", async () => {
    const { move, asked } = core([
      chat(1, "alpha", { in_front: false }),
      chat(2, "alpha", { in_front: true }),
    ]);
    render(<App />);
    await section();
    await waitFor(() => expect(row("steward 1")).toBeTruthy());

    move(1, "running", [1], FAILED);

    await waitFor(() =>
      expect(
        row("steward 1").querySelector('[data-scope="chat-mark"][data-part="root"]'),
      ).toHaveAttribute("data-variant", "failed"),
    );
    expect(seen(asked)).toEqual([]);
  });

  it("is seen once its chat is in front and focused, and not while the window is away", async () => {
    const focused = vi.spyOn(document, "hasFocus").mockReturnValue(false);
    const { move, asked } = core([chat(1, "alpha", { in_front: true })]);
    render(<App />);
    await section();
    await waitFor(() => expect(row("steward 1")).toBeTruthy());

    move(1, "running", [1], FAILED);
    await waitFor(() =>
      expect(
        row("steward 1").querySelector('[data-scope="chat-mark"][data-part="root"]'),
      ).toHaveAttribute("data-variant", "failed"),
    );
    expect(seen(asked)).toEqual([]);

    focused.mockReturnValue(true);
    act(() => void window.dispatchEvent(new Event("focus")));

    await waitFor(() => expect(seen(asked)).toContainEqual(expect.objectContaining({ id: "r-1" })));
    focused.mockRestore();
  });
});
