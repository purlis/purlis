// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { forgetLateAnswers, saidOfLate, timed } from "./answerWatch";
import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
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
import type { Moved, OpenChat } from "./bindings";
import {
  dragWithTheKeyboard,
  laidOutInAColumn,
  openTheInbox,
  queryStripNamed,
  stripNamed,
  closeFromTheRail,
} from "./test-strips";
import { DEFAULT_THEME, drawIn, inForce, onDrawn, type Theme } from "./theme/theme";
import { KEPT_DRAWN } from "./projectShown";
import { onAMac } from "./tabKeys";
import { themeInForce } from "./theme/inForce.testkit";
import { PURLIS_LIGHT } from "./theme/handed.testkit";

/**
 * A window holding more than one project (ADR 0033, spec decision 23).
 *
 * **The thing being pinned is that switching is navigation and not a teardown.** The operator
 * asked for Zed's project tabs by name and gave the reason: eight projects is eight things to
 * arrange. The reason he wanted one window PER project before that was the same one — fifty
 * chats in project A must not be torn down because he glanced at project B — so a window that
 * merged them by closing one would be the feature with its point removed.
 *
 * Everything here drives the real `App`, because the extraction it is about is `App`'s: until
 * `PlaneView` existed, a second project had nowhere to put its tabs, its sidebar or its chat
 * states, and the window could draw exactly one.
 */

/** How many times each chat's pane was started and stopped, by project and session: a pane is
 *  its terminal, and a terminal is closed once. */
const panes = vi.hoisted(() => new Map<string, { started: number; stopped: number }>());

vi.mock("./SessionPane", async () => {
  const { useEffect } = await import("react");
  return {
    SessionPane: ({ plane, session }: { plane: string; session: number }) => {
      useEffect(() => {
        const key = `${plane}#${session}`;
        const of = panes.get(key) ?? { started: 0, stopped: 0 };
        panes.set(key, of);
        of.started += 1;
        return () => {
          of.stopped += 1;
        };
      }, [plane, session]);
      return <div data-testid="pane">session {session}</div>;
    },
  };
});

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
  panes.clear();
  // A late answer a test left unanswered is the module's, not the next window's.
  forgetLateAnswers();
});

declare global {
  interface Window {
    __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
  }
}

const ONE = "/home/dev/one";
const TWO = "/home/dev/two";

/** A chat the core says a project has open. */
function chat(one: Partial<OpenChat> & { session: number; name: string }): OpenChat {
  return {
    cwd: null,
    // No harness and no persona, so its tab is its own name alone and the assertions below read
    // the names they gave it (the default before the name is charter-app#254's, tested there).
    harness: null,
    in_front: true,
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
    ...one,
  };
}

/**
 * What the core says a project's plane holds.
 *
 * **Both projects have an `alpha`**, and that is the point rather than laziness: a workspace
 * name is half an answer, and the panels used to resolve their project out of the process's
 * working directory. A fixture where only one project had the name could not tell a panel
 * asking about the right project from one asking about the only project that had it.
 */
function sidebarOf(root: string) {
  return {
    root,
    workspaces: [
      { name: "alpha", path: `${root}/workspaces/alpha`, vision: "Ship it", todos: [], chats: [] },
    ],
    personas: [],
    persona: null,
    unfiled: [],
  };
}

/**
 * A core holding whatever projects a test names, with every call kept.
 *
 * `chats` says what each project has open, keyed by its root — which is the whole point of
 * these tests: every project numbers its chats from one, so two projects each holding a chat
 * `1` is the shape a window can be wrong about.
 */
function core(
  over: {
    launch?: string | null;
    restore?: {
      planes: string[];
      active: number | null;
      dropped: string[];
      gone?: { path: string; said: string }[];
    } | null;
    chats?: Record<string, OpenChat[]>;
    /** What `project_theme_drawn` answers for each project, by its root (charter-app#273). */
    themes?: Record<string, string | null>;
    /** The chats the asks registry says wait on a reply, by project (#1690): the Inbox lists
     *  them. None said, the registry answers nothing. */
    asks?: Record<string, number[]>;
    /** What `close_plane` answers with, where a test holds its answer back (#1949). */
    closes?: () => Promise<null>;
  } = {},
) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  // Every handler, not the last one: `chat-moved` is emitted on the APP and each project
  // listens for it, so a test that fired one handler would fire whichever project happened to
  // mount last — and the whole question here is what the OTHER project does with it.
  const listeners = new Map<string, number[]>();
  const chats = over.chats ?? {};
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "plugin:event|listen") {
      const { event, handler } = given as unknown as { event: string; handler: number };
      listeners.set(event, [...(listeners.get(event) ?? []), handler]);
      return 1;
    }
    const plane = given.plane as string | undefined;
    if (cmd === "plane_at_launch")
      return over.launch === undefined || over.launch === null
        ? { plane: null, from: null, why: null }
        : { plane: over.launch, from: over.launch, why: null };
    // One remembered window, as every test here remembers it; a second window is
    // `Windows.test.tsx`'s.
    if (cmd === "planes_to_restore") {
      const back = over.restore ?? { planes: [], active: null, dropped: [] };
      return {
        windows: back.planes.length > 0 ? [{ planes: back.planes, active: back.active }] : [],
        dropped: back.dropped,
        gone: back.gone ?? [],
      };
    }
    if (cmd === "open_plane") return { plane: given.path, ask: null };
    if (cmd === "close_plane" && over.closes !== undefined) return over.closes();
    if (cmd === "plane_sidebar") return sidebarOf(plane ?? "");
    if (cmd === "opened_chats") return chats[plane ?? ""] ?? [];
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
    if (cmd === "theme_in_force") return themeInForce(args);
    if (cmd === "project_theme_drawn") return over.themes?.[plane ?? ""] ?? null;
    if (cmd === "asks_waiting" && over.asks !== undefined)
      return {
        plane,
        asks: (over.asks[plane ?? ""] ?? []).map((session) => ({
          session,
          ask: `question:${session}`,
          says: "Waiting on your reply",
          options: [],
          source: "question",
          chain: [chats[plane ?? ""]?.find((one) => one.session === session)?.name ?? "?"],
          answer: { via: "in-its-pane" },
        })),
      };
    return null;
  });
  return {
    asked,
    /** Fires `event` to every handler of it, the way the core emits one. */
    fire(event: string, payload: unknown) {
      for (const handler of listeners.get(event) ?? [])
        window.__TAURI_INTERNALS__.runCallback(handler, { event, id: 1, payload });
    },
    /** Fires one `chat-moved`, the way the core pushes one. */
    move(moved: Moved) {
      const handlers = listeners.get("chat-moved") ?? [];
      if (handlers.length === 0) throw new Error("the window is not listening for moves");
      for (const handler of handlers) {
        window.__TAURI_INTERNALS__.runCallback(handler, {
          event: "chat-moved",
          id: 1,
          payload: moved,
        });
      }
    },
    /** Hands the window a second launch's directory, the way the single-instance plugin does. */
    secondLaunch(cwd: string) {
      const handler = last(listeners, "open-plane");
      window.__TAURI_INTERNALS__.runCallback(handler, {
        event: "open-plane",
        id: 1,
        payload: cwd,
      });
    },
    /** A notification about `session` in `plane`, sent while the window was behind another app
     *  and clicked: the click brings the window back, and it lands there (#1694). */
    notified(plane: string, session: number) {
      const behind = vi.spyOn(document, "hasFocus").mockReturnValue(false);
      for (const handler of listeners.get("asks-notified") ?? [])
        window.__TAURI_INTERNALS__.runCallback(handler, {
          event: "asks-notified",
          id: 1,
          payload: { plane, session },
        });
      behind.mockRestore();
      fireEvent.focus(window);
    },
    askToQuit() {
      const handler = last(listeners, "quit-asked");
      window.__TAURI_INTERNALS__.runCallback(handler, {
        event: "quit-asked",
        id: 1,
        payload: null,
      });
    },
  };
}

/** The handler still listening for an event only the window itself listens for. React runs
 *  an effect twice under StrictMode, so the live one is the last registered. */
function last(listeners: Map<string, number[]>, event: string): number {
  const handlers = listeners.get(event) ?? [];
  if (handlers.length === 0) throw new Error(`the window is not listening for ${event}`);
  return handlers[handlers.length - 1];
}

/** The projects on the strip, and which one is in front. */
const projectTabs = () =>
  within(stripNamed("Projects"))
    .getAllByRole("tab")
    .map(
      (tab) =>
        `${tab.getAttribute("aria-label")}${
          tab.getAttribute("aria-selected") === "true" ? "*" : ""
        }`,
    );

/** How many terminals the first chat of `plane` has open: one while its pane is drawn, none
 *  once it is closed, and never fewer, which would be one closed twice. */
const open = (plane: string) => {
  const of = panes.get(`${plane}#1`);
  return (of?.started ?? 0) - (of?.stopped ?? 0);
};
/** How many times that chat's pane has been started. */
const started = (plane: string) => panes.get(`${plane}#1`)?.started ?? 0;

/** One project's tab on the strip, by the name it carries. Scoped, because a chat tab in the
 *  project in front can be called after the project it is in. */
function projectTab(name: string): HTMLElement {
  const tab = within(stripNamed("Projects"))
    .getAllByRole("tab")
    .find((one) => one.getAttribute("aria-label") === name);
  if (!tab) throw new Error(`no project tab called ${name}`);
  return tab;
}

/** The chat tabs of the project in front. */
const chatTabs = () =>
  within(stripNamed("Tabs"))
    .getAllByRole("tab")
    .map((tab) => tab.querySelector(".tab-name")?.textContent);

/** What the chat tab called `name` says its chat is doing. */
const stateOnTab = (name: string) =>
  within(stripNamed("Tabs"))
    .getAllByRole("tab")
    .find((tab) => tab.querySelector(".tab-name")?.textContent === name)
    ?.querySelector("[data-state]")
    ?.getAttribute("data-state");

/** Opens a project by typing its path into the opener. */
async function openByPath(path: string) {
  const person = userEvent.setup();
  await person.type(await screen.findByLabelText("Or type a path"), path);
  await person.click(screen.getByRole("button", { name: "Open project" }));
  return person;
}

describe("a window holding more than one project", () => {
  it("opens a second project as another tab, without letting go of the first", async () => {
    const { asked } = core({
      launch: ONE,
      chats: { [ONE]: [chat({ session: 1, name: "one.1" })] },
    });
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["one.1"]));

    await userEvent.click(screen.getByRole("button", { name: "Open a project…" }));
    await openByPath(TWO);

    await waitFor(() => expect(projectTabs()).toEqual(["one", "two*"]));
    // **Nothing was closed.** A window that merged two projects by letting go of one would be
    // this feature with its reason removed.
    expect(asked.some((one) => one.cmd === "close_plane")).toBe(false);
    expect(asked.some((one) => one.cmd === "close_session")).toBe(false);
  });

  it("keeps the first project's chats running while the second is in front", async () => {
    // The operator's own reason for wanting one window per project, which project tabs have
    // to preserve rather than trade away: fifty chats in project A are not torn down because
    // he glanced at project B.
    const { asked } = core({
      launch: ONE,
      chats: {
        [ONE]: [chat({ session: 1, name: "one.1" }), chat({ session: 2, name: "one.2" })],
        [TWO]: [chat({ session: 1, name: "two.1" })],
      },
    });
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["one.1", "one.2"]));

    await userEvent.click(screen.getByRole("button", { name: "Open a project…" }));
    await openByPath(TWO);
    await waitFor(() => expect(chatTabs()).toEqual(["two.1"]));

    expect(asked.some((one) => one.cmd === "close_session")).toBe(false);
    // And going back shows the first project's tabs again, without asking the core for them
    // a second time: they were never thrown away.
    const asksSoFar = asked.filter((one) => one.cmd === "opened_chats").length;
    await userEvent.click(projectTab("one"));
    await waitFor(() => expect(chatTabs()).toEqual(["one.1", "one.2"]));
    expect(asked.filter((one) => one.cmd === "opened_chats").length).toBe(asksSoFar);
  });

  it("ignores a chat from the queue, and its project's count goes down with it (charter-app#248)", async () => {
    const { move, asked } = core({
      launch: ONE,
      chats: { [ONE]: [chat({ session: 1, name: "one.1" }), chat({ session: 2, name: "one.2" })] },
    });
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["one.1", "one.2"]));
    const asking = {
      plane: ONE,
      state: "waiting",
      moved_at: 1,
      reports: [],
      refusals: [],
      children: [],
    };
    move({ ...asking, session: 1, needs_you: true, queue: [1, 2], sequence: 1 });
    move({ ...asking, session: 2, needs_you: true, queue: [1, 2], sequence: 2 });
    const count = () =>
      projectTab("one").querySelector('[data-part="badge"]')?.getAttribute("data-count");
    await waitFor(() => expect(count()).toBe("2"));

    // The palette's row: the hand's list that had the ✕ retired into the Inbox (#1695).
    await userEvent.keyboard("{Meta>}k{/Meta}");
    await userEvent.keyboard("Ignore one.1 until it asks again");
    await userEvent.keyboard("{Enter}");

    // The core holds the ignore, and answers it the way it answers every move.
    await waitFor(() =>
      expect(asked.filter((one) => one.cmd === "ignore_needs_you").map((one) => one.args)).toEqual([
        { plane: ONE, session: 1 },
      ]),
    );
    move({ ...asking, session: 1, needs_you: false, queue: [2], sequence: 10 });
    await waitFor(() => expect(count()).toBe("1"));

    // A report the board took before the ignore, landing after it, does not bring it back.
    // The one after it is older than the ignore too, but it is the newest word about chat 2 —
    // so chat 2's tab changing is proof that both had been taken in when the count is read.
    move({ ...asking, session: 1, needs_you: true, queue: [1, 2], sequence: 3 });
    move({ ...asking, session: 2, state: "running", needs_you: false, queue: [1], sequence: 4 });
    await waitFor(() => expect(stateOnTab("one.2")).toBe("running"));
    expect(count()).toBe("1");
    expect(screen.getByRole("button", { name: "1 chat needs you" })).toBeInTheDocument();
  });

  it("goes from the title bar to a chat in a project behind the one in front (charter-app#249, #1695)", async () => {
    const { move } = core({
      launch: ONE,
      asks: { [ONE]: [1] },
      chats: {
        [ONE]: [
          chat({ session: 1, name: "one.1", in_front: false }),
          chat({ session: 2, name: "one.2", in_front: true }),
        ],
        [TWO]: [chat({ session: 1, name: "two.1" })],
      },
    });
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["one.1", "one.2"]));
    await userEvent.click(screen.getByRole("button", { name: "Open a project…" }));
    await openByPath(TWO);
    await waitFor(() => expect(chatTabs()).toEqual(["two.1"]));
    // Project ONE's chat 1 asks while TWO is in front — and ONE's chat 2 was the tab in front
    // over there, so Go has to change the tab as well as the project.
    move({
      plane: ONE,
      session: 1,
      state: "waiting",
      needs_you: true,
      queue: [1],
      moved_at: 1,
      sequence: 1,
      reports: [],
      refusals: [],
      children: [],
    });
    const hand = await screen.findByRole("button", { name: "1 thing waits on you" });
    expect(
      within(screen.getByTestId("title-bar")).getByRole("button", { name: "1 thing waits on you" }),
    ).toBe(hand);

    // The hand opens the Inbox of the project something waits in, brought to the front.
    await userEvent.click(hand);
    await waitFor(() => expect(projectTabs()).toEqual(["one*", "two"]));
    const inbox = await screen.findByRole("tabpanel", { name: "Inbox" });
    await userEvent.click(await within(inbox).findByRole("button", { name: "Go to chat one.1" }));

    await waitFor(() => expect(projectTabs()).toEqual(["one*", "two"]));
    expect(
      within(stripNamed("Tabs")).getByRole("tab", { selected: true }).querySelector(".tab-name")
        ?.textContent,
    ).toBe("one.1");
  });

  it("names in one project's Inbox another project of the window that waits, and goes there (#1700)", async () => {
    const { move } = core({
      launch: ONE,
      asks: { [ONE]: [1] },
      chats: {
        [ONE]: [chat({ session: 1, name: "one.1" })],
        [TWO]: [chat({ session: 1, name: "two.1" })],
      },
    });
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["one.1"]));
    await userEvent.click(screen.getByRole("button", { name: "Open a project…" }));
    await openByPath(TWO);
    await waitFor(() => expect(chatTabs()).toEqual(["two.1"]));
    move({
      plane: ONE,
      session: 1,
      state: "waiting",
      needs_you: true,
      queue: [1],
      moved_at: 1,
      sequence: 1,
      reports: [],
      refusals: [],
      children: [],
    });
    await screen.findByRole("button", { name: "1 thing waits on you" });

    // TWO's own Inbox, with nothing of its own waiting: a line for ONE, which answers nothing.
    await openTheInbox();
    const inbox = await screen.findByRole("tabpanel", { name: "Inbox" });
    // This project's scope is empty and says what waits wider (qw359): one press lists it.
    expect(await within(inbox).findByText("1 waits in all projects.")).toBeInTheDocument();
    await userEvent.click(within(inbox).getByRole("button", { name: "Show all projects" }));
    const others = await within(inbox).findByRole("region", {
      name: "Other projects in this window",
    });
    expect(others).toHaveTextContent("one · 1 waiting on you");
    expect(
      within(others)
        .getAllByRole("button")
        .map((one) => one.textContent),
    ).toEqual(["Go to project"]);
    expect(within(others).queryByText(/two/)).toBeNull();

    await userEvent.click(within(others).getByRole("button", { name: "Go to project one" }));
    await waitFor(() => expect(projectTabs()).toEqual(["one*", "two"]));
    const there = await screen.findByRole("tabpanel", { name: "Inbox" });
    await within(there).findByRole("button", { name: "Go to chat one.1" });
    expect(
      within(there).queryByRole("region", { name: "Other projects in this window" }),
    ).toBeNull();
  });

  it("draws no queue in the Attention panel any more (charter-app#249)", async () => {
    const { move } = core({ launch: ONE, chats: { [ONE]: [chat({ session: 1, name: "one.1" })] } });
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["one.1"]));
    move({
      plane: ONE,
      session: 1,
      state: "waiting",
      needs_you: true,
      queue: [1],
      moved_at: 1,
      sequence: 1,
      reports: [],
      refusals: [],
      children: [],
    });
    await screen.findByRole("button", { name: "1 chat needs you" });

    // In none of the right side's views (#1678).
    expect(document.querySelectorAll(".panels").length).toBeGreaterThan(0);
    for (const view of document.querySelectorAll<HTMLElement>(".panels")) {
      expect(within(view).queryByLabelText("Needs you")).toBeNull();
      expect(within(view).queryByText(/need you/)).toBeNull();
    }
  });

  it("draws each project's own theme while it is in front, in the window and every terminal", async () => {
    // charter-app#273: the switch is live, and it reaches the terminal through `onDrawn` (#216).
    core({ launch: ONE, themes: { [ONE]: "purlis-light", [TWO]: null } });
    const terminal: Theme[] = [];
    const stop = onDrawn((theme) => terminal.push(theme));
    try {
      render(<App />);
      await waitFor(() => expect(inForce()).toBe(PURLIS_LIGHT));

      await userEvent.click(screen.getByRole("button", { name: "Open a project…" }));
      await openByPath(TWO);
      await waitFor(() => expect(projectTabs()).toEqual(["one", "two*"]));
      await waitFor(() => expect(inForce()).toBe(DEFAULT_THEME));

      await userEvent.click(projectTab("one"));
      await waitFor(() => expect(inForce()).toBe(PURLIS_LIGHT));
      expect(terminal).toEqual([PURLIS_LIGHT, DEFAULT_THEME, PURLIS_LIGHT]);
    } finally {
      stop();
      drawIn(DEFAULT_THEME);
    }
  });

  it("says on a project's tab when a chat over there needs you", async () => {
    // The reason a project behind the one on screen goes on listening rather than being torn
    // down: an operator looking at project B has no other way to learn that A is waiting.
    const { move } = core({
      launch: ONE,
      chats: {
        [ONE]: [chat({ session: 1, name: "one.1" })],
        [TWO]: [chat({ session: 1, name: "two.1" })],
      },
    });
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["one.1"]));
    await userEvent.click(screen.getByRole("button", { name: "Open a project…" }));
    await openByPath(TWO);
    await waitFor(() => expect(chatTabs()).toEqual(["two.1"]));

    // Project ONE's chat 1, while project TWO is on screen. Both projects have a chat 1, so
    // a window that ignored the plane on the event would mark the wrong tab.
    move({
      plane: ONE,
      session: 1,
      state: "waiting",
      needs_you: true,
      queue: [1],
      moved_at: 1,
      sequence: 1,
      reports: [],
      refusals: [],
      children: [],
    });

    await waitFor(() =>
      expect(
        projectTab("one").querySelector('[data-part="badge"]')?.getAttribute("data-count"),
      ).toBe("1"),
    );
    expect(projectTab("two").querySelector('[data-part="badge"]')).toBeNull();
  });

  it("asks about the workspace of the project in front, and not of the one it launched in", async () => {
    // The last two commands that resolved a plane out of `current_dir()`. A window showing a
    // project the launch had not opened drew the launch's `alpha` under this project's
    // heading — already wrong when #121 let a window open a second project, and plainly
    // wrong now that a window holds both at once.
    const { asked } = core({ launch: ONE });
    render(<App />);
    await waitFor(() => expect(asked.some((one) => one.cmd === "workspace_panels")).toBe(true));

    await userEvent.click(screen.getByRole("button", { name: "Open a project…" }));
    await openByPath(TWO);
    await waitFor(() => expect(projectTabs()).toEqual(["one", "two*"]));

    for (const cmd of ["workspace_panels", "workspace_repos"]) {
      await waitFor(() =>
        expect(asked.filter((one) => one.cmd === cmd).pop()?.args).toEqual({
          plane: TWO,
          workspace: "alpha",
        }),
      );
    }
  });

  it("closes one project without disturbing the other", async () => {
    const { asked } = core({
      launch: ONE,
      chats: {
        [ONE]: [chat({ session: 1, name: "one.1" })],
        [TWO]: [chat({ session: 1, name: "two.1" })],
      },
    });
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["one.1"]));
    await userEvent.click(screen.getByRole("button", { name: "Open a project…" }));
    await openByPath(TWO);
    await waitFor(() => expect(projectTabs()).toEqual(["one", "two*"]));

    await closeFromTheRail("two");

    // It has a chat open, so it asks first (charter-app#239's ruling), and this answers it.

    await userEvent.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", {
        name: /^Close and end/,
      }),
    );

    await waitFor(() => expect(projectTabs()).toEqual(["one*"]));
    // The core was told to let go of THAT project and no other, and the tab beside it came
    // to the front — `closeTab`'s rule, one scope up.
    expect(asked.filter((one) => one.cmd === "close_plane").map((one) => one.args)).toEqual([
      { plane: TWO },
    ]);
    expect(chatTabs()).toEqual(["one.1"]);
  });

  it("takes a closing project's tab out at once, while the core is still ending its chats (#1949)", async () => {
    // The core lets go of a project off the window's thread, which can take a while: the tab
    // goes as the close is asked, so a second press can never close it again.
    let answer: (said: null) => void = () => {};
    const { asked } = core({
      launch: ONE,
      chats: {
        [ONE]: [chat({ session: 1, name: "one.1" })],
        [TWO]: [chat({ session: 1, name: "two.1" })],
      },
      closes: () => new Promise((said) => (answer = said)),
    });
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["one.1"]));
    await userEvent.click(screen.getByRole("button", { name: "Open a project…" }));
    await openByPath(TWO);
    await waitFor(() => expect(projectTabs()).toEqual(["one", "two*"]));

    await closeFromTheRail("two");
    await userEvent.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", {
        name: /^Close and end/,
      }),
    );

    // Not answered yet, and the tab is gone. The strip says one close is still letting go, for
    // whoever waits on it (the scenario specs).
    await waitFor(() => expect(projectTabs()).toEqual(["one*"]));
    expect(stripNamed("Projects").closest("[data-letting-go]")).toHaveAttribute(
      "data-letting-go",
      "1",
    );
    expect(asked.filter((one) => one.cmd === "close_plane")).toHaveLength(1);
    answer(null);
    await waitFor(() => expect(chatTabs()).toEqual(["one.1"]));
    await waitFor(() =>
      expect(stripNamed("Projects").closest("[data-letting-go]")).toHaveAttribute(
        "data-letting-go",
        "0",
      ),
    );
    expect(asked.filter((one) => one.cmd === "close_plane")).toHaveLength(1);
  });

  it("says on the window's line when the core has not answered a command for a while (#1949)", async () => {
    core({ launch: ONE, chats: { [ONE]: [chat({ session: 1, name: "one.1" })] } });
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["one.1"]));
    // A command the window asked, never answered: timed as every command is.
    void timed("createProject", () => new Promise(() => {}), 10)("/home/dev/made");
    expect(
      await screen.findByText(saidOfLate({ command: "createProject", at: 0, ms: undefined })),
    ).toBeInTheDocument();
  });

  it("takes the late line away once the answer comes (#2029's review)", async () => {
    core({ launch: ONE, chats: { [ONE]: [chat({ session: 1, name: "one.1" })] } });
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["one.1"]));
    let answer: (said: string) => void = () => {};
    const asked = timed(
      "createProject",
      () => new Promise<string>((said) => (answer = said)),
      10,
    )("/home/dev/made");
    const late = saidOfLate({ command: "createProject", at: 0, ms: undefined });
    expect(await screen.findByText(late)).toBeInTheDocument();
    await act(async () => {
      answer("made");
      await asked;
    });
    await waitFor(() => expect(screen.queryByText(late)).toBeNull());
  });

  it("opens a project it is still letting go of only once that is done, and keeps it (#1949)", async () => {
    // Close, then open it again at once: the open waits for the close and says so, the core's
    // `plane-closed` (told before its close answers) takes nothing out, and the project is
    // back as a tab.
    let answer: (said: null) => void = () => {};
    const { asked, fire } = core({
      launch: ONE,
      chats: { [ONE]: [chat({ session: 1, name: "one.1" })] },
      closes: () => new Promise((said) => (answer = said)),
    });
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["one.1"]));
    await userEvent.click(screen.getByRole("button", { name: "Open a project…" }));
    await openByPath(TWO);
    await waitFor(() => expect(projectTabs()).toEqual(["one", "two*"]));
    await closeFromTheRail("two");
    await waitFor(() => expect(projectTabs()).toEqual(["one*"]));
    const opens = () => asked.filter((one) => one.cmd === "open_plane").length;
    const before = opens();

    await userEvent.click(screen.getByRole("button", { name: "Open a project…" }));
    await openByPath(TWO);
    expect(await screen.findByText(/finish letting go of this project/)).toBeInTheDocument();
    expect(opens()).toBe(before);

    fire("plane-closed", { plane: TWO });
    answer(null);
    await waitFor(() => expect(projectTabs()).toEqual(["one", "two*"]));
    expect(opens()).toBe(before + 1);
    expect(screen.queryByText(/finish letting go of this project/)).toBeNull();
  });

  it("warns about every project's chats when it is asked to quit, and tells two chat 1s apart", async () => {
    // Quit ends the process, and the process holds them all. A warning that counted only what
    // was on screen would understate what it is about to end by however many projects the
    // operator had merged into the window — and every project numbers its chats from one, so
    // the two `1`s have to be two rows.
    const { askToQuit } = core({
      launch: ONE,
      chats: {
        [ONE]: [chat({ session: 1, name: "one.1" })],
        [TWO]: [chat({ session: 1, name: "two.1" })],
      },
    });
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["one.1"]));
    await userEvent.click(screen.getByRole("button", { name: "Open a project…" }));
    await openByPath(TWO);
    await waitFor(() => expect(projectTabs()).toEqual(["one", "two*"]));

    askToQuit();

    const dialog = within(await screen.findByRole("alertdialog"));
    expect(dialog.getByText(/Quitting ends 2 chats/)).toBeInTheDocument();
    expect(within(dialog.getByRole("list")).getByText("one.1")).toBeInTheDocument();
    expect(within(dialog.getByRole("list")).getByText("two.1")).toBeInTheDocument();
  });

  it("tells the core what it holds and which of them is in front", async () => {
    // One call, because it is one fact: the notification gate reads the front project and the
    // cold-launch restore reads the strip, and two calls could disagree about the same window.
    const { asked } = core({ launch: ONE });
    render(<App />);
    await waitFor(() => expect(projectTabs()).toEqual(["one*"]));

    await userEvent.click(screen.getByRole("button", { name: "Open a project…" }));
    await openByPath(TWO);

    await waitFor(() =>
      expect(asked.filter((one) => one.cmd === "window_holds_planes").pop()?.args).toEqual({
        held: { planes: [ONE, TWO], active: 1 },
      }),
    );
  });

  it("puts its projects in the order a project was dragged into on the rail, and remembers it", async () => {
    // SI-6: the rail's order is the window's arrangement, which is what the machine store keeps
    // for the next cold launch — so the drag is written down by the same call. Up and down the
    // rail (#2915).
    laidOutInAColumn();
    const { asked } = core({ restore: { planes: [ONE, TWO], active: 0, dropped: [] } });
    render(<App />);
    await waitFor(() => expect(projectTabs()).toEqual(["one*", "two"]));

    within(stripNamed("Projects")).getAllByRole("tab")[1].focus();
    await dragWithTheKeyboard("{ArrowUp}");

    await waitFor(() => expect(projectTabs()).toEqual(["two", "one*"]));
    await waitFor(() =>
      expect(asked.filter((one) => one.cmd === "window_holds_planes").pop()?.args).toEqual({
        held: { planes: [TWO, ONE], active: 1 },
      }),
    );
  });

  it("says it is holding no project in front while the opener is up", async () => {
    // `+` pressed on a window full of projects. Everything it holds is still running, and
    // none of it is on screen — so a notification about any of them is sent rather than
    // suppressed.
    const { asked } = core({ launch: ONE });
    render(<App />);
    await waitFor(() => expect(projectTabs()).toEqual(["one*"]));

    await userEvent.click(screen.getByRole("button", { name: "Open a project…" }));

    await waitFor(() =>
      expect(asked.filter((one) => one.cmd === "window_holds_planes").pop()?.args).toEqual({
        held: { planes: [ONE], active: null },
      }),
    );
    // And the welcome says what it is, rather than welcoming an operator looking at one project
    // as though he had none.
    expect(await screen.findByRole("heading", { level: 1 })).toHaveTextContent(
      "Add another project",
    );
  });

  it("opens a second launch's directory as another tab rather than saying it cannot", async () => {
    // ADR 0033's last unspent half: a second launch hands its plane to the process already
    // running. It used to be told on screen that the window was busy with another project.
    const { secondLaunch } = core({ launch: ONE });
    render(<App />);
    await waitFor(() => expect(projectTabs()).toEqual(["one*"]));

    secondLaunch(TWO);

    await waitFor(() => expect(projectTabs()).toEqual(["one", "two*"]));
  });

  it("raises the project a second launch names when it is already a tab", async () => {
    // "Open it" for a project this window holds means "show me that project". The core
    // answers with the id it already has and binds nothing twice; the window brings its tab
    // to the front rather than opening a second one.
    const { secondLaunch } = core({ launch: ONE });
    render(<App />);
    await waitFor(() => expect(projectTabs()).toEqual(["one*"]));
    await userEvent.click(screen.getByRole("button", { name: "Open a project…" }));
    await openByPath(TWO);
    await waitFor(() => expect(projectTabs()).toEqual(["one", "two*"]));

    secondLaunch(ONE);

    await waitFor(() => expect(projectTabs()).toEqual(["one*", "two"]));
  });
});

describe("the cold launch putting the last quit's projects back", () => {
  it("opens every project it remembered, on the tab that was in front", async () => {
    const { asked } = core({
      launch: null,
      restore: { planes: [ONE, TWO], active: 1, dropped: [] },
    });

    render(<App />);

    await waitFor(() => expect(projectTabs()).toEqual(["one", "two*"]));
    // Through `open_plane`, which is the trust gate — never a path of its own. A restore that
    // opened these itself would be ADR 0035 turned off for every project the operator had
    // ever had open at once.
    expect(asked.filter((one) => one.cmd === "open_plane").map((one) => one.args.path)).toEqual([
      ONE,
      TWO,
    ]);
  });

  it("says which project it could not take back, and opens the rest", async () => {
    // ADR 0033: a project that has moved or gone is dropped with a line saying so, never an
    // error dialog. A restore is a convenience, and one that blocks the launch is worse than
    // the thing it was restoring.
    core({
      launch: null,
      restore: {
        planes: [TWO],
        active: 0,
        dropped: [],
        gone: [{ path: "/home/dev/gone", said: "~/dev/gone is no longer there" }],
      },
    });

    render(<App />);

    expect(await screen.findByText(/~\/dev\/gone is no longer there/)).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await waitFor(() => expect(projectTabs()).toEqual(["two*"]));
  });

  it("leaves the launch's own project in front of everything it put back", async () => {
    // Running `charter` inside a project is the operator saying which project he means, and
    // it outranks an arrangement from yesterday. The rest still come back as tabs.
    core({ launch: ONE, restore: { planes: [TWO], active: 0, dropped: [] } });

    render(<App />);

    await waitFor(() => expect(projectTabs()).toEqual(["one*", "two"]));
  });

  it("does not write the arrangement back before it has finished reading it", async () => {
    // The store the restore reads from is the store the window writes to. A window that
    // reported "I hold nothing" on its first render would wipe the very record it is about to
    // read — so nothing is said about the arrangement until the restore is done.
    const { asked } = core({
      launch: null,
      restore: { planes: [ONE, TWO], active: 0, dropped: [] },
    });

    render(<App />);

    await waitFor(() => expect(asked.some((one) => one.cmd === "window_holds_planes")).toBe(true));
    const first = asked.filter((one) => one.cmd === "window_holds_planes")[0];
    expect(first.args).toEqual({ held: { planes: [ONE, TWO], active: 0 } });
  });

  it("comes up on the opener when there is nothing to put back", async () => {
    core({ launch: null, restore: { planes: [], active: null, dropped: [] } });

    render(<App />);

    // The screen with no project on it is the welcome, whatever the machine remembers (#1812).
    expect(await screen.findByRole("heading", { level: 1 })).toHaveTextContent("Welcome to purlis");
    expect(queryStripNamed("Projects")).not.toBeInTheDocument();
  });
});

const THREE = "/home/dev/three";

/** The switcher's rows, as it lists them: each project's name, `*` on the one Enter would run. */
const switcherRows = () =>
  within(screen.getByRole("listbox", { name: "Projects" }))
    .getAllByRole("option")
    .map(
      (row) =>
        `${row.querySelector(".palette-title")?.textContent}${
          row.getAttribute("aria-selected") === "true" ? "*" : ""
        }`,
    );

/** The switcher's own key, as this platform spells it (`switcherKey.ts`). */
const switcherKey = () =>
  fireEvent.keyDown(
    window,
    onAMac() ? { key: "p", metaKey: true } : { key: "P", ctrlKey: true, shiftKey: true },
  );

describe("a project that is not in front stays drawn, off the window", () => {
  /** `count` projects back from the last quit, each with one chat, the first in front. */
  function holding(count: number) {
    const planes = Array.from({ length: count }, (_, n) => `/home/dev/p${n}`);
    const held = core({
      launch: null,
      restore: { planes, active: 0, dropped: [] },
      chats: Object.fromEntries(
        planes.map((plane, n) => [plane, [chat({ session: 1, name: `p${n}.1` })]]),
      ),
    });
    return { ...held, planes };
  }

  /** Brings a project to the front by its tab, and waits for its chat's tab. */
  async function goTo(n: number) {
    fireEvent.click(projectTab(`p${n}`));
    await waitFor(() => expect(chatTabs()).toEqual([`p${n}.1`]));
  }

  it("has one project's view in the window, whichever is in front", async () => {
    holding(3);
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["p0.1"]));
    await goTo(1);
    await goTo(0);

    // A second tab strip, or a second copy of an id, is what every query here would trip over.
    expect(document.querySelectorAll('[data-strip="Tabs"]')).toHaveLength(1);
    expect(screen.getAllByTestId("pane")).toHaveLength(1);
  });

  it("puts back what it drew, and reads what it held again only once it is on screen", async () => {
    const { asked, planes } = holding(3);
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["p0.1"]));
    const pane = screen.getByTestId("pane");
    /** What a view reads of its project's settings: at its mount, and again as they change. */
    const reads = (...commands: string[]) =>
      asked.filter((one) => one.args.plane === planes[0] && commands.includes(one.cmd)).length;
    await waitFor(() => expect(reads("sandbox_state", "project_lives_here")).toBeGreaterThan(0));
    await goTo(1);
    // Whatever the first project still owed as it left has been asked by now.
    await act(async () => {
      await new Promise((done) => setTimeout(done, 100));
    });
    const before = reads("sandbox_state", "project_lives_here");
    const builtOnce = reads("project_icons_drawn");

    act(() => {
      fireEvent.click(projectTab("p0"));
    });

    // On screen when the press returns: the very pane it had, so its terminal was never closed
    // and none was started, and nothing was asked of the core to draw it.
    expect(chatTabs()).toEqual(["p0.1"]);
    expect(screen.getByTestId("pane")).toBe(pane);
    expect(reads("sandbox_state", "project_lives_here")).toBe(before);

    // Behind that, what it showed is read again: nothing else may ever say what changed while
    // the project was away.
    await waitFor(() =>
      expect(reads("sandbox_state", "project_lives_here")).toBeGreaterThan(before),
    );
    // And what is only read as a view is built is not read again: it was not built.
    expect(reads("project_icons_drawn")).toBe(builtOnce);
  });

  it("puts a dialog away with its project, and it is there again when the project is", async () => {
    const { secondLaunch, planes } = holding(2);
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["p0.1"]));
    fireEvent.click(
      within(stripNamed("Workspaces")).getByRole("button", { name: "New workspace…" }),
    );
    expect(await screen.findByRole("dialog")).toBeInTheDocument();

    // By a way that needs no press under the dialog: a second launch naming the other project.
    act(() => secondLaunch(planes[1]));
    await waitFor(() => expect(projectTabs()).toEqual(["p0", "p1*"]));
    expect(screen.queryByRole("dialog")).toBeNull();

    act(() => secondLaunch(planes[0]));
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
  });

  it("puts away what a part of the view asked over the window, when a notification brings another project forward", async () => {
    // The review's case: a confirm opened deeper in a project's view is drawn over the whole
    // window. Kept with its project, "Delete persona steward?" asked in one project stood over
    // another, named no project, and deleted in the first. Every overlay asks whether its
    // project is shown (the families' surfaces), so it is put away with the project and is there
    // again when the person is back in the project it is about.
    const planes = ["/home/dev/p0", "/home/dev/p1"];
    const { asked, notified } = core({
      launch: null,
      restore: { planes, active: 0, dropped: [] },
      chats: Object.fromEntries(
        planes.map((plane, n) => [plane, [chat({ session: 1, name: `p${n}.1` })]]),
      ),
      asks: { [planes[1]]: [1] },
    });
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["p0.1"]));
    // The window knows something waits over there: only then does a notification land.
    await waitFor(() =>
      expect(asked.some((one) => one.cmd === "asks_waiting" && one.args.plane === planes[1])).toBe(
        true,
      ),
    );
    await act(async () => {});
    // The doctor's dialog: opened by the status line, a part of the view, and not by the
    // project's own state.
    fireEvent.click(screen.getByTestId("status-doctor"));
    expect(await screen.findByRole("dialog", { name: "Doctor" })).toBeInTheDocument();
    const built = started(planes[0]);

    // No press reaches a tab under a dialog; a clicked notification changes the project anyway.
    act(() => notified(planes[1], 1));
    await waitFor(() => expect(projectTabs()).toEqual(["p0", "p1*"]));

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.queryByRole("alertdialog")).toBeNull();
    // The other project is not held by it: nothing of a dialog is left on the document.
    expect(document.body.style.pointerEvents).not.toBe("none");

    // Back in the project it was opened in, it is there again, and the project was kept.
    fireEvent.click(projectTab("p0"));
    expect(await screen.findByRole("dialog", { name: "Doctor" })).toBeInTheDocument();
    expect(started(planes[0])).toBe(built);
    expect(open(planes[0])).toBe(1);
  });

  it("closes a chat's terminal once when its project is no longer kept, and once when it is closed", async () => {
    const { planes } = holding(KEPT_DRAWN + 1);
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["p0.1"]));
    for (let n = 1; n <= KEPT_DRAWN; n++) await goTo(n);

    // The first project is the one left longest ago: its view went, and its terminal with it.
    await waitFor(() => expect(open(planes[0])).toBe(0));
    // The rest are kept: each has its one terminal still.
    for (let n = 1; n <= KEPT_DRAWN; n++) expect(open(planes[n])).toBe(1);

    // A project kept behind the front one is closed: its terminal is closed with it, once.
    await closeFromTheRail("p2");
    fireEvent.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", {
        name: /^Close and end/,
      }),
    );
    await waitFor(() => expect(projectTabs()).not.toContain("p2"));
    await waitFor(() => expect(open(planes[2])).toBe(0));
    expect(open(planes[0])).toBe(0);

    // And everything the window still holds goes once, with the window: none is closed twice.
    cleanup();
    for (const plane of planes) expect(open(plane)).toBe(0);
  });

  // Five projects brought to the front and three closed, each a render of the whole window: about
  // 1.3s on a quiet machine, past the default five seconds on a loaded runner. Its time is watched
  // with the window's other jsdom tests since the buttons moved to React Aria (#2904).
  it("keeps the room a closed project had, for the projects the window still holds", async () => {
    // `recent` = [p0, p4, p3, p2, p1] once every one has been in front and the first again:
    // the second is one too many. With the three between them closed, it is kept.
    const { planes } = holding(KEPT_DRAWN + 1);
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["p0.1"]));
    for (let n = 1; n <= KEPT_DRAWN; n++) await goTo(n);
    await goTo(0);
    for (const n of [2, 3, 4]) {
      await closeFromTheRail(`p${n}`);
      fireEvent.click(
        within(await screen.findByRole("alertdialog")).getByRole("button", {
          name: /^Close and end/,
        }),
      );
      await waitFor(() => expect(projectTabs()).not.toContain(`p${n}`));
    }
    await goTo(1);
    const built = [started(planes[0]), started(planes[1])];

    // Back and forth between the two that are left builds neither again.
    await goTo(0);
    await goTo(1);
    await goTo(0);
    expect([started(planes[0]), started(planes[1])]).toEqual(built);
    expect([open(planes[0]), open(planes[1])]).toEqual([1, 1]);
  }, 15_000);

  it("says in the doctor's dialog what each switch took, for the person to copy", async () => {
    holding(2);
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["p0.1"]));
    await goTo(1);
    await goTo(0);

    fireEvent.click(screen.getByTestId("status-doctor"));
    const doctor = await screen.findByRole("dialog", { name: "Doctor" });
    // The window's own first project, then the two switches.
    expect(within(doctor).getByText(/^Project switches in this window \(3\)$/)).toBeInTheDocument();
    const lines = [...doctor.querySelectorAll(".doctor-unchecked li")].map(
      (line) => line.textContent ?? "",
    );
    expect(lines.at(-1)).toMatch(
      /to p0: drawn .*, painted .*; the core's synchronous commands before the paint: .*; after it: /,
    );
    expect(lines.at(-2)).toMatch(/to p1: /);
  });

  it("lists the window's own lines in the Inbox of whichever project is in front", async () => {
    // The window draws its one list where the project in front gives it a place (#1695). A
    // project that kept its view kept that place too, and the list followed the last project
    // to be built: into a view nobody could see, or back under the title bar, over the panes.
    const planes = ["/home/dev/p0", "/home/dev/p1"];
    core({
      launch: null,
      restore: {
        planes,
        active: 0,
        dropped: [],
        gone: [{ path: "/home/dev/gone", said: "~/dev/gone is no longer there" }],
      },
      chats: Object.fromEntries(
        planes.map((plane, n) => [plane, [chat({ session: 1, name: `p${n}.1` })]]),
      ),
    });
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["p0.1"]));
    /** Where the line is drawn: inside the Inbox's place for the window's lines, in the window. */
    const inTheInbox = () =>
      [...document.querySelectorAll(".inbox-window-lines")].some((place) =>
        place.textContent?.includes("~/dev/gone is no longer there"),
      );
    await waitFor(() => expect(inTheInbox()).toBe(true));

    await goTo(1);
    await waitFor(() => expect(inTheInbox()).toBe(true));
    await goTo(0);
    await waitFor(() => expect(inTheInbox()).toBe(true));
    await goTo(1);
    await waitFor(() => expect(inTheInbox()).toBe(true));
    // One place for it in the window, and the line once.
    expect(document.querySelectorAll(".inbox-window-lines")).toHaveLength(1);
    expect(screen.getAllByText(/~\/dev\/gone is no longer there/)).toHaveLength(1);
  });

  it("keeps the projects the person was in last, and draws one left longer ago again", async () => {
    holding(KEPT_DRAWN + 2);
    render(<App />);
    await waitFor(() => expect(chatTabs()).toEqual(["p0.1"]));
    const first = screen.getByTestId("pane");
    await goTo(1);
    const second = screen.getByTestId("pane");
    // On through the rest, so the first project is no longer among the last ones.
    for (let n = 2; n < KEPT_DRAWN + 2; n++) await goTo(n);

    await goTo(0);
    expect(screen.getByTestId("pane")).not.toBe(first);

    // And a window holding many projects keeps the views of a few: the second went with it.
    await goTo(1);
    expect(screen.getByTestId("pane")).not.toBe(second);
    // While the one just left is put back as it was.
    const again = screen.getByTestId("pane");
    await goTo(0);
    await goTo(1);
    expect(screen.getByTestId("pane")).toBe(again);
  });
});

describe("the project switcher (FR-27)", () => {
  /** Three projects back from the last quit, `one` in front, and the operator having been in
   *  `two` and then `three` — so the last project before this one is `two`. */
  async function beenInTwoThenThree() {
    core({ launch: null, restore: { planes: [ONE, TWO, THREE], active: 0, dropped: [] } });
    render(<App />);
    await waitFor(() => expect(projectTabs()).toEqual(["one*", "two", "three"]));
    await userEvent.click(projectTab("two"));
    await userEvent.click(projectTab("three"));
    await waitFor(() => expect(projectTabs()).toEqual(["one", "two", "three*"]));
  }

  it("asks the core for a project's theme once, however often the switcher brings it back", async () => {
    // A switch back that asked again drew the whole window a second time when the core
    // replied, after the project was already in front (FR-27's profile).
    const { asked } = core({
      launch: null,
      restore: { planes: [ONE, TWO, THREE], active: 0, dropped: [] },
    });
    render(<App />);
    await waitFor(() => expect(projectTabs()).toEqual(["one*", "two", "three"]));
    const themesOf = (plane: string) =>
      asked.filter((one) => one.cmd === "project_theme_drawn" && one.args.plane === plane).length;
    const visit = async (names: string[]) => {
      for (const name of names) {
        await userEvent.click(projectTab(name));
        await waitFor(() => expect(projectTabs()).toContain(`${name}*`));
        await waitFor(() => expect(themesOf(paths[name])).toBeGreaterThan(0));
      }
    };
    const paths: Record<string, string> = { one: ONE, two: TWO, three: THREE };
    // Each in front once, so each has been asked whoever does the asking.
    await visit(["two", "three", "one"]);
    const before = [ONE, TWO, THREE].map(themesOf);

    await visit(["two", "three", "one", "two", "one"]);

    expect([ONE, TWO, THREE].map(themesOf)).toEqual(before);
  });

  it("lists the open projects, the last one you were in first and aimed at", async () => {
    await beenInTwoThenThree();

    switcherKey();

    // The one in front is listed, and cannot run — the strip's rule — so Enter is on the one
    // before it, and the rest follow in the order the operator was last in them.
    await waitFor(() => expect(switcherRows()).toEqual(["three", "two*", "one"]));
    expect(screen.getByText("It is already in front.")).toBeInTheDocument();
  });

  it("goes back to the last project with the key and Enter, and nothing else", async () => {
    await beenInTwoThenThree();

    switcherKey();
    await waitFor(() => expect(switcherRows()).toEqual(["three", "two*", "one"]));
    await userEvent.keyboard("{Enter}");

    await waitFor(() => expect(projectTabs()).toEqual(["one", "two*", "three"]));
    expect(screen.queryByRole("listbox", { name: "Projects" })).not.toBeInTheDocument();
    // And the next switch back is to `three`, which is now the last one before this.
    switcherKey();
    await waitFor(() => expect(switcherRows()).toEqual(["two", "three*", "one"]));
  });

  it("moves down the list on each further press of its key, the way a window switcher does", async () => {
    await beenInTwoThenThree();

    switcherKey();
    await waitFor(() => expect(switcherRows()).toEqual(["three", "two*", "one"]));
    switcherKey();

    await waitFor(() => expect(switcherRows()).toEqual(["three", "two", "one*"]));
  });

  it("goes round past the project in front, which a press of its key can never aim at", async () => {
    await beenInTwoThenThree();
    switcherKey();
    await waitFor(() => expect(switcherRows()).toEqual(["three", "two*", "one"]));
    switcherKey();
    await waitFor(() => expect(switcherRows()).toEqual(["three", "two", "one*"]));

    switcherKey();

    await waitFor(() => expect(switcherRows()).toEqual(["three", "two*", "one"]));
  });

  it("keeps F2 for the palette: in the switcher it hands nothing to the chat", async () => {
    const { asked } = core({
      launch: null,
      restore: { planes: [ONE, TWO], active: 0, dropped: [] },
      chats: { [ONE]: [chat({ session: 1, name: "one.1" })] },
    });
    render(<App />);
    await waitFor(() => expect(projectTabs()).toEqual(["one*", "two"]));
    switcherKey();
    await waitFor(() => expect(switcherRows()).toEqual(["one", "two*"]));

    fireEvent.keyDown(window, { key: "F2" });

    expect(switcherRows()).toEqual(["one", "two*"]);
    expect(asked.some((one) => one.cmd === "send_input")).toBe(false);
  });

  it("narrows to what is typed, by the project's name", async () => {
    await beenInTwoThenThree();
    switcherKey();
    await waitFor(() => expect(switcherRows()).toHaveLength(3));

    await userEvent.keyboard("on");

    await waitFor(() => expect(switcherRows()).toEqual(["one*"]));
  });

  it("is a row of the palette, which turns into the switcher when it is run", async () => {
    await beenInTwoThenThree();
    await userEvent.keyboard("{Meta>}k{/Meta}");
    await userEvent.keyboard("Switch project…");
    await userEvent.keyboard("{Enter}");

    await waitFor(() => expect(switcherRows()).toEqual(["three", "two*", "one"]));
    // Nothing was switched by opening it, and Escape leaves it as the palette is left.
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(projectTabs()).toEqual(["one", "two", "three*"]));
  });

  it("finds files on its key while the window holds one project, and lists no project", async () => {
    core({ launch: ONE });
    render(<App />);
    await waitFor(() => expect(projectTabs()).toEqual(["one*"]));

    // Taken (FM-7, V86 F10): ⌘P finds files, and there are files to find with one project
    // open. Nothing to switch to, so no project is listed beside them.
    expect(switcherKey()).toBe(false);
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText(/Files in project one/)).toBeInTheDocument();
    expect(screen.queryByRole("listbox", { name: "Projects" })).not.toBeInTheDocument();
  });

  it("draws no switcher in the title bar while the window holds one project", async () => {
    core({ launch: ONE });
    render(<App />);
    await waitFor(() => expect(projectTabs()).toEqual(["one*"]));

    expect(screen.queryByRole("button", { name: "Switch project…" })).not.toBeInTheDocument();
  });
});
