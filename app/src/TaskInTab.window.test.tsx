// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode, useEffect, useRef } from "react";
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
import { hoverTipOf } from "./ui/tooltip/tooltip.testkit";
import { askToEnd, chatList, paneMore } from "./menus.testkit";
import type {
  ChatBlocked,
  DispatchPending,
  FinishedTask,
  LaidOut,
  Moved,
  OpenChat,
  Shown,
  VaultRefused,
} from "./bindings";
import type { State } from "./chatState";
import { chatsOf } from "./laidOut";
import { aTabForEachChat, leadOf, onScreenOf } from "./laidOut.testkit";
import { cardOf as cardOfRow } from "./chatCard.testkit";
import { forgetKeyboard } from "./paneKeyboard";
import { REFERENCE_TYPE } from "./references";
import { forgetThisLaunch } from "./regions";
import { chipSays, rowSays, stripNamed } from "./test-strips";
import { askOfBlock, askOfDispatch, askOfVault } from "./test-asks";
import { forgetInboxOpen, inboxAwayAtLaunch } from "./test-inbox";
import { findResult } from "./actionResult.testkit";
import { heard } from "./frame.testkit";
import { onTheWire } from "./chatEvents.testkit";
import { findRow, findTree, rowsOf } from "./ui/tree/tree.testkit";
import { rowName } from "./menus.testkit";

/**
 * **A task opens in the pane it returns to** (#1486), against the whole window: pressing a task
 * swaps what a pane shows, and adds no tab. While a tab shows a
 * task, the pane's existing top line carries a breadcrumb and the tab's label says the task.
 *
 * The core here is a fixture: a task is a chat whose `from` says `task: true` and
 * `tab: false`, as `ChatsSection.window.test.tsx` has it.
 */

vi.mock("./SessionPane", async () => {
  const { invoke } = await import("@tauri-apps/api/core");
  const { paneDrawn } = await import("./paneKeyboard");
  return {
    /** A pane that takes the keyboard when the window gives it, as a terminal does, and sends
     *  what is typed in it to its own session, as a terminal does. */
    SessionPane: ({
      plane,
      session,
      readOnly,
    }: {
      plane: string;
      session: number;
      readOnly?: boolean;
    }) => {
      const pane = useRef<HTMLDivElement>(null);
      useEffect(() => paneDrawn(plane, session, () => pane.current?.focus()), [plane, session]);
      return (
        <div
          data-testid="pane"
          data-session={session}
          data-read-only={readOnly ? "" : undefined}
          tabIndex={-1}
          ref={pane}
          // A screen that is read-only takes no input, as the terminal's `disableStdin` does.
          onKeyDown={(event) => {
            if (!readOnly) void invoke("send_input", { plane, session, text: event.key });
          }}
        >
          session {session}
        </div>
      );
    },
  };
});

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

/** The number a reopened finished task's new chat is given (#1485). */
const REOPENED = 25;

const PLANE = "/home/dev/plane";

type Lineage = NonNullable<OpenChat["from"]>;
type Listed = OpenChat & { workspace: string };

/** A chat working in `workspace`, as the core lists it. */
function chat(session: number, workspace: string, more: Partial<OpenChat> = {}): Listed {
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

/** A task of chat `asker`. */
function taskOf(asker: number, more: Partial<Lineage> = {}): Lineage {
  return {
    chat: asker,
    name: `steward ${asker}`,
    workspace: "alpha",
    task: true,
    tab: false,
    reported: false,
    unreported: false,
    ...more,
  };
}

function asListed(one: Listed): OpenChat {
  const sent: OpenChat & { workspace?: string } = { ...one };
  delete sent.workspace;
  return sent;
}

type Asked = { cmd: string; args: Record<string, unknown> };

/** A dispatch chat `session` asked for that no grant covers: held until the person answers. */
function held(session: number, id: number): DispatchPending {
  return {
    plane: PLANE,
    id,
    session,
    chat: `chat ${session}`,
    asking: "steward",
    target: "devops",
    brief: "Check why the deploy is red.",
    brief_cut: false,
    brief_lines: 1,
    levels: ["chat", "you", "project"],
    locked: null,
    never_unread: null,
    works_in: null,
    works_in_missing: false,
    allowed_in: [],
    works_with: "devops works with its own access: no vault; no hosts beyond the project's.",
    also: [],
    shown: "s0",
    task: null,
    task_cut: false,
    profile: null,
    mode: "task",
    runs_on: null,
  };
}

/** A vault chat `session` was refused. */
function refusedVault(session: number): VaultRefused {
  return {
    plane: PLANE,
    session,
    vault: "prod",
    persona: "steward",
    tagged_for: "devops",
    dispatch_to: null,
    locked: null,
  };
}

/** What chat `session`'s sandbox blocked: its own work, with nothing to allow. */
function blocked(session: number): ChatBlocked {
  return {
    plane: PLANE,
    session,
    operation: "write",
    kind: "toolchain-cache",
    ours: false,
    harness: "claude",
    said: "a write to a toolchain's package cache",
    offer: "none",
    target: null,
    route: null,
    levels: [],
    held: false,
    ruled: null,
  };
}

/** Chat `session`'s sandbox blocked reaching `host`, which Allow names (#1508). */
function blockedOnHost(session: number, host = "registry.npmjs.org"): ChatBlocked {
  return {
    plane: PLANE,
    session,
    operation: "connect",
    kind: "host",
    ours: false,
    harness: "claude",
    said: "a connection to a host the project does not list",
    offer: "host",
    target: host,
    route: null,
    levels: ["chat", "you", "project"],
    held: false,
    ruled: null,
  };
}

/**
 * The core, holding `open` chats in two workspaces. It keeps the tabs the window tells it
 * (`window_layout`, #4751), as the record does, and what it holds for the person: dispatches
 * waiting for a grant and vaults it refused, by chat.
 */
/**
 * The layout a launch is answered when a test says none: a tab for each chat the record kept a
 * tab for (every session, and a handoff or task whose lineage says it has one), on its
 * workspace's strip. Since #4758 a chat gets a tab only from the stored layout.
 */
function tabbed(open: Listed[]): LaidOut {
  return aTabForEachChat(
    open.filter((one) => one.from == null || one.from.tab),
    (session) => open.find((one) => one.session === session)?.workspace ?? null,
  );
}

function core(open: Listed[], atLaunch: LaidOut = tabbed(open)) {
  const asked: Asked[] = [];
  const listeners = new Map<string, number[]>();
  const dispatches = new Map<number, DispatchPending[]>();
  const refusals = new Map<number, VaultRefused[]>();
  /** The finished rows the core lists, as it reads them from its dispatch records (#1485). */
  const finished: FinishedTask[] = [];
  /** How many of the next `window_layout`s the core refuses. */
  const refusing = { layouts: 0, calls: 0 };
  /** Every tab as the window last said it, and the core took it: a reload is answered it. */
  const laid: { now: LaidOut | null } = { now: atLaunch };
  /** The tasks the core's next answer to several allows, where keeping fails part way. */
  const answering: { only?: number[] } = {};
  /** The queue the core said last: what its asks registry says waits on a reply (#1690). */
  let queued: number[] = [];
  /** The blocks the core holds, by chat: each an ask of the registry's (#1695). */
  const blocks = new Map<number, ChatBlocked[]>();
  const nameIn = (session: number) => {
    const one = open.find((chat) => chat.session === session);
    return [one?.label ?? one?.name ?? `chat ${session}`];
  };
  /** The chats whose blocks an answer named are held on them no more. */
  const answeredBlocks = (sessions: readonly number[]) => {
    for (const session of sessions) blocks.delete(session);
  };
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: a });
    if (cmd === "plugin:event|listen") {
      const { event, handler } = args as { event: string; handler: number };
      listeners.set(event, [...(listeners.get(event) ?? []), handler]);
      return 1;
    }
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "asks_waiting")
      return {
        plane: PLANE,
        asks: queued
          .map((session): Shown => ({
            session,
            ask: `question:${session}`,
            says: "Waiting on your reply",
            options: [],
            source: "question",
            chain: (() => {
              const one = open.find((chat) => chat.session === session);
              return [one?.label ?? one?.name ?? `chat ${session}`];
            })(),
            answer: { via: "in-its-pane" },
          }))
          .concat(
            [...dispatches.values()].flat().map((one) => askOfDispatch(one, nameIn(one.session))),
            [...blocks.values()].flat().flatMap((one) => askOfBlock(one, nameIn(one.session))),
            [...refusals.values()].flat().map((one) => askOfVault(one, nameIn(one.session))),
          ),
      };
    if (cmd === "opened_chats") return open.map(asListed);
    if (cmd === "open_chat_tab") {
      const one = open.find((chat) => chat.session === a.session);
      if (one?.from) one.from = { ...one.from, tab: true };
      return null;
    }
    if (cmd === "window_layout") {
      if (refusing.calls > 0) {
        refusing.calls -= 1;
        // A call that never reached the core: no answer of its at all.
        throw new Error("the link to the core went away");
      }
      if (refusing.layouts > 0) {
        refusing.layouts -= 1;
        // The core's own refusal, as a command answers one: its sentence.
        throw "purlis did not keep that layout: it shows chat 5 in two panes.";
      }
      // The first chat of the front tab is the one the core takes to be in front.
      laid.now = a.layout as LaidOut;
      const lead = leadOf(laid.now);
      for (const one of open) one.in_front = one.session === lead;
      return null;
    }
    if (cmd === "reopened_layout") return laid.now;
    if (cmd === "dispatch_grants_needed") return dispatches.get(a.session as number) ?? [];
    if (cmd === "allow_dispatch" || cmd === "keep_dispatch_blocked") {
      for (const [session, waiting] of dispatches)
        dispatches.set(
          session,
          waiting.filter((one) => one.id !== a.id),
        );
      return cmd === "allow_dispatch" ? { said: "Allowed for this chat." } : true;
    }
    if (cmd === "vault_refusals") return refusals.get(a.session as number) ?? [];
    if (cmd === "allow_refused_vault" || cmd === "keep_vault_blocked") {
      refusals.set(a.session as number, []);
      return { said: cmd === "allow_refused_vault" ? "Allowed." : "Kept blocked." };
    }
    // The core holds the name it is given and answers it: the chat's label from then on.
    if (cmd === "rename_chat") {
      const one = open.find((chat) => chat.session === a.session);
      if (one) one.label = a.label as string;
      return a.label;
    }
    if (cmd === "allow_sandbox_block_for_tasks") {
      const tasks = (a.seen as { task: number }[]).map((one) => one.task);
      const answered = answering.only ?? tasks;
      answeredBlocks(answered);
      return {
        said:
          answered.length === tasks.length
            ? "Allowed for each of these tasks on its own."
            : `It was allowed for chat ${answered.join(" and ")} only, and not for the rest.`,
        answered,
      };
    }
    if (cmd === "keep_sandbox_block_for_tasks") {
      const tasks = (a.seen as { task: number }[]).map((one) => one.task);
      answeredBlocks(tasks);
      return tasks;
    }
    if (cmd === "allow_sandbox_block") {
      answeredBlocks([a.session as number]);
      return { said: "Allowed for this chat." };
    }
    if (cmd === "reference_into_chat") return { kind: "typed", text: "@src/main.rs" };
    if (cmd === "ask_chat_restart") return null;
    if (cmd === "restart_chat") {
      // The same chat on its conversation, under a new number. The core re-points what the
      // chats it asked for say of it only when the test says the list has caught up.
      const one = open.find((chat) => chat.session === a.session);
      if (!one) throw new Error(`no chat ${String(a.session)} is open`);
      one.session = (a.session as number) + 20;
      return { chat: asListed(one), notices: [], not_yet: null };
    }
    if (cmd === "reopen_finished_task") {
      // A new chat on the finished task's conversation, an ordinary chat from then on.
      const one = { ...open[0], session: REOPENED, name: "sweep", from: null, shows: null };
      open.push(one);
      return asListed(one);
    }
    if (cmd === "owed_restarts") return [];
    if (cmd === "finished_tasks") return [...finished];
    if (cmd === "stopping_chats") return [];
    // A copy, as the core's answer reaches the window over IPC: the window keeps an answer
    // equal to the one it holds (#4729), so one sharing this fake's chats would change with them.
    if (cmd === "plane_sidebar")
      return structuredClone({
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
      });
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
  const said = async (event: string, payload: unknown) => {
    [event, payload] = onTheWire(event, payload);
    const handlers = listeners.get(event) ?? [];
    if (handlers.length === 0) throw new Error(`the window is not listening for ${event}`);
    await act(async () => {
      for (const handler of handlers) heard(handler, { event, id: 1, payload });
      await Promise.resolve();
    });
  };
  /** The core says the rows changed, and the window reads its list again. */
  const rowsChanged = () =>
    said("plane-changed", {
      plane: PLANE,
      changes: [{ kind: "chats", workspace: null, persona: null, path: "" }],
      answers: [{ answer: "sidebar" }],
    });
  return {
    asked,
    open,
    finished,
    refusing,
    laid,
    answering,
    rowsChanged,
    /** The core says chat `session`'s stop has ended it: it is gone from what the core lists. */
    ended: async (session: number) => {
      open.splice(
        open.findIndex((chat) => chat.session === session),
        1,
      );
      await said("chat-stop", { plane: PLANE, session, phase: "stopped" });
    },
    /** The core says chat `session` moved to `state`, with `queue` asking for the person. */
    move: async (session: number, state: State, at: number, queue: number[] = []) => {
      queued = queue;
      const moved: Moved = {
        plane: PLANE,
        session,
        state,
        needs_you: queue.includes(session),
        queue,
        moved_at: at,
        sequence: at,
        reports: [],
        refusals: [],
        children: [],
        needs: null,
        stopped: null,
      };
      await said("chat-moved", moved);
    },
    /** Chat `session` asks to dispatch with no grant: the core holds it and says so. */
    holdDispatch: async (session: number, id = 70 + session) => {
      const one = held(session, id);
      dispatches.set(session, [...(dispatches.get(session) ?? []), one]);
      await said("dispatch-grant-needed", one).catch(() => undefined);
    },
    /** Chat `session` is refused a vault: the core holds it and says so. */
    refuseVault: async (session: number) => {
      const one = refusedVault(session);
      refusals.set(session, [one]);
      await said("chat-vault-refused", one).catch(() => undefined);
    },
    /** Chat `session`'s sandbox blocked something of its own. */
    block: (session: number) => said("chat-sandbox-blocked", blocked(session)),
    /** Chat `session`'s sandbox blocked reaching `host` (#1508): the core holds it, an ask. */
    blockHost: (session: number, host?: string) => {
      const one = blockedOnHost(session, host);
      blocks.set(session, [...(blocks.get(session) ?? []), one]);
      return said("chat-sandbox-blocked", one);
    },
  };
}

const section = () => findTree("Chats of this project");

/** A chat's row, by its name: not a finished task's of the same name (#3198). */
const row = (tree: HTMLElement, name: string) => {
  const found = rowsOf(tree)
    .filter((one) => one.classList.contains("chat"))
    .find((one) => one.querySelector(".session")?.textContent === name);
  if (found === undefined) throw new Error(`no row is named ${name}`);
  return found;
};

const rows = (tree: HTMLElement) => rowsOf(tree);

/** The finished tasks' rows named `name`, once one is drawn: rows of the tree (#3198). */
const finishedRowsNamed = (name: string) =>
  waitFor(async () => {
    const found = rowsOf(await section()).filter(
      (one) => one.classList.contains("finished-task") && one.getAttribute("aria-label") === name,
    );
    if (found.length === 0) throw new Error(`no finished row is named ${name}`);
    return found;
  });

const strip = () => stripNamed("Tabs");
const tabNames = () =>
  within(strip())
    .queryAllByRole("tab")
    .map((tab) => tab.querySelector(".tab-name")?.textContent);

/** The tab of the session called `name`. */
const tab = (name: string) => {
  const found = within(strip())
    .getAllByRole("tab")
    .find((one) => one.querySelector(".tab-name")?.textContent === name);
  if (found === undefined) throw new Error(`no tab is ${name}'s`);
  return found;
};

/** The sessions the panes on screen show, left to right. */
const onScreen = () =>
  screen.queryAllByTestId("pane").map((pane) => Number(pane.getAttribute("data-session")));

/** The breadcrumb on screen, where a pane draws one. */
const crumbs = () => screen.queryByRole("navigation", { name: "Chat path" });

/** The ended band over a pane left on a chat that ended (#4756), where there is one. */
const away = () => screen.queryByRole("status", { name: /^Ended/ });
/** The ended band, once it is drawn. */
const band = () => screen.findByRole("status", { name: /^Ended/ });

/** The hand a tab wears for a chat of its own that is waiting and is not on screen: a button
 *  on the tab's chip, beside the tab's own button (#1487), which goes to that chat. */
const handOn = (name: string) =>
  within(tab(name).closest(".tab") as HTMLElement).queryByRole("button", { name: /needs? you\./ });

/** What that hand says is waiting. */
const handSays = (name: string) =>
  handOn(name)
    ?.getAttribute("aria-label")
    ?.replace(/\. Go to .*$/, "");

/** A Notice on the pane, by its accessible name. */
const notice = (name: string | RegExp) => screen.queryByRole("status", { name });

/** What the breadcrumb reads, as a person reads it. */
const crumbsSay = () => crumbs()?.textContent?.replace(/\s+/g, " ").trim();

const commandsOf = (asked: Asked[], cmd: string) =>
  asked.filter((one) => one.cmd === cmd).map((one) => one.args);

/** The tabs the window last told the core (#4751), or none. */
const laidSaid = (asked: Asked[]) =>
  commandsOf(asked, "window_layout").at(-1)?.layout as LaidOut | undefined;

/** What the core takes to be on screen from the tabs it was last told (`Chats::looked_at`). */
const onScreenSaid = (asked: Asked[]) => {
  const laid = laidSaid(asked);
  return laid === undefined ? [] : onScreenOf(laid);
};

/** Every chat in any pane of `laid`. */
const panedIn = (laid: LaidOut | null | undefined) =>
  laid === null || laid === undefined ? [] : laid.tabs.flatMap((tab) => chatsOf(tab.tree));

/**
 * steward 1 in alpha dispatched three tasks: talk (4) and sweep (5) in alpha, probe (6) in
 * beta. talk dispatched deep (7). steward 2 is another session in alpha.
 */
const withTasks = () => [
  chat(1, "alpha"),
  chat(2, "alpha"),
  chat(4, "alpha", { persona: "devops", label: "talk", from: taskOf(1) }),
  chat(5, "alpha", { persona: "devops", label: "sweep", from: taskOf(1) }),
  chat(6, "beta", { persona: "devops", label: "probe", from: taskOf(1) }),
  chat(7, "alpha", { persona: "devops", label: "deep", from: taskOf(4, { name: "talk" }) }),
];

/** The window, drawn, with every chat of `open` listed, on the stored layout `laid` if given. */
async function drawn(open: Listed[], laid?: LaidOut) {
  const held = core(open, laid);
  render(<App />);
  const tree = await section();
  await waitFor(() => expect(rows(tree)).toHaveLength(open.length));
  return { ...held, tree };
}

beforeEach(() => {
  globalThis.localStorage.clear();
  forgetThisLaunch();
  forgetKeyboard();
});

// A pane's Notice is listed in the Inbox too, and the right side opens on it since #1738: these
// tests are about the pane's, so the side opens on another view.
beforeEach(inboxAwayAtLaunch);
afterEach(forgetInboxOpen);
afterEach(() => {
  cleanup();
  clearMocks();
});

/** What the workspace strip's tab for `workspace` says of its counts (`CountPill`): its one
 *  accessible name, or nothing where it draws no pill. */

describe("pressing a task", () => {
  it("shows it in the focused pane and adds no tab (#4757)", async () => {
    const { tree, asked } = await drawn(withTasks());
    expect(tabNames()).toEqual(["steward 1", "steward 2"]);
    expect(onScreen()).toEqual([1]);

    await userEvent.click(row(tree, "talk"));

    // The pane shows talk itself, and its tab is named for what its pane shows (#4762).
    await waitFor(() => expect(onScreen()).toEqual([4]));
    expect(tabNames()).toEqual(["talk", "steward 2"]);
    expect(tab("talk").getAttribute("aria-selected")).toBe("true");
    // The tab is not made an ordinary one: the core is told nothing of a tab for the task.
    expect(commandsOf(asked, "open_chat_tab")).toEqual([]);
    // Its row is the current one in the list.
    expect(row(tree, "talk").getAttribute("aria-current")).toBe("true");
    expect(row(tree, "steward 1").getAttribute("aria-current")).toBeNull();
    // And no row says "no tab" of a task: none of them has one, and that is how tasks are.
    expect(tree.textContent).not.toContain("no tab");
  });

  it("lands in the focused pane of the tab in front, not in the pane it returns to (#4757)", async () => {
    const { tree } = await drawn(withTasks());
    await userEvent.click(tab("steward 2"));
    await waitFor(() => expect(onScreen()).toEqual([2]));

    await userEvent.click(row(tree, "sweep"));

    await waitFor(() => expect(onScreen()).toEqual([5]));
    expect(tabNames()).toEqual(["steward 1", "sweep"]);
    expect(tab("sweep").getAttribute("aria-selected")).toBe("true");
  });

  it("shows a task of a task in the focused pane, like any chat (#4757)", async () => {
    const { tree } = await drawn(withTasks());

    await userEvent.click(row(tree, "deep"));

    await waitFor(() => expect(onScreen()).toEqual([7]));
    expect(tabNames()).toEqual(["deep", "steward 2"]);
    // Its head and its tab's hover say the task's number, not its session's (#6212).
    await waitFor(() =>
      expect(document.querySelector(".pane-head .pane-number")?.textContent).toBe("#7"),
    );
    expect((await hoverTipOf(tab("deep")))?.split("\n")[0]).toBe("#7 · deep");
  });

  it("lands in the focused pane for a task whose session has closed, and adds no tab (#4757)", async () => {
    // Chat 9 asked for it and has closed since: the task is at the top of the list.
    const { tree, asked } = await drawn([
      chat(1, "alpha"),
      chat(12, "alpha", { persona: "devops", label: "left behind", from: taskOf(9) }),
    ]);

    await userEvent.click(row(tree, "left behind"));

    await waitFor(() => expect(onScreen()).toEqual([12]));
    expect(tabNames()).toEqual(["left behind"]);
    expect(commandsOf(asked, "open_chat_tab")).toEqual([]);
    expect(crumbs()).toBeNull();
  });

  it("shows a handoff in the focused pane as a session of its own, with no breadcrumb (#4757)", async () => {
    // Chat 3 is a handoff from steward 1, and this window draws no tab for it (the record
    // says it has one, and the fixture's tab was closed): it is a session of its own.
    const { tree, asked } = await drawn([
      chat(1, "alpha"),
      chat(3, "alpha", {
        label: "moved work",
        from: { ...taskOf(1), task: false, tab: false },
      }),
    ]);

    await userEvent.click(row(tree, "moved work"));

    await waitFor(() => expect(onScreen()).toEqual([3]));
    expect(tabNames()).toEqual(["moved work"]);
    expect(crumbs()).toBeNull();
    expect(commandsOf(asked, "open_chat_tab")).toEqual([]);
  });
});

describe("a tab remembers which chat it shows", () => {
  it("shows the task its pane was given when the tab comes back to the front (#4757)", async () => {
    const { tree } = await drawn(withTasks());
    await userEvent.click(row(tree, "talk"));
    await waitFor(() => expect(onScreen()).toEqual([4]));

    await userEvent.click(tab("steward 2"));
    await waitFor(() => expect(onScreen()).toEqual([2]));
    await userEvent.click(tab("talk"));

    await waitFor(() => expect(onScreen()).toEqual([4]));
    expect(crumbsSay()).toContain("talk");
  });

  it("tells the core what each tab shows, and every tab comes back on it, and only those, when the window is loaded again (#4758)", async () => {
    const open = [...withTasks(), chat(8, "alpha", { label: "notes", from: taskOf(2) })];
    const first = core(open);
    const window = render(<App />);
    const tree = await section();
    await waitFor(() => expect(rows(tree)).toHaveLength(open.length));
    // Two tabs, each left on a task: the one in front and the one behind it. A press lands in
    // the focused pane of the tab in front (#4757), so each tab is brought forward first.
    await userEvent.click(tab("steward 2"));
    await waitFor(() => expect(onScreen()).toEqual([2]));
    await userEvent.click(row(tree, "notes"));
    await waitFor(() => expect(onScreen()).toEqual([8]));
    await userEvent.click(tab("steward 1"));
    await waitFor(() => expect(onScreen()).toEqual([1]));
    await userEvent.click(row(tree, "sweep"));
    await waitFor(() => expect(onScreen()).toEqual([5]));
    // Every tab is told with what its pane shows (#4751): the front tab's task is on screen.
    await waitFor(() => expect(onScreenSaid(first.asked)).toEqual([5]));
    expect(panedIn(laidSaid(first.asked)).sort()).toEqual([5, 8]);
    window.unmount();
    forgetThisLaunch();

    // The same core: a reload, and a relaunch that put the chats back, both ask it.
    render(<App />);
    await section();

    await waitFor(() => expect(onScreen()).toEqual([5]));
    // Each tab comes back on its task, in its pane. The two sessions, in no pane, get no tab:
    // a launch draws the tabs the layout kept and no others (#4758).
    expect(tabNames()).toEqual(["sweep", "notes"]);
    expect(tab("sweep").getAttribute("aria-selected")).toBe("true");
    expect(crumbsSay()).toContain("sweep");
    // And the tab behind is on its task too.
    await userEvent.click(tab("notes"));
    await waitFor(() => expect(onScreen()).toEqual([8]));
  });

  it("says so to the core when a tab goes back to its own chat", async () => {
    const { tree, asked } = await drawn(withTasks());
    await userEvent.click(row(tree, "sweep"));
    await waitFor(() => expect(onScreen()).toEqual([5]));

    await userEvent.click(row(tree, "steward 1"));

    await waitFor(() => expect(onScreen()).toEqual([1]));
    await waitFor(() => expect(onScreenSaid(asked)).toEqual([1]));
    expect(panedIn(laidSaid(asked))).not.toContain(5);
  });

  it("tells the core the task a tab came back on, as the stored layout said it showed (#4758)", async () => {
    // A record keeps which chat each pane showed in its layout (#4751): steward 1's tab was on
    // sweep. A record from before layouts said it in the chat's `shows`, which the core's
    // upgrade reads into a layout (#4758), so the window reads the layout only.
    const open = withTasks();
    const laid = aTabForEachChat(
      [
        { session: 5, in_front: true },
        { session: 2, in_front: false },
      ],
      () => "alpha",
    );
    const { asked } = await drawn(open, laid);

    await waitFor(() => expect(onScreen()).toEqual([5]));
    await waitFor(() => expect(onScreenSaid(asked)).toEqual([5]));
  });

  it("says the tabs again where the core did not take them, at the next change", async () => {
    const { tree, refusing, laid } = await drawn(withTasks());
    refusing.layouts = 1;
    await userEvent.click(row(tree, "sweep"));
    await waitFor(() => expect(onScreen()).toEqual([5]));
    expect(panedIn(laid.now)).not.toContain(5);

    // The next change to the tabs says them again, with what else changed.
    await userEvent.click(tab("steward 2"));
    await waitFor(() => expect(onScreen()).toEqual([2]));

    await waitFor(() => expect(panedIn(laid.now)).toContain(5));
    expect(onScreenOf(laid.now as LaidOut)).toEqual([2]);
  });

  it("says the tabs again at the next draw where the call never reached the core", async () => {
    // A failed call is no verdict on the layout, unlike a refusal: nothing waits for a change.
    const { tree, refusing, laid, rowsChanged } = await drawn(withTasks());
    refusing.calls = 1;
    await userEvent.click(row(tree, "sweep"));
    await waitFor(() => expect(onScreen()).toEqual([5]));

    await rowsChanged();

    await waitFor(() => expect(panedIn(laid.now)).toContain(5));
  });

  it("never says a layout the core refused again unchanged, however often the window draws", async () => {
    // A refusal is the core's verdict on that layout (#4751): said again unchanged it would be
    // refused again, for as long as the window kept drawing.
    const { tree, asked, refusing, rowsChanged } = await drawn(withTasks());
    const before = commandsOf(asked, "window_layout").length;
    refusing.layouts = Number.POSITIVE_INFINITY;
    await userEvent.click(row(tree, "sweep"));
    await waitFor(() => expect(onScreen()).toEqual([5]));
    await waitFor(() => expect(commandsOf(asked, "window_layout")).toHaveLength(before + 1));

    // The window draws again (the plane changes, the sidebar is read again) and says nothing.
    await rowsChanged();
    await rowsChanged();
    await new Promise((settled) => setTimeout(settled, 300));

    expect(commandsOf(asked, "window_layout")).toHaveLength(before + 1);
  });
});

describe("a shown task that ends", () => {
  it("keeps its last screen, read-only, under the ended band, and the keyboard stays where it was", async () => {
    const { tree, ended, asked } = await drawn(withTasks());
    await userEvent.click(row(tree, "sweep"));
    await waitFor(() => expect(onScreen()).toEqual([5]));
    await waitFor(() => expect(document.activeElement?.getAttribute("data-session")).toBe("5"));
    const before = document.activeElement;

    await ended(5);

    // Nothing flipped: the pane a press put sweep in keeps it as ended (#4757), its tab named
    // for it, and the breadcrumb still says the path. The band says so (#4756).
    const said = await band();
    expect(tab("sweep")).toBeTruthy();
    expect(crumbsSay()).toBe("steward 1 › sweep · ended without a report");
    expect(said.textContent).toContain("sweep ended");
    expect(tabNames()).toEqual(["sweep", "steward 2"]);
    // Its last screen stays, and takes nothing typed.
    expect(onScreen()).toEqual([5]);
    expect(screen.getByTestId("pane")).toHaveAttribute("data-read-only");
    expect(document.activeElement).toBe(before);
    // No finished row says it can be reopened yet: Clear pane alone.
    expect(within(said).queryByRole("button", { name: "Resume" })).toBeNull();

    await userEvent.click(within(said).getByRole("button", { name: "Clear pane" }));

    // Clear pane closes the pane (#5019): the tab's only pane, and not the strip's last tab, so
    // the tab goes and the one beside it is in front.
    await waitFor(() => expect(tabNames()).toEqual(["steward 2"]));
    expect(onScreen()).toEqual([2]);
    expect(away()).toBeNull();
    expect(crumbs()).toBeNull();
    // Nothing more is ended by it.
    expect(commandsOf(asked, "close_session")).toEqual([]);
  });

  it("is resumed in its pane from the band, where its finished row can be reopened", async () => {
    const { tree, ended, finished, asked } = await drawn(withTasks());
    await userEvent.click(row(tree, "sweep"));
    await waitFor(() => expect(onScreen()).toEqual([5]));

    finished.push(finishedRow("sweep", { chat: 5, report: "Swept." }));
    await ended(5);
    const said = await band();

    await userEvent.click(within(said).getByRole("button", { name: "Resume" }));

    await waitFor(() =>
      expect(commandsOf(asked, "reopen_finished_task").map((one) => one.id)).toEqual(["01K6sweep"]),
    );
    // The reopened chat takes the pane the task was left in, not a tab of its own.
    await waitFor(() => expect(onScreen()).toEqual([REOPENED]));
    expect(tabNames()).toHaveLength(2);
    expect(away()).toBeNull();
  });

  it("keeps its Brief on the head, read by its finished row's record", async () => {
    // Nothing of an ended task is listed any more, so its one state button is its Brief
    // (#4757, #1494): the row's, not the chat's, which is gone.
    const { tree, ended, finished } = await drawn(withTasks());
    await userEvent.click(row(tree, "sweep"));
    await waitFor(() => expect(onScreen()).toEqual([5]));

    finished.push(finishedRow("sweep", { chat: 5, report: "Swept." }));
    await ended(5);
    await band();

    expect(await screen.findByRole("button", { name: /^Brief/ })).toBeTruthy();
  });

  it("has no Brief on the head once no finished row is left to read it by", async () => {
    const { tree, ended } = await drawn(withTasks());
    await userEvent.click(row(tree, "sweep"));
    await waitFor(() => expect(onScreen()).toEqual([5]));

    await ended(5);
    await band();

    expect(screen.queryByRole("button", { name: /^Brief/ })).toBeNull();
  });

  it("says how it reported, for one that had", async () => {
    const open = withTasks();
    open[3].from = taskOf(1, { reported: true, outcome: "done" });
    const { tree, ended } = await drawn(open);
    await userEvent.click(row(tree, "sweep"));
    await waitFor(() => expect(onScreen()).toEqual([5]));

    await ended(5);

    await band();
    expect(crumbsSay()).toBe("steward 1 › sweep · done");
  });

  /** A finished row of steward 1, as the core lists one for a task that ended. */
  const finishedRow = (name: string, more: Partial<FinishedTask>): FinishedTask => ({
    id: `01K6${name}`,
    asker: 1,
    name,
    persona: "devops",
    how: "done",
    outcome: "done",
    folds: true,
    report: "",
    changed: null,
    ended: "2026-10-08T12:04:30+00:00",
    place: "alpha",
    branch: null,
    reopens: true,
    not_reopened: null,
    chat: null,
    did_not_start: false,
    attempts: 0,
    waits: null,
    ...more,
  });

  it("shows the report of the task it was left on, as text, and says how it ended as its finished row does", async () => {
    // The pane last saw sweep still owing its report: the record is what says how it ended.
    const { tree, ended, finished } = await drawn(withTasks());
    await userEvent.click(row(tree, "sweep"));
    await waitFor(() => expect(onScreen()).toEqual([5]));

    finished.push(
      finishedRow("sweep", {
        chat: 5,
        how: "blocked",
        outcome: "blocked",
        folds: false,
        report: "The queue is locked.\n<b>Nothing</b> was changed.",
      }),
    );
    await ended(5);

    const pane = await band();
    const report = await within(pane).findByRole("region", { name: "Report from sweep" });
    // A chat's text (#5214): its line break kept, its raw HTML dropped, never drawn.
    expect(report.textContent).toBe("The queue is locked.\nNothing was changed.");
    expect(report.querySelector("br")).not.toBeNull();
    expect(report.querySelector("b")).toBeNull();

    // One vocabulary: the pane, the breadcrumb and the finished row say the same state, and
    // the core's own word is beside it where the row draws it.
    const [onRow] = await finishedRowsNamed("sweep");
    const says = (on: Element | undefined) => ({
      word: on?.querySelector(".shown-state .word")?.textContent,
      shape: on?.querySelector(".shown-state .shape")?.getAttribute("data-shape"),
    });
    expect(says(onRow)).toEqual({ word: "failed", shape: "cross" });
    // The core's own word, after the state's in what the row says beside its name (#3198), as
    // the pane draws it beside its state.
    expect(onRow.getAttribute("aria-description")).toBe("failed, blocked");
    expect(says(pane)).toEqual(says(onRow));
    expect(pane.querySelector(".outcome")?.textContent?.trim()).toBe("blocked");
    expect(crumbsSay()).toBe("steward 1 › sweep · failed");
    // Its way back is its finished row's Reopen, which that row offers.
    expect(within(pane).getByRole("button", { name: "Resume" })).toBeTruthy();
  });

  it("never draws the report of another task of the same name, before its own row is read or after", async () => {
    // steward 1 dispatched "sweep" before, and that one failed: its row is there already.
    // The second "sweep" (chat 5) ends while a pane is left on it.
    const { tree, ended, finished, rowsChanged } = await drawn(withTasks());
    const earlier = finishedRow("sweep", {
      id: "01K6EARLIER",
      chat: 3,
      how: "failed",
      outcome: "failed",
      folds: false,
      report: "The earlier sweep failed.",
    });
    finished.push(earlier);
    await rowsChanged();
    await finishedRowsNamed("sweep");
    await userEvent.click(row(tree, "sweep"));
    await waitFor(() => expect(onScreen()).toEqual([5]));

    // It ends, and the core's finished rows do not hold it yet: one row is named sweep, and
    // it is the other task's.
    await ended(5);

    const pane = await band();
    expect(pane.textContent).not.toContain("The earlier sweep failed.");
    expect(within(pane).queryByRole("region", { name: "Report from sweep" })).toBeNull();
    expect(crumbsSay()).toBe("steward 1 › sweep · ended without a report");

    // Its own row arrives: its report, and how it ended.
    finished.push(finishedRow("sweep", { id: "01K6THIS", chat: 5, report: "This sweep is done." }));
    await rowsChanged();

    const report = await within(pane).findByRole("region", { name: "Report from sweep" });
    expect(report.textContent).toBe("This sweep is done.");
    expect(crumbsSay()).toBe("steward 1 › sweep · done");

    // Clear finished takes the folded rows, this task's among them. One row named sweep is
    // left, the other task's, and the pane does not take it up.
    finished.splice(0, finished.length, earlier);
    await rowsChanged();

    await waitFor(() =>
      expect(within(pane).queryByRole("region", { name: "Report from sweep" })).toBeNull(),
    );
    expect((await finishedRowsNamed("sweep")).length).toBeGreaterThan(0);
    expect(pane.textContent).not.toContain("The earlier sweep failed.");
    expect(crumbsSay()).not.toContain("failed");
  });

  it("draws no report for a row that has no chat's number: one from before this launch, or one that is not an ended task", async () => {
    const { tree, ended, finished } = await drawn(withTasks());
    await userEvent.click(row(tree, "sweep"));
    await waitFor(() => expect(onScreen()).toEqual([5]));

    // Reopened, its own row is cleared; what is left under that name carries no number.
    finished.push(finishedRow("sweep", { chat: null, report: "Not this pane's." }));
    await ended(5);

    const pane = await band();
    // Done, so folded into its chat's "Finished (1)" row (#3198).
    await findRow(await section(), "Finished (1)");
    expect(within(pane).queryByRole("region", { name: "Report from sweep" })).toBeNull();
    expect(pane.textContent).not.toContain("Not this pane's.");
  });

  it("is still what its pane shows when its tab comes back from behind", async () => {
    const { tree, ended } = await drawn(withTasks());
    await userEvent.click(row(tree, "sweep"));
    await waitFor(() => expect(onScreen()).toEqual([5]));
    await userEvent.click(tab("steward 2"));
    await waitFor(() => expect(onScreen()).toEqual([2]));

    await ended(5);
    // The tab sweep was pressed into is named for it (#4757).
    await userEvent.click(tab("sweep"));

    expect(await band()).toBeTruthy();
    expect(crumbsSay()).toMatch(/^steward 1 › sweep · /);
  });
});

describe("a pane never shows a task without its breadcrumb", () => {
  it("keeps drawing a task of a task when the task between it and the session ends (#4757)", async () => {
    const { tree, ended } = await drawn(withTasks());
    await userEvent.click(row(tree, "deep"));
    await waitFor(() => expect(onScreen()).toEqual([7]));

    // talk asked for deep, and talk ends. deep is still running, and the pane was given deep
    // itself: a pane never changes what it shows by itself.
    await ended(4);
    await new Promise((resolve) => setTimeout(resolve, 300));

    expect(onScreen()).toEqual([7]);
    expect(away()).toBeNull();
  });

  it("says the path again once the session is started again under a new number (#4757)", async () => {
    const { tree, open, rowsChanged } = await drawn(withTasks());
    await userEvent.click(row(tree, "talk"));
    await waitFor(() => expect(onScreen()).toEqual([4]));
    await userEvent.click(tab("steward 2"));
    await waitFor(() => expect(onScreen()).toEqual([2]));

    // steward 1 is restarted from its list (#4753): it is chat 21 now, in no pane (#4757). The
    // core's list still says talk is a task of chat 1. The pane was given talk itself, so it
    // draws talk's terminal as any chat's pane does, and says its path once the list does.
    await chatList("steward 1");
    await userEvent.click(await screen.findByRole("menuitem", { name: "Restart" }));
    await waitFor(() => expect(open[0].session).toBe(21));
    await userEvent.click(tab("talk"));

    // The list catches up: talk and the rest are tasks of chat 21.
    for (const one of open) if (one.from?.chat === 1) one.from = { ...one.from, chat: 21 };
    await rowsChanged();

    await waitFor(() => expect(onScreen()).toEqual([4]));
    expect(crumbsSay()).toMatch(/› talk · /);
    expect(away()).toBeNull();
  });
});

describe("the breadcrumb", () => {
  it("is not drawn while a tab shows its session's own chat", async () => {
    await drawn(withTasks());
    expect(onScreen()).toEqual([1]);
    expect(crumbs()).toBeNull();
  });

  it("says the session, the task and the task's state, in the pane's existing top line", async () => {
    const { tree, move } = await drawn(withTasks());
    const corner = () => document.querySelector(".pane-head");
    const rowsBefore = corner()?.children.length;

    await userEvent.click(row(tree, "talk"));
    await waitFor(() => expect(onScreen()).toEqual([4]));
    await move(4, "running", 10);

    expect(crumbsSay()).toBe("steward 1 › talk · working");
    // In the line the gauge and the harness's name are in: the pane gains no row.
    expect(crumbs()?.parentElement?.className).toBe("pane-chips");
    expect(corner()?.children.length).toBe(rowsBefore);
    // The state is the one the task's row says, by the one function.
    expect(within(row(tree, "talk")).getByText("working")).toBeTruthy();
  });

  it("goes back to the session's own chat when the session's name is pressed", async () => {
    const { tree } = await drawn(withTasks());
    await userEvent.click(row(tree, "talk"));
    await waitFor(() => expect(onScreen()).toEqual([4]));

    await userEvent.click(
      within(crumbs() as HTMLElement).getByRole("button", { name: "steward 1" }),
    );

    await waitFor(() => expect(onScreen()).toEqual([1]));
    expect(crumbs()).toBeNull();
    expect(tabNames()).toEqual(["steward 1", "steward 2"]);
  });

  it("shows the whole path for a task of a task, and each name in it goes to that chat", async () => {
    const { tree } = await drawn(withTasks());
    await userEvent.click(row(tree, "deep"));
    await waitFor(() => expect(onScreen()).toEqual([7]));

    expect(crumbsSay()).toMatch(/^steward 1 › talk › deep · /);
    // The chat shown is where the person is: it is not a way to anywhere.
    expect(within(crumbs() as HTMLElement).queryByRole("button", { name: "deep" })).toBeNull();

    await userEvent.click(within(crumbs() as HTMLElement).getByRole("button", { name: "talk" }));

    await waitFor(() => expect(onScreen()).toEqual([4]));
    expect(crumbsSay()).toMatch(/^steward 1 › talk · /);
  });

  it("says the workspace of a task that works in another one, which brings that workspace forward (#4757)", async () => {
    const { tree } = await drawn(withTasks());

    await userEvent.click(row(tree, "probe"));

    await waitFor(() => expect(onScreen()).toEqual([6]));
    // probe works in beta: beta's strip comes forward. It has no tab yet, so it gets one until
    // every workspace keeps its own (#4758).
    expect(tabNames()).toEqual(["probe"]);
    expect(crumbsSay()).toMatch(/^steward 1 › probe in beta · /);
  });

  // #6213: a coordinator names chats by number, so the palette finds one by it, `6` or `#6`,
  // whichever workspace it works in, and goes there as its row's press does.
  it("is found in the palette by its number, bare or with #, and shown in its own workspace", async () => {
    await drawn(withTasks());
    const found = async (query: string) => {
      await userEvent.keyboard("{F2}");
      const palette = await screen.findByRole("dialog", { name: "Command palette" });
      const box = within(palette).getByRole("combobox");
      await userEvent.clear(box);
      await userEvent.type(box, query);
      return within(palette).getAllByRole("option");
    };

    expect((await found("6"))[0].textContent).toContain("Show task probe in beta");
    await userEvent.keyboard("{Escape}");
    const options = await found("#6");
    expect(options[0].textContent).toContain("Show task probe in beta");
    await userEvent.keyboard("{Enter}");

    await waitFor(() => expect(onScreen()).toEqual([6]));
    expect(crumbsSay()).toMatch(/^steward 1 › probe in beta · /);
  });

  // #6213: a plain chat of another workspace, with no tab in this window, is found by its
  // number as a task is, and its row shows it.
  it("finds a plain chat of another workspace with no tab here by its number", async () => {
    const open = [...withTasks(), chat(8, "beta", { label: "notes" })];
    core(open, tabbed(withTasks()));
    render(<App />);
    // Its Chats list is this workspace's, so it lists the tasks and not chat 8.
    const tree = await section();
    await waitFor(() => expect(rows(tree)).toHaveLength(withTasks().length));
    await userEvent.keyboard("{F2}");
    const palette = await screen.findByRole("dialog", { name: "Command palette" });
    await userEvent.type(within(palette).getByRole("combobox"), "#8");

    const [first] = within(palette).getAllByRole("option");
    expect(first.textContent).toContain("Show chat notes in beta");
    await userEvent.keyboard("{Enter}");

    await waitFor(() => expect(onScreen()).toEqual([8]));
  });

  it("says nothing of a workspace for a task that works where its session does", async () => {
    const { tree } = await drawn(withTasks());
    await userEvent.click(row(tree, "talk"));
    await waitFor(() => expect(onScreen()).toEqual([4]));
    expect(crumbsSay()).not.toContain(" in ");
  });
});

describe("the tab's label", () => {
  it("stays on the task when the tab is pressed to bring it to the front", async () => {
    const { tree } = await drawn(withTasks());
    await userEvent.click(row(tree, "talk"));
    await waitFor(() => expect(onScreen()).toEqual([4]));
    await userEvent.click(tab("steward 2"));
    await waitFor(() => expect(onScreen()).toEqual([2]));

    // One press brings it forward, on the task its pane was given (#4757), and a second does
    // not take the pane anywhere else.
    await userEvent.click(tab("talk"));
    await waitFor(() => expect(onScreen()).toEqual([4]));
    await userEvent.click(tab("talk"));
    await new Promise((resolve) => setTimeout(resolve, 400));
    expect(onScreen()).toEqual([4]);
  });

  it("stays on the task when the tab in front is double-clicked to rename it", async () => {
    const { tree } = await drawn(withTasks());
    await userEvent.click(row(tree, "talk"));
    await waitFor(() => expect(onScreen()).toEqual([4]));

    await userEvent.dblClick(tab("talk"));

    // Its Rename is open, and it is the task's: the tab is named for what its pane shows
    // (#4757, #4762), and the chat it renames is that one, never the session's (#5287, #5343).
    const asking = await screen.findByRole("dialog", { name: "Rename talk" });
    expect(within(asking).getByLabelText("New name")).toHaveValue("talk");
    expect(
      within(asking).getByRole("checkbox", { name: "Only this tab" }),
    ).toHaveAccessibleDescription("The chat keeps its name, talk.");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await new Promise((resolve) => setTimeout(resolve, 400));
    expect(onScreen()).toEqual([4]);
    expect(crumbsSay()).toMatch(/^steward 1 › talk · /);
  });

  it("stays on the task when the tab in front is dragged along the strip", async () => {
    const { tree } = await drawn(withTasks());
    await userEvent.click(row(tree, "talk"));
    await waitFor(() => expect(onScreen()).toEqual([4]));

    // Picked up, carried past the tab beside it and put down, from the keyboard: the drag's
    // own keys are not a press of the tab.
    //
    // The strip is in the editor region since DS-6 (#1836), whose panel scrolls, so dnd-kit's
    // keyboard sensor brings the tab into view as the drag starts — a tab jsdom draws at 0×0,
    // so always. jsdom has no `scrollIntoView`, and other files rely on that, so one is planted
    // for this drag only (as `Palette.test.tsx` does).
    Object.defineProperty(Element.prototype, "scrollIntoView", {
      configurable: true,
      value: () => {},
    });
    try {
      const dragged = tab("talk");
      dragged.focus();
      await userEvent.keyboard("{Shift>}[Space]{/Shift}");
      await userEvent.keyboard("{ArrowRight}");
      await userEvent.keyboard("[Space]");
      await new Promise((resolve) => setTimeout(resolve, 400));
    } finally {
      Reflect.deleteProperty(Element.prototype, "scrollIntoView");
    }

    // The pane still shows the task it was given (#4757).
    expect(onScreen()).toEqual([4]);
    expect(tab("talk")).toBeTruthy();
  });
});

describe("a task that needs you and is not on screen", () => {
  it("puts the hand on the tab it returns to, and switches nothing", async () => {
    const { move } = await drawn(withTasks());
    expect(handOn("steward 1")).toBeNull();

    await move(5, "waiting", 10, [5]);

    expect(handSays("steward 1")).toBe("sweep needs you");
    expect(handOn("steward 2")).toBeNull();
    // Nothing moved: the session's own chat is still what is on screen.
    expect(onScreen()).toEqual([1]);
  });

  it("puts the hand on the tab its pane shows it in, once that tab is not in front (#4757)", async () => {
    const { tree, move } = await drawn(withTasks());
    await userEvent.click(row(tree, "talk"));
    await waitFor(() => expect(onScreen()).toEqual([4]));
    // On screen in the tab in front: the pane is what says it, and the tab wears nothing.
    await move(4, "waiting", 10, [4]);
    expect(handOn("talk")).toBeNull();

    await userEvent.click(tab("steward 2"));
    await waitFor(() => expect(onScreen()).toEqual([2]));

    // talk is the tab's own chat now, so its state mark says it, and no hand stands for it.
    expect(within(tab("talk")).queryByRole("img", { name: "needs you" })).not.toBeNull();
    expect(handOn("talk")).toBeNull();
  });

  it("is gone to from the Inbox the title bar opens, which shows it in the focused pane (#4757)", async () => {
    const { move } = await drawn(withTasks());
    await userEvent.click(tab("steward 2"));
    await waitFor(() => expect(onScreen()).toEqual([2]));
    await move(5, "waiting", 10, [5]);

    // The hand opens the Inbox (#1695), and its Go to chat opens the task in its tab.
    await userEvent.click(
      await screen.findByRole("button", { name: /^1 (thing waits|chat needs)/ }),
    );
    const inbox = await screen.findByRole("tabpanel", { name: "Asks" });
    await userEvent.click(
      await within(inbox).findByRole("button", { name: /^Go to chat .*sweep/ }),
    );

    // A Go is a press (#4757): it lands in the focused pane of the tab in front.
    await waitFor(() => expect(onScreen()).toEqual([5]));
    expect(tabNames()).toEqual(["steward 1", "sweep"]);
    expect(tab("sweep").getAttribute("aria-selected")).toBe("true");
    // It is on screen now, so the tab wears no hand for it.
    expect(handOn("sweep")).toBeNull();
  });

  it("is gone to from the hand on a row above it in the Chats list", async () => {
    const { tree, move } = await drawn(withTasks());
    await move(7, "waiting", 10, [7]);

    // A control of steward 1's row (#3198).
    const hand = row(tree, "steward 1").querySelector<HTMLElement>(".rolled-up");
    if (!hand) throw new Error("steward 1 wears no hand for the task below it");
    await userEvent.click(hand);

    // In the focused pane, which was steward 1's (#4757).
    await waitFor(() => expect(onScreen()).toEqual([7]));
    expect(tabNames()).toEqual(["deep", "steward 2"]);
  });

  it("leaves the tab where it is after the prompt is answered", async () => {
    const { tree, move } = await drawn(withTasks());
    await move(5, "waiting", 10, [5]);
    await userEvent.click(row(tree, "sweep"));
    await waitFor(() => expect(onScreen()).toEqual([5]));

    // Answered: the task works again, and later ends its turn with nothing to ask.
    await move(5, "running", 11, []);
    await move(5, "waiting", 12, []);
    await move(1, "running", 13, []);

    expect(onScreen()).toEqual([5]);
    expect(crumbsSay()).toMatch(/^steward 1 › sweep · /);
  });

  it("is worn by the tab it returns to, and counted on that tab's workspace, when it works in another", async () => {
    const { move } = await drawn(withTasks());
    await move(6, "waiting", 10, [6]);

    // probe works in beta, and is a task of steward 1 in alpha: that tab, on alpha's strip.
    expect(handSays("steward 1")).toBe("probe needs you");
    // And alpha's chip in the title bar counts it, where beta's row has no tab that wears it.
    expect(chipSays("alpha")).toBe("alpha, 1 waiting");
    expect(await rowSays("beta")).toBe("beta");
  });
});

describe("a Notice of a chat that is not the one its tab shows", () => {
  it("goes to its chat on Go to it", async () => {
    const { holdDispatch } = await drawn(withTasks());
    // sweep is in no pane: its Notice is drawn on its session's pane (#4757: a press of Go to
    // it shows sweep in the focused pane).
    await holdDispatch(5);
    const held = await screen.findByRole("status", {
      name: "“sweep” (a task of “steward 1”): Dispatch to devops",
    });

    await userEvent.click(within(held).getByRole("button", { name: "Go to it" }));

    await waitFor(() => expect(onScreen()).toEqual([5]));
    // On its own pane it reads as it always did: no name before it, and no way to itself.
    const own = await screen.findByRole("status", { name: "Dispatch to devops" });
    expect(own.textContent).toMatch(/^This chat runs as steward and wants to dispatch/);
    expect(within(own).queryByRole("button", { name: "Go to it" })).toBeNull();
  });

  it("is drawn for a task while the tab shows the session's own chat, and names the task and its session", async () => {
    const { asked, holdDispatch } = await drawn(withTasks());
    expect(onScreen()).toEqual([1]);

    await holdDispatch(5);

    const held = await screen.findByRole("status", {
      name: "“sweep” (a task of “steward 1”): Dispatch to devops",
    });
    await userEvent.click(within(held).getByRole("button", { name: "Keep blocked" }));
    await waitFor(() =>
      expect(commandsOf(asked, "keep_dispatch_blocked")).toEqual([{ plane: PLANE, id: 75 }]),
    );
    expect(onScreen()).toEqual([1]);
  });

  it("is drawn for a vault a task was refused, and Keep blocked is sent for the task", async () => {
    const { asked, refuseVault } = await drawn(withTasks());

    await refuseVault(5);

    const refused = await screen.findByRole("status", {
      name: "“sweep” (a task of “steward 1”): Vault",
    });
    await userEvent.click(within(refused).getByRole("button", { name: "Keep blocked" }));
    await waitFor(() =>
      expect(commandsOf(asked, "keep_vault_blocked")).toEqual([
        { plane: PLANE, session: 5, vault: "prod", reason: null },
      ]),
    );
  });

  it("is drawn for a sandbox block of a hidden chat, and dismissing it dismisses that chat's", async () => {
    // sweep is in no pane (#4757): its block is drawn on its session's pane.
    const { block } = await drawn(withTasks());

    await block(5);

    const said = await screen.findByRole("status", {
      name: "“sweep” (a task of “steward 1”): Sandbox block",
    });
    expect(handSays("steward 1")).toBe("sweep needs you");
    await userEvent.click(within(said).getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(notice(/Sandbox block$/)).toBeNull());
    expect(handOn("steward 1")).toBeNull();
  });

  it("puts the hand on the tab, in front or behind, until it is answered", async () => {
    // The two asking are tasks in no pane (#4757), whose Notices steward 1's pane draws.
    const { tree, holdDispatch, refuseVault } = await drawn(withTasks());

    // A held dispatch is not in the core's needs-you queue: the tab is what says it.
    await holdDispatch(4);
    await waitFor(() => expect(handSays("steward 1")).toBe("talk needs you"));
    // And another task's refusal, on a tab that is behind and whose pane is not drawn at all.
    await userEvent.click(tab("steward 2"));
    await waitFor(() => expect(onScreen()).toEqual([2]));
    await refuseVault(5);
    await waitFor(() =>
      expect(handSays("steward 1")).toMatch(/^(sweep|talk) and \d more need you$/),
    );

    // Answered in its own pane, which a press gives it (#4757): the hand for it goes.
    await userEvent.click(row(tree, "talk"));
    await waitFor(() => expect(onScreen()).toEqual([4]));
    const held = await screen.findByRole("status", { name: "Dispatch to devops" });
    await userEvent.click(within(held).getByRole("button", { name: "Keep blocked" }));
    await waitFor(() => expect(handSays("steward 1")).toBe("sweep needs you"));
  });

  it("does not carry what one chat's Notice was answered under another chat when the tab is switched", async () => {
    const { tree, holdDispatch } = await drawn(withTasks());
    await holdDispatch(1);
    const own = await screen.findByRole("status", { name: "Dispatch to devops" });
    await userEvent.click(within(own).getByRole("button", { name: "Allow for this chat" }));
    // steward 1's Notice says what the Allow answered, on steward 1's pane.
    await waitFor(() => expect(notice("Dispatch to devops")?.textContent).toContain("Allowed"));

    await userEvent.click(row(tree, "talk"));
    await waitFor(() => expect(onScreen()).toEqual([4]));

    // talk asked for nothing and was answered nothing: its pane says nothing of a dispatch.
    expect(notice("Dispatch to devops")).toBeNull();
    // And when talk does ask, it is asked, not told what steward 1 was answered.
    await holdDispatch(4);
    const asks = await screen.findByRole("status", { name: "Dispatch to devops" });
    expect(asks.textContent).toContain("Nothing starts until you answer");
    expect(asks.textContent).not.toContain("Allowed");
  });
});

describe("typing while a task is shown", () => {
  it("goes to that task and to no other chat, and the keyboard is in its pane after the switch", async () => {
    const { tree, asked } = await drawn(withTasks());

    await userEvent.click(row(tree, "talk"));
    await waitFor(() => expect(onScreen()).toEqual([4]));

    // The switch put the keyboard in the task's pane: nothing is clicked before typing.
    await waitFor(() => expect(document.activeElement?.getAttribute("data-session")).toBe("4"));
    await userEvent.keyboard("ok");

    expect(commandsOf(asked, "send_input")).toEqual([
      { plane: PLANE, session: 4, text: "o" },
      { plane: PLANE, session: 4, text: "k" },
    ]);
  });

  it("goes to the session's own chat again once the tab is back on it", async () => {
    const { tree, asked } = await drawn(withTasks());
    await userEvent.click(row(tree, "talk"));
    await waitFor(() => expect(onScreen()).toEqual([4]));
    await userEvent.click(
      within(crumbs() as HTMLElement).getByRole("button", { name: "steward 1" }),
    );
    await waitFor(() => expect(onScreen()).toEqual([1]));

    await waitFor(() => expect(document.activeElement?.getAttribute("data-session")).toBe("1"));
    await userEvent.keyboard("y");

    expect(commandsOf(asked, "send_input")).toEqual([{ plane: PLANE, session: 1, text: "y" }]);
  });

  it("reaches no chat while the task shown has ended", async () => {
    const { tree, asked, ended } = await drawn(withTasks());
    await userEvent.click(row(tree, "sweep"));
    await waitFor(() => expect(onScreen()).toEqual([5]));
    await ended(5);
    await band();

    await userEvent.keyboard("x");

    expect(commandsOf(asked, "send_input")).toEqual([]);
  });
});

describe("a file dropped on a tab", () => {
  const reference = {
    plane: PLANE,
    workspace: "alpha",
    repo: "svc",
    piece: null,
    path: "src/main.rs",
    folder: false,
  };
  const dropOn = (target: Element) =>
    fireEvent.drop(target, {
      dataTransfer: {
        types: [REFERENCE_TYPE],
        getData: (type: string) => (type === REFERENCE_TYPE ? JSON.stringify(reference) : ""),
      },
    });

  it("goes to the task the tab's pane shows and reads, and the sentence names that task", async () => {
    const { tree, asked } = await drawn(withTasks());
    await userEvent.click(row(tree, "talk"));
    await waitFor(() => expect(onScreen()).toEqual([4]));

    // The tab is talk's now: its pane was given talk (#4757).
    dropOn(tab("talk").closest(".tab") as Element);

    await waitFor(() =>
      expect(commandsOf(asked, "reference_into_chat").map((sent) => sent.session)).toEqual([4]),
    );
    expect(await findResult(/Typed @src\/main\.rs into talk\. Nothing was sent\./)).toBeTruthy();
  });

  it("goes to the session's own chat while the tab shows it", async () => {
    const { asked } = await drawn(withTasks());

    dropOn(tab("steward 1").closest(".tab") as Element);

    await waitFor(() =>
      expect(commandsOf(asked, "reference_into_chat").map((sent) => sent.session)).toEqual([1]),
    );
  });
});

describe("closing, while a tab shows a task", () => {
  it("closes the tab a press gave the task, asking nothing and ending neither chat (#4756)", async () => {
    const { tree, asked } = await drawn(withTasks());
    await userEvent.click(row(tree, "talk"));
    await waitFor(() => expect(onScreen()).toEqual([4]));

    // The tab is talk's now: its pane was given talk (#4757), and a task's tab closes by its
    // minimise.
    const close = tab("talk")
      .closest(".tab")
      ?.querySelector<HTMLElement>('button[data-part="closer"]');
    expect(close?.getAttribute("aria-label")).toBe("Send talk back to its row");
    await userEvent.click(close as HTMLElement);

    await waitFor(() => expect(tabNames()).toEqual(["steward 2"]));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(commandsOf(asked, "close_session")).toEqual([]);
    // Both go on in the sidebar.
    expect(row(tree, "steward 1")).toBeTruthy();
    expect(row(tree, "talk")).toBeTruthy();
  });

  it("ends the session, not the task, by the session's End… (#4753)", async () => {
    const { tree, asked } = await drawn(withTasks());
    await userEvent.click(row(tree, "talk"));
    await waitFor(() => expect(onScreen()).toEqual([4]));

    const question = await askToEnd("steward 1");
    expect(question.textContent).not.toContain("This tab is showing");
    expect(commandsOf(asked, "close_session")).toEqual([]);
    await userEvent.click(within(question).getByRole("button", { name: "Close" }));

    await waitFor(() =>
      expect(commandsOf(asked, "close_session")).toEqual([{ plane: PLANE, session: 1 }]),
    );
  });

  it("draws no close on the pane, and the task's two ways to end are rows of its ⋯ (#4760)", async () => {
    const { tree } = await drawn(withTasks());
    await userEvent.click(row(tree, "talk"));
    await waitFor(() => expect(onScreen()).toEqual([4]));

    expect(screen.queryByRole("button", { name: "Close this pane" })).toBeNull();
    const ending = screen
      .queryAllByRole("button")
      .map((button) => button.getAttribute("aria-label") ?? button.textContent ?? "")
      .filter((name) => /\b(end|stop|close)\b/i.test(name) && /talk/.test(name));
    // The two ways a person ends a task (#1488) are its list's rows, in its ⋯ (#4760): no
    // button on screen ends the task.
    expect(ending).toEqual([]);
    await userEvent.click(paneMore(/^More for talk/));
    const list = await screen.findByRole("menu");
    expect(within(list).getByRole("menuitem", { name: /^Stop and get its report/ })).toBeTruthy();
    expect(within(list).getByRole("menuitem", { name: /^Close now/ })).toBeTruthy();
    await userEvent.keyboard("{Escape}");
    // Its head's right-click lists the pane's rows, and no Close pane where the head draws none.
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    fireEvent.contextMenu(
      screen.getByRole("navigation", { name: "Chat path" }).closest(".pane-chips") as HTMLElement,
    );
    const rows = [...(await screen.findByRole("menu")).querySelectorAll('[role^="menuitem"]')].map(
      (one) => one.getAttribute("aria-label") ?? one.textContent,
    );
    expect(rows).toContain("Split right");
    expect(rows).not.toContain("End this pane's chat");
    // The splits are the pane's still.
    expect(screen.getAllByRole("button", { name: /^Split/ })).toHaveLength(2);
  });
});

/** The chat `asker` handed its work off to: a session of its own, with a tab. */
const handoffFrom = (asker: number, more: Partial<Lineage> = {}): Lineage => ({
  ...taskOf(asker),
  task: false,
  tab: true,
  ...more,
});

/** The rows of the list as `level name`, top to bottom. */
const levels = (tree: HTMLElement) =>
  rows(tree).map(
    (one) => `${one.getAttribute("aria-level")} ${one.querySelector(".session")?.textContent}`,
  );

/** The name of the tab in front. */
const inFront = () =>
  within(strip())
    .getAllByRole("tab")
    .find((one) => one.getAttribute("aria-selected") === "true")
    ?.querySelector(".tab-name")?.textContent;

describe("a handoff is a session of its own (#1492, V100-69)", () => {
  /** steward 1 asked talk (4) for a task, and handed work off to drop commons (3). */
  const moved = () => [
    chat(1, "alpha"),
    chat(3, "alpha", { label: "drop commons", from: handoffFrom(1) }),
    chat(4, "alpha", { persona: "devops", label: "talk", from: taskOf(1) }),
  ];

  /** What the card of row `name` says (#1675). */
  const cardOf = async (tree: HTMLElement, name: string) => (await cardOfRow(row(tree, name))).text;

  it("has its own tab with a close, and its own row at the top, under no chat", async () => {
    const { tree, asked } = await drawn(moved());

    expect(tabNames()).toEqual(["steward 1", "drop commons"]);
    const close = tab("drop commons").closest(".tab")?.querySelector('button[data-part="closer"]');
    expect(close?.getAttribute("aria-label")).toBe("Close drop commons");
    // The task is under the chat that asked; the handoff is beside it.
    expect(levels(tree)).toEqual(["1 steward 1", "2 talk", "1 drop commons"]);
    expect(onScreen()).toEqual([1]);

    // Pressing its row brings its own tab forward. Nothing is shown inside steward 1's tab.
    await userEvent.click(row(tree, "drop commons").querySelector(".session") as HTMLElement);

    await waitFor(() => expect(onScreen()).toEqual([3]));
    expect(inFront()).toBe("drop commons");
    expect(tabNames()).toEqual(["steward 1", "drop commons"]);
    expect(crumbs()).toBeNull();
    expect(commandsOf(asked, "open_chat_tab")).toEqual([]);
    expect(onScreenSaid(asked)).toEqual([3]);
    expect(laidSaid(asked)?.tabs.map((tab) => chatsOf(tab.tree))).toEqual([[1], [3]]);

    // And a task of the chat it came from lands in the focused pane like any chat (#4757).
    await userEvent.click(row(tree, "talk").querySelector(".session") as HTMLElement);
    await waitFor(() => expect(onScreen()).toEqual([4]));
    expect(inFront()).toBe("talk");
    expect(crumbsSay()).toContain("steward 1 › talk");
  });

  it("is in nobody's counts: it folds under no row and raises no hand but its own", async () => {
    const { tree, move } = await drawn([
      chat(1, "alpha"),
      chat(3, "alpha", { label: "drop commons", from: handoffFrom(1) }),
    ]);
    // Nothing is under the chat it came from: that row has nothing to fold or to sum up.
    expect(row(tree, "steward 1").getAttribute("aria-expanded")).toBeNull();
    expect(row(tree, "steward 1").querySelector('[data-part="twist"]')).toBeNull();
    expect(row(tree, "steward 1").querySelector(".below-summary")).toBeNull();

    await move(3, "waiting", 10, [3]);

    expect(within(row(tree, "drop commons")).getByText("needs you")).toBeTruthy();
    // The chat it came from is not waiting for anybody, on its row or on its tab.
    expect(row(tree, "steward 1").querySelector("button.rolled-up")).toBeNull();
    expect(row(tree, "steward 1").getAttribute("aria-description")).not.toContain("needs you");
    expect(handOn("steward 1")).toBeNull();
    // Nor does that chat's tab offer it as one of its own chats.
    expect(within(tab("steward 1")).queryByText(/drop commons/)).toBeNull();
  });

  it("is named in the card of the chat it came from, which names that chat in its own", async () => {
    const { tree } = await drawn([
      ...moved(),
      chat(5, "alpha", { label: "release notes", from: handoffFrom(1) }),
    ]);

    // The newest first, and how many more, in the card and not on the row (#1675).
    expect(await cardOf(tree, "steward 1")).toContain("handed off to release notes and 1 more");
    expect(row(tree, "steward 1").textContent).not.toContain("handed off");
    // Each handed-off chat says where it came from, and hands nothing off itself.
    expect(await cardOf(tree, "drop commons")).toContain("from steward 1");
    expect(await cardOf(tree, "release notes")).toContain("from steward 1");
    expect(await cardOf(tree, "drop commons")).not.toContain("handed off");
    // A task says nothing of the kind: it is under the chat that asked.
    expect(await cardOf(tree, "talk")).not.toContain("from steward 1");
  });

  it("is gone to from the row's menu, which has a row for each chat the work went to", async () => {
    const { tree } = await drawn([
      ...moved(),
      chat(5, "alpha", { label: "release notes", from: handoffFrom(1) }),
    ]);

    fireEvent.contextMenu(row(tree, "steward 1"));

    // Every chat it went to, the newest first: the one the row names and the "1 more".
    const menu = await screen.findByRole("menu");
    // Each names the chat it goes to, which is another chat than the row's: the one name a
    // row of this menu says.
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map(rowName)
        .filter((said) => said?.startsWith("Go to ")),
    ).toEqual(["Go to release notes", "Go to drop commons"]);
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Go to drop commons" }));

    await waitFor(() => expect(onScreen()).toEqual([3]));
    expect(inFront()).toBe("drop commons");

    // A chat that handed nothing off has no such row.
    fireEvent.contextMenu(row(tree, "talk"));
    const other = await screen.findByRole("menu");
    expect(within(other).queryByText(/handed off to/)).toBeNull();
  });

  it("is still named in the card of the chat it came from while that chat works", async () => {
    const { tree, move } = await drawn(moved());

    await move(1, "running", 10);

    expect(within(row(tree, "steward 1")).getByText("working")).toBeTruthy();
    expect(await cardOf(tree, "steward 1")).toContain("handed off to drop commons");
    // Where it came from is always said.
    expect(await cardOf(tree, "drop commons")).toContain("from steward 1");
  });

  // One rename to a test: each card read waits out the card's own rest and leave, real time.
  /** Renames the chat called `was`, on its tab, as a person does. */
  const rename = async (was: string, to: string) => {
    await userEvent.dblClick(tab(was));
    const asking = await screen.findByRole("dialog", { name: `Rename ${was}` });
    await waitFor(() => expect(within(asking).getByLabelText("New name")).toHaveFocus());
    await userEvent.keyboard(`{Control>}a{/Control}${to}{Enter}`);
    await waitFor(() => expect(tabNames()).toContain(to));
  };

  it("follows a rename of the chat it went to", async () => {
    const { tree } = await drawn(moved());

    await rename("drop commons", "ship it");
    await waitFor(() => expect(row(tree, "ship it")).toBeTruthy());
    expect(await cardOf(tree, "steward 1")).toContain("handed off to ship it");
  });

  it("follows a rename of the chat it came from", async () => {
    const { tree } = await drawn(moved());

    await rename("steward 1", "lead");
    await waitFor(() => expect(row(tree, "lead")).toBeTruthy());
    expect(await cardOf(tree, "drop commons")).toContain("from lead");
    expect(await cardOf(tree, "lead")).toContain("handed off to drop commons");
  });
});

describe("a chat the person asked for from a tab (#1492, V100-70)", () => {
  /** In steward 1's tab the person asked devops (talk, 4); steward 1 dispatched sweep (5). */
  const asked = () => [
    chat(1, "alpha"),
    chat(2, "alpha"),
    chat(4, "alpha", { persona: "devops", label: "talk", from: taskOf(1, { by_person: true }) }),
    chat(5, "alpha", { persona: "devops", label: "sweep", from: taskOf(1) }),
  ];

  it("is a task of that session, nested under it and marked as the person's in its card", async () => {
    const { tree } = await drawn(asked());

    expect(levels(tree)).toEqual(["1 steward 1", "2 talk", "2 sweep", "1 steward 2"]);
    expect(tabNames()).toEqual(["steward 1", "steward 2"]);
    // In its card (#1675), and not on its one line.
    expect((await cardOfRow(row(tree, "talk"))).text).toContain("asked by you");
    expect(row(tree, "talk").textContent).not.toContain("asked by you");
    // A task the chat dispatched itself, and the session's own chat, are not marked.
    expect((await cardOfRow(row(tree, "sweep"))).text).not.toContain("asked by you");
    expect((await cardOfRow(row(tree, "steward 1"))).text).not.toContain("asked by you");
  });

  it("opens in the focused pane, and its breadcrumb says whose it is after its state (#4757)", async () => {
    const { tree } = await drawn(asked());

    await userEvent.click(row(tree, "talk"));

    await waitFor(() => expect(onScreen()).toEqual([4]));
    expect(tabNames()).toEqual(["talk", "steward 2"]);
    expect(inFront()).toBe("talk");
    expect(crumbsSay()).toBe("steward 1 › talk · running (no detail from claude) · asked by you");

    await userEvent.click(row(tree, "sweep"));

    await waitFor(() => expect(onScreen()).toEqual([5]));
    expect(crumbsSay()).toBe("steward 1 › sweep · running (no detail from claude)");
  });
});

describe("what a task asks, on the pane it returns to (#1508)", () => {
  /** The clock the questions read, moved by hand: a press too soon after a question is drawn
   *  does nothing. */
  const clock = { now: 1_000_000 };
  beforeEach(() => {
    clock.now = 1_000_000;
    vi.spyOn(Date, "now").mockImplementation(() => clock.now);
    vi.spyOn(performance, "now").mockImplementation(() => clock.now);
  });
  afterEach(() => vi.restoreAllMocks());
  /** Time enough for a question to settle. */
  const settle = () => {
    clock.now += 5_000;
  };

  /** What the window sends back for task `task` blocked on the npm registry. */
  const seenOn = (task: number) => ({
    task,
    operation: "connect",
    kind: "host",
    what: "host",
    target: "registry.npmjs.org",
  });

  it("names a task two levels down by its whole path, each name quoted", async () => {
    const { holdDispatch } = await drawn(withTasks());
    expect(onScreen()).toEqual([1]);

    await holdDispatch(7);

    // Reachable from the pane it returns to without opening the task, and named from the core's
    // record of who asked whom.
    expect(
      await screen.findByRole("status", {
        name: "“deep” (a task of “steward 1” › “talk”): Dispatch to devops",
      }),
    ).toBeTruthy();
    expect(onScreen()).toEqual([1]);
  });

  it("asks once for three tasks blocked on one host, and one answer restarts all three", async () => {
    const { asked, blockHost, move } = await drawn(withTasks());

    await blockHost(4);
    // The same host as a grant matches it: one question.
    await blockHost(5, "Registry.npmjs.org:443");
    await blockHost(6);
    settle();

    const question = await screen.findByRole("status", { name: "Sandbox block for 3 tasks" });
    expect(question.textContent).toContain("3 tasks want to reach registry.npmjs.org");
    expect(question.textContent).toContain(
      "“talk” (a task of “steward 1”), “sweep” (a task of “steward 1”) and “probe” (a task of “steward 1”)",
    );
    // One question: no task asks it again on its own.
    expect(screen.queryAllByRole("status", { name: /Sandbox block$/ })).toEqual([]);

    await userEvent.click(within(question).getByRole("button", { name: "Other scopes…" }));
    await userEvent.click(screen.getByRole("button", { name: "Allow only for these 3 tasks" }));

    // Each task with the block it was shown blocked on, exactly.
    await waitFor(() =>
      expect(commandsOf(asked, "allow_sandbox_block_for_tasks")).toEqual([
        {
          plane: PLANE,
          session: 1,
          seen: [seenOn(4), { ...seenOn(5), target: "Registry.npmjs.org:443" }, seenOn(6)],
          level: "chat",
        },
      ]),
    );
    // Nothing was allowed for the session that asked them.
    expect(commandsOf(asked, "allow_sandbox_block")).toEqual([]);
    await waitFor(() => expect(notice("Sandbox block for 3 tasks")).toBeNull());
    // What the answer did only says something, so it stands behind "+N more" while the
    // pane has two that ask (#1647): still drawn, and read there.
    expect(
      screen.queryByRole("status", { name: "Sandbox block", hidden: true })?.textContent,
    ).toContain("Allowed for each");

    // Each restarts on its conversation once its turn ends.
    for (const task of [4, 5, 6]) await move(task, "waiting", 20 + task);
    await waitFor(() =>
      expect(
        commandsOf(asked, "restart_chat")
          .map((one) => one.session)
          .sort(),
      ).toEqual([4, 5, 6]),
    );
  });

  it("does nothing on a press as the question forms, where a task's own Allow was a moment ago", async () => {
    const { asked, blockHost } = await drawn(withTasks());
    await blockHost(4);
    expect(
      await screen.findByRole("status", { name: "“talk” (a task of “steward 1”): Sandbox block" }),
    ).toBeTruthy();
    settle();

    // sweep is blocked the moment before the person presses talk's own Allow: the question
    // for both is under the pointer now, and pressing it allows nothing.
    await blockHost(5);
    const question = await screen.findByRole("status", { name: "Sandbox block for 2 tasks" });
    await userEvent.click(within(question).getByRole("button", { name: "Other scopes…" }));
    await userEvent.click(screen.getByRole("button", { name: "Allow only for these 2 tasks" }));
    expect(question.textContent).toContain("nothing was answered");
    await userEvent.click(within(question).getByRole("button", { name: "Keep blocked" }));
    expect(commandsOf(asked, "allow_sandbox_block_for_tasks")).toEqual([]);
    expect(commandsOf(asked, "keep_sandbox_block_for_tasks")).toEqual([]);
    expect(commandsOf(asked, "allow_sandbox_block")).toEqual([]);
  });

  it("keeps the session's own block apart, and asks it on its own", async () => {
    const { asked, blockHost } = await drawn(withTasks());

    await blockHost(1);
    await blockHost(4);
    await blockHost(5);

    expect(await screen.findByRole("status", { name: "Sandbox block for 2 tasks" })).toBeTruthy();
    const own = await screen.findByRole("status", { name: "Sandbox block" });
    await userEvent.click(
      within(own).getByRole("button", { name: "Allow for me on this machine" }),
    );
    await waitFor(() =>
      expect(commandsOf(asked, "allow_sandbox_block")).toEqual([
        {
          plane: PLANE,
          session: 1,
          shown: { operation: "connect", kind: "host", what: "host", target: "registry.npmjs.org" },
          level: "you",
        },
      ]),
    );
    // The tasks' question still stands: the session's answer did not reach them.
    expect(notice("Sandbox block for 2 tasks")).toBeTruthy();
    expect(commandsOf(asked, "allow_sandbox_block_for_tasks")).toEqual([]);
  });

  it("says who joined after it was shown, and an answer pressed just then grants nothing", async () => {
    const { asked, blockHost } = await drawn(withTasks());
    await blockHost(4);
    await blockHost(5);
    await screen.findByRole("status", { name: "Sandbox block for 2 tasks" });
    settle();

    // probe is blocked too, the moment before the person presses.
    await blockHost(6);
    const question = await screen.findByRole("status", { name: "Sandbox block for 3 tasks" });
    expect(question.textContent).toContain(
      "“probe” (a task of “steward 1”) joined this question after it was first shown.",
    );
    await userEvent.click(within(question).getByRole("button", { name: "Other scopes…" }));
    await userEvent.click(screen.getByRole("button", { name: "Allow only for these 3 tasks" }));

    expect(question.textContent).toContain("nothing was answered");
    expect(commandsOf(asked, "allow_sandbox_block_for_tasks")).toEqual([]);

    // Read, and answered again: it is the three it now shows.
    settle();
    await userEvent.click(screen.getByRole("button", { name: "Allow only for these 3 tasks" }));
    await waitFor(() =>
      expect(
        commandsOf(asked, "allow_sandbox_block_for_tasks").map((one) =>
          (one.seen as { task: number }[]).map((seen) => seen.task),
        ),
      ).toEqual([[4, 5, 6]]),
    );
  });

  it("puts away only the tasks an answer allowed when keeping failed part way", async () => {
    const { answering, blockHost } = await drawn(withTasks());
    await blockHost(4);
    await blockHost(5);
    const question = await screen.findByRole("status", { name: "Sandbox block for 2 tasks" });
    settle();
    answering.only = [4];

    await userEvent.click(within(question).getByRole("button", { name: "Other scopes…" }));
    await userEvent.click(screen.getByRole("button", { name: "Allow only for these 2 tasks" }));

    // sweep was not allowed: its block is still asked, on its own.
    expect(
      await screen.findByRole("status", { name: "“sweep” (a task of “steward 1”): Sandbox block" }),
    ).toBeTruthy();
    expect(notice("“talk” (a task of “steward 1”): Sandbox block")).toBeNull();
    // What the answer did only says something, so it stands behind "+N more" while the
    // pane has two that ask (#1647): still drawn, and read there.
    expect(
      screen.queryByRole("status", { name: "Sandbox block", hidden: true })?.textContent,
    ).toContain("allowed for chat 4 only");
  });

  it("asks a task blocked after the answer in a new question, and Keep blocked answers each listed", async () => {
    const { asked, blockHost } = await drawn(withTasks());
    await blockHost(4);
    await blockHost(5);
    const question = await screen.findByRole("status", { name: "Sandbox block for 2 tasks" });
    settle();

    await userEvent.click(within(question).getByRole("button", { name: "Keep blocked" }));
    await waitFor(() =>
      expect(commandsOf(asked, "keep_sandbox_block_for_tasks")).toEqual([
        { plane: PLANE, session: 1, seen: [seenOn(4), seenOn(5)], reason: null },
      ]),
    );
    await waitFor(() => expect(notice("Sandbox block for 2 tasks")).toBeNull());
    expect(screen.queryAllByRole("status", { name: /Sandbox block/ })).toEqual([]);

    // probe is blocked afterwards: it is asked on its own, and was answered nothing.
    await blockHost(6);
    expect(
      await screen.findByRole("status", {
        name: "“probe” (a task of “steward 1”): Sandbox block",
      }),
    ).toBeTruthy();
    expect(commandsOf(asked, "allow_sandbox_block_for_tasks")).toEqual([]);
  });
});
