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
import type { LaidOut, LaidTab, OpenChat } from "./bindings";
import { onTheWire } from "./chatEvents.testkit";
import { heard } from "./frame.testkit";
import { chatsOf } from "./laidOut";
import { aTabForEachChat } from "./laidOut.testkit";
import { forgetKeyboard } from "./paneKeyboard";
import { forgetThisLaunch } from "./regions";
import { goToWorkspace, stripNamed } from "./test-strips";

/**
 * **Where a pick of ⌘⇧J's menu lands, and where a kept task stays** (#5224), against the whole
 * window and a core that is a fixture: the paths #5218 landed with only the compiler holding
 * them. A task on screen in another of the project's windows is jumped to there (#4768); a pick
 * goes to the pane it was asked from only while that pane is there (#5058); and a task kept in
 * its pane stays on its workspace's strip when the plane was read again before its end arrived
 * (#5106). Which window a press belongs to is the core's (`layout_windows.rs`), and stood in
 * for here by the two things it goes by: the windows' tabs, and the pane used last.
 */

vi.mock("./SessionPane", async () => {
  const { paneDrawn } = await import("./paneKeyboard");
  return {
    /** A pane that takes the keyboard when the window gives it, as a terminal does. */
    SessionPane: ({ plane, session }: { plane: string; session: number }) => {
      const pane = useRef<HTMLDivElement>(null);
      useEffect(() => paneDrawn(plane, session, () => pane.current?.focus()), [plane, session]);
      return (
        <div
          data-testid="pane"
          data-session={session}
          data-chat-keyboard=""
          tabIndex={-1}
          ref={pane}
        >
          session {session}
        </div>
      );
    },
  };
});

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const PLANE = "/home/dev/plane";

type Lineage = NonNullable<OpenChat["from"]>;
type Listed = OpenChat & { workspace: string };

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

/** A tab of window `window`: `chats` side by side, on `workspace`'s strip. */
function tabOf(window: number, front: boolean, workspace: string, ...chats: number[]): LaidTab {
  const [first, ...rest] = chats;
  return {
    workspace,
    pinned: false,
    window,
    front,
    focused: "a",
    tree: rest.reduce<LaidTab["tree"]>(
      (a, one) => ({
        kind: "split",
        direction: "row",
        share: 500,
        a,
        b: { kind: "chat", chat: one },
      }),
      { kind: "chat", chat: first },
    ),
  };
}

function core(
  open: Listed[],
  how: {
    layout?: LaidOut;
    /** The label of the tab window whose pane was used last, until a pane here is. */
    usedLast?: string;
    /** The chats on screen in a tab window, with its label. */
    elsewhere?: Record<number, string>;
  } = {},
) {
  const asked: Asked[] = [];
  let laidOut: unknown =
    how.layout ??
    aTabForEachChat(
      open.filter((one) => one.from?.tab !== false),
      (session) => open.find((one) => one.session === session)?.workspace ?? null,
    );
  let usedLast = how.usedLast;
  const listeners = new Map<string, number[]>();
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: a });
    if (cmd === "plugin:event|listen") {
      const { event, handler } = args as { event: string; handler: number };
      listeners.set(event, [...(listeners.get(event) ?? []), handler]);
      return 1;
    }
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "asks_waiting") return { plane: PLANE, asks: [] };
    if (cmd === "opened_chats") return open.map(asListed);
    if (cmd === "window_layout") {
      laidOut = a.layout;
      return null;
    }
    if (cmd === "reopened_layout") return laidOut;
    if (cmd === "dispatch_grants_needed") return [];
    if (cmd === "vault_refusals") return [];
    if (cmd === "owed_restarts") return [];
    if (cmd === "finished_tasks") return [];
    if (cmd === "stopping_chats") return [];
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
          chats: open.filter((one) => one.workspace === name).map(asListed),
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
    if (cmd === "tab_windows_back") return 0;
    if (cmd === "pane_used") {
      usedLast = undefined;
      return null;
    }
    if (cmd === "press_elsewhere") return how.elsewhere?.[a.chat as number] ?? usedLast ?? null;
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
  return {
    asked,
    /** The core says the rows changed, and the window reads its list again. */
    rowsChanged: () =>
      said("plane-changed", {
        plane: PLANE,
        changes: [{ kind: "chats", workspace: null, persona: null, path: "" }],
        answers: [{ answer: "sidebar" }],
      }),
    /** The core lists chat `session` no more. */
    unlists: (session: number) =>
      void open.splice(
        open.findIndex((one) => one.session === session),
        1,
      ),
    /** The core says chat `session`'s stop has ended it. */
    stopped: (session: number) => said("chat-stop", { plane: PLANE, session, phase: "stopped" }),
    sent: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args),
    /** The chats of each tab, as the window last said them. */
    told: () =>
      (asked.filter((one) => one.cmd === "window_layout").at(-1)?.args.layout as LaidOut).tabs.map(
        (tab) => chatsOf(tab.tree),
      ),
  };
}

const section = () => screen.findByRole("tree", { name: "Chats of this project" });

const strip = () => stripNamed("Tabs");
const tabNames = () =>
  within(strip())
    .queryAllByRole("tab")
    .map((one) => one.querySelector(".tab-name")?.textContent);
const inFront = () =>
  within(strip())
    .getAllByRole("tab")
    .find((one) => one.getAttribute("aria-selected") === "true")
    ?.querySelector(".tab-name")?.textContent;

const row = (tree: HTMLElement, name: string) => {
  const found = within(tree)
    .getAllByRole("treeitem")
    .find((one) => one.querySelector(".session")?.textContent === name);
  if (found === undefined) throw new Error(`no row is named ${name}`);
  return found;
};

const menu = () => screen.queryByRole("menu", { name: /^Tasks of / });
const line = (name: string) => {
  const found = within(menu() as HTMLElement)
    .getAllByRole("menuitem")
    .find((item) => item.querySelector(".name")?.textContent === name);
  if (found === undefined) throw new Error(`the menu has no line for ${name}`);
  return found;
};

/** The chats the panes on screen show, left to right. */
const onScreen = () =>
  screen.queryAllByTestId("pane").map((one) => Number(one.getAttribute("data-session")));
const pane = (session: number) => {
  const found = screen.queryAllByTestId("pane").find((one) => one.dataset.session === `${session}`);
  if (found === undefined) throw new Error(`no pane shows ${session}`);
  return found;
};

/** ⌘⇧J, with the chord the window's own keys take here: Ctrl+Shift, off a Mac. */
const askForTasks = () =>
  fireEvent.keyDown(document.activeElement ?? document.body, {
    key: "J",
    ctrlKey: true,
    shiftKey: true,
  });

/** The window, drawn, with every chat of `open` listed. */
async function drawn(open: Listed[], how: Parameters<typeof core>[1] = {}) {
  const held = core(open, how);
  render(<App />);
  const tree = await section();
  await waitFor(() => expect(within(tree).getAllByRole("treeitem")).toHaveLength(open.length));
  return held;
}

/** What the window was just told has been read and drawn. */
const settled = () => act(() => new Promise((done) => setTimeout(done, 0)));

/** Chat 1's menu, open from its pane, with the keyboard on the task called `first`. */
async function menuOpenOn(first: string) {
  await waitFor(() => expect(onScreen()).toContain(1));
  pane(1).focus();
  askForTasks();
  await waitFor(() => expect(document.activeElement).toBe(line(first)));
}

beforeEach(() => {
  globalThis.localStorage.clear();
  forgetThisLaunch();
  forgetKeyboard();
});
afterEach(() => {
  cleanup();
  clearMocks();
});

describe("a pick of ⌘⇧J's menu in a project with another window (#5058, #4768)", () => {
  /** steward 1 here with its tasks talk (4, in no pane) and sweep (5, in tab window 1). */
  const open = () => [
    chat(1, "alpha"),
    chat(4, "alpha", { persona: "devops", label: "talk", from: taskOf(1) }),
    chat(5, "alpha", { persona: "devops", label: "sweep", from: taskOf(1) }),
  ];
  const layout: LaidOut = {
    strip: "alpha",
    tabs: [tabOf(0, true, "alpha", 1), tabOf(1, true, "alpha", 5)],
  };
  /** The window has put back the project's tab windows, so it knows it is not the only one. */
  const toldOfWindows = async (held: { sent: (cmd: string) => unknown[] }) =>
    waitFor(() => expect(held.sent("tab_windows_back")).toHaveLength(1));

  it("jumps to a task on screen in another window, and this window's pane stays as it was", async () => {
    const held = await drawn(open(), { layout, elsewhere: { 5: "window-1" } });
    await toldOfWindows(held);
    await menuOpenOn("talk");

    await userEvent.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(line("sweep"));
    await userEvent.keyboard("{Enter}");

    await waitFor(() => expect(held.sent("press_elsewhere")).toEqual([{ plane: PLANE, chat: 5 }]));
    await waitFor(() => expect(menu()).toBeNull());
    await settled();
    // That window came forward with it, and a chat is never shown in two.
    expect(onScreen()).toEqual([1]);
    expect(held.told().flat()).not.toContain(5);
    // The keyboard is where it was, for whenever the person comes back to this window.
    expect(document.activeElement).toBe(pane(1));
  });

  it("shows a task in no pane in the pane asked from, though another window's pane was used last", async () => {
    // ⌘⇧J from a pane is that pane being used, so the press is this window's.
    const held = await drawn(open(), { layout, usedLast: "window-1" });
    await toldOfWindows(held);
    await menuOpenOn("talk");

    await userEvent.keyboard("{Enter}");

    await waitFor(() => expect(onScreen()).toEqual([4]));
    expect(held.told().flat()).toContain(4);
    await waitFor(() => expect(document.activeElement).toBe(pane(4)));
  });

  it("asks no other window about a task on screen in this one", async () => {
    const held = await drawn(
      [
        chat(1, "alpha"),
        chat(2, "alpha"),
        chat(4, "alpha", { persona: "devops", label: "talk", from: taskOf(1, { tab: true }) }),
      ],
      {
        layout: {
          strip: "alpha",
          tabs: [
            tabOf(0, true, "alpha", 1),
            tabOf(0, false, "alpha", 4),
            tabOf(1, true, "alpha", 2),
          ],
        },
        // Were it asked, the core would take it away: it is not.
        usedLast: "window-1",
      },
    );
    await toldOfWindows(held);
    await menuOpenOn("talk");

    await userEvent.keyboard("{Enter}");

    await waitFor(() => expect(onScreen()).toEqual([4]));
    expect(inFront()).toBe("talk");
    expect(held.sent("press_elsewhere")).toEqual([]);
  });
});

describe("a pick of ⌘⇧J's menu when its pane has gone (#5058)", () => {
  const talk = () => chat(4, "alpha", { persona: "devops", label: "talk", from: taskOf(1) });

  it("goes with the pane it was asked from, so a pick never lands where nothing is (#5341)", async () => {
    const held = await drawn([chat(1, "alpha"), chat(2, "alpha"), talk()], {
      layout: { strip: "alpha", tabs: [tabOf(0, true, "alpha", 1, 2)] },
    });
    await menuOpenOn("talk");

    await held.stopped(1);

    await waitFor(() => expect(onScreen()).toEqual([2]));
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  });

  it("goes to the pane asked from while it is still there, wherever the task is filed", async () => {
    // Filed in beta: a press with no pane to go to would take it to beta's strip.
    await drawn(
      [
        chat(1, "alpha"),
        chat(2, "alpha"),
        chat(4, "beta", { persona: "devops", label: "talk", from: taskOf(1) }),
      ],
      {
        layout: { strip: "alpha", tabs: [tabOf(0, true, "alpha", 1, 2)] },
      },
    );
    await menuOpenOn("talk");

    await userEvent.keyboard("{Enter}");

    // The left pane, which asked; the right one still shows steward 2.
    await waitFor(() => expect(onScreen()).toEqual([4, 2]));
  });
});

describe("a task kept in its pane when the plane was read again before its end (#5106)", () => {
  it("stays on its asker's strip, under its ended band", async () => {
    const held = await drawn([
      chat(1, "alpha"),
      chat(2, "alpha"),
      chat(4, "alpha", { persona: "devops", label: "talk", from: taskOf(1) }),
    ]);
    // A tab of its own: it names no workspace, and stands where its chat is filed.
    fireEvent.contextMenu(row(await section(), "talk"));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Move to its own tab" }));
    await waitFor(() => expect(tabNames()).toEqual(["steward 1", "steward 2", "talk"]));

    // The plane is read again without it, and only then does its end arrive.
    const read = held.sent("plane_sidebar").length;
    held.unlists(4);
    await held.rowsChanged();
    await waitFor(() => expect(held.sent("plane_sidebar").length).toBeGreaterThan(read));
    await settled();
    // Its tab has not moved meanwhile.
    expect(tabNames()).toEqual(["steward 1", "steward 2", "talk"]);
    await held.stopped(4);
    await settled();

    // It stands where it stood, beside its workspace's other tabs, as ended.
    expect(tabNames()).toEqual(["steward 1", "steward 2", "talk"]);
    expect(onScreen()).toEqual([4]);
    expect(screen.getByRole("button", { name: "Clear pane" })).toBeInTheDocument();
    // And no other strip has it.
    await goToWorkspace("beta");
    await waitFor(() => expect(tabNames()).toEqual(["Empty tab"]));
  });

  it("stays on the strip it was filed on, where its end arrived first, away from its asker's", async () => {
    const held = await drawn([
      chat(1, "alpha"),
      chat(4, "beta", { persona: "devops", label: "talk", from: taskOf(1) }),
    ]);
    fireEvent.contextMenu(row(await section(), "talk"));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Move to its own tab" }));
    await goToWorkspace("beta");
    await waitFor(() => expect(tabNames()).toContain("talk"));

    await held.stopped(4);
    held.unlists(4);
    await held.rowsChanged();
    await settled();

    expect(tabNames()).toContain("talk");
    await goToWorkspace("alpha");
    await waitFor(() => expect(tabNames()).toEqual(["steward 1"]));
  });

  it("stands on its asker's strip from that read on, away from the strip it was filed on", async () => {
    const held = await drawn([
      chat(1, "alpha"),
      chat(4, "beta", { persona: "devops", label: "talk", from: taskOf(1) }),
    ]);
    fireEvent.contextMenu(row(await section(), "talk"));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Move to its own tab" }));
    await goToWorkspace("beta");
    await waitFor(() => expect(tabNames()).toContain("talk"));

    held.unlists(4);
    await held.rowsChanged();
    await settled();
    await held.stopped(4);
    await settled();

    await goToWorkspace("alpha");
    await waitFor(() => expect(tabNames()).toEqual(["steward 1", "talk"]));
  });
});
