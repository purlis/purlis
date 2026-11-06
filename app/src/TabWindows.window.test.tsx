// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

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
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import App, { TAB_WINDOW } from "./App";
import type { LaidOut, LaidTab, OpenChat } from "./bindings";
import { chatsOf } from "./laidOut";
import { LAYOUT_MOVED } from "./tabWindows";

/**
 * **Tab windows, as the window draws them** (#4768): a tab torn off into a bare window of its
 * project draws that project's tabs that are its own, with no sidebar and no project strip,
 * and says nothing to the other windows; the project window lets go of a tab the core moved
 * away, and remakes the tab windows its record names. Which window holds which tab is the
 * core's (`layout_windows.rs`, `chats/tests/windows.rs`) and tested there.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  clearMocks();
});

declare global {
  interface Window {
    __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
  }
}

const ONE = "/home/dev/one";

/** The chat on a harness, where the others are shells. */
const ON_A_HARNESS = 9;

function chat(session: number): OpenChat {
  return {
    session,
    name: String(session),
    cwd: null,
    harness: session === ON_A_HARNESS ? "claude" : null,
    in_front: false,
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

function tab(window: number, front: boolean, session: number): LaidTab {
  return {
    workspace: null,
    pinned: false,
    window,
    front,
    focused: "",
    tree: { kind: "chat", chat: session },
  };
}

function core(over: {
  tabWindow?: number;
  layout: LaidOut;
  chats: number[];
  /** Chats the sidebar lists as well, in no tab: a tab of theirs was closed. */
  inNoTab?: number[];
  /** What `press_elsewhere` answers: the label of the window the press landed in. */
  pressed?: string;
}) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  const listeners = new Map<string, number[]>();
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "plugin:event|listen") {
      const { event, handler } = given as unknown as { event: string; handler: number };
      listeners.set(event, [...(listeners.get(event) ?? []), handler]);
      return 1;
    }
    const plane = (given.plane as string | undefined) ?? ONE;
    if (cmd === "plane_at_launch") return { plane: ONE, from: ONE, why: null };
    if (cmd === "planes_to_restore") return { windows: [], dropped: [] };
    if (cmd === "tab_window_handed")
      return over.tabWindow === undefined ? null : { plane: ONE, number: over.tabWindow };
    if (cmd === "charter_windows") return ["main", "window-1"];
    if (cmd === "plane_sidebar")
      return {
        root: plane,
        workspaces: [],
        personas: [],
        persona: null,
        unfiled: [...over.chats, ...(over.inNoTab ?? [])].map(chat),
      };
    if (cmd === "opened_chats") return over.chats.map(chat);
    if (cmd === "reopened_layout") return over.layout;
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
    if (cmd === "tab_windows_back") return 0;
    if (cmd === "press_elsewhere") return over.pressed ?? null;
    return null;
  });
  const fire = (event: string, payload: unknown) => {
    const handlers = listeners.get(event) ?? [];
    if (handlers.length === 0) throw new Error(`the window is not listening for ${event}`);
    act(() => {
      for (const handler of handlers)
        window.__TAURI_INTERNALS__.runCallback(handler, { event, id: 1, payload });
    });
  };
  const said = () =>
    asked.filter((one) => one.cmd === "window_layout").map((one) => one.args.layout as LaidOut);
  return { asked, fire, said, sent: (cmd: string) => asked.filter((one) => one.cmd === cmd) };
}

/** Every chat each tab a layout names, with its window. */
const shape = (laid: LaidOut | undefined) =>
  laid?.tabs.map((one) => [one.window, chatsOf(one.tree)]) ?? [];

describe("a tab window (#4768)", () => {
  it("draws only its own tabs, with no sidebar and no project strip, and says only those", async () => {
    mockWindows("window-1");
    const { said, sent } = core({
      tabWindow: 1,
      layout: { strip: null, tabs: [tab(0, true, 1), tab(1, true, 2)] },
      chats: [1, 2],
    });
    render(<App />);

    await screen.findByText("session 2");
    expect(screen.queryByText("session 1")).toBeNull();
    expect(document.querySelector("main.window")?.getAttribute(TAB_WINDOW)).toBe("1");
    expect(screen.queryByRole("tablist", { name: "Projects" })).toBeNull();
    await waitFor(() => expect(shape(said().at(-1))).toEqual([[1, [2]]]));
    // It holds no project of its own, and remakes no window.
    expect(sent("window_holds_planes")).toEqual([]);
    expect(sent("tab_windows_back")).toEqual([]);
  });
});

describe("a tab window's panes (#5205, story 27)", () => {
  it("draws the same head and foot as the project window: moving a tab changes nothing about its panes", async () => {
    mockWindows("window-1");
    core({
      tabWindow: 1,
      layout: { strip: null, tabs: [tab(1, true, ON_A_HARNESS)] },
      chats: [ON_A_HARNESS],
    });
    render(<App />);

    const frame = (await screen.findByText(`session ${ON_A_HARNESS}`)).closest(
      ".pane-frame",
    ) as HTMLElement;
    expect(frame.querySelector(".pane-head")).not.toBeNull();
    await waitFor(() => expect(frame.lastElementChild).toHaveAttribute("data-scope", "pane-foot"));
    // The tab's only pane: its two splits and no Close, as anywhere (#5318).
    expect(within(frame).getByRole("button", { name: "Split right" })).toBeInTheDocument();
    expect(within(frame).queryByRole("button", { name: "Close this pane" })).toBeNull();
  });
});

describe("the project window of a project with a tab window (#4768)", () => {
  it("remakes the tab windows its record names, and gives no tab to a chat one of them shows", async () => {
    const { said, sent } = core({
      layout: { strip: null, tabs: [tab(0, true, 1), tab(1, true, 2)] },
      chats: [1, 2],
    });
    render(<App />);

    await screen.findByText("session 1");
    await waitFor(() => expect(sent("tab_windows_back")).toHaveLength(1));
    await waitFor(() => expect(shape(said().at(-1))).toEqual([[0, [1]]]));
  });

  it("lets go of a tab the core moved to a tab window, and ends nothing", async () => {
    const { fire, said, sent } = core({
      layout: { strip: null, tabs: [tab(0, true, 1), tab(0, false, 2)] },
      chats: [1, 2],
    });
    render(<App />);
    await screen.findByText("session 1");
    await waitFor(() =>
      expect(shape(said().at(-1))).toEqual([
        [0, [1]],
        [0, [2]],
      ]),
    );

    fire(LAYOUT_MOVED, {
      plane: ONE,
      layout: { strip: null, tabs: [tab(0, true, 1), tab(1, true, 2)] },
    });

    await waitFor(() => expect(shape(said().at(-1))).toEqual([[0, [1]]]));
    expect(sent("close_session")).toEqual([]);
  });
});

/** The sidebar's row of chat `session`. */
const row = (session: number) => {
  const found = within(screen.getByRole("tree", { name: "Chats of this project" }))
    .getAllByRole("treeitem")
    .find((one) => one.querySelector(".session")?.textContent === String(session));
  if (found === undefined) throw new Error(`no row is chat ${session}`);
  return found;
};

describe("the most recently used pane across the project's windows (#5108)", () => {
  it("a pane used in the project window of a project with a tab window is said to the core", async () => {
    const { sent } = core({
      layout: { strip: null, tabs: [tab(0, true, 1), tab(1, true, 2)] },
      chats: [1, 2],
    });
    render(<App />);
    await waitFor(() => expect(sent("tab_windows_back")).toHaveLength(1));

    fireEvent.pointerDown(await screen.findByText("session 1"));

    await waitFor(() => expect(sent("pane_used").at(-1)?.args).toEqual({ plane: ONE }));
  });

  it("a key typed in a pane of a tab window is said to the core", async () => {
    mockWindows("window-1");
    const { sent } = core({
      tabWindow: 1,
      layout: { strip: null, tabs: [tab(0, true, 1), tab(1, true, 2)] },
      chats: [1, 2],
    });
    render(<App />);

    fireEvent.keyDown(await screen.findByText("session 2"), { key: "a" });

    await waitFor(() => expect(sent("pane_used").at(-1)?.args).toEqual({ plane: ONE }));
  });

  it("a pane given focus back as its window comes forward is not a use of it", async () => {
    // The window coming forward hands focus back to the pane that last had it: a press on a
    // sidebar row would otherwise always be this window's.
    const { sent } = core({
      layout: { strip: null, tabs: [tab(0, true, 1), tab(1, true, 2)] },
      chats: [1, 2],
    });
    render(<App />);
    await waitFor(() => expect(sent("tab_windows_back")).toHaveLength(1));

    fireEvent.focus(await screen.findByText("session 1"));
    await act(() => new Promise((done) => setTimeout(done, 0)));

    expect(sent("pane_used")).toEqual([]);
  });

  it("a window says its pane's use once until it has been away", async () => {
    const { sent } = core({
      layout: { strip: null, tabs: [tab(0, true, 1), tab(1, true, 2)] },
      chats: [1, 2],
    });
    render(<App />);
    await waitFor(() => expect(sent("tab_windows_back")).toHaveLength(1));
    const pane = await screen.findByText("session 1");

    fireEvent.pointerDown(pane);
    fireEvent.keyDown(pane, { key: "a" });
    fireEvent.keyDown(pane, { key: "b" });
    await waitFor(() => expect(sent("pane_used")).toHaveLength(1));
    // Another window was used meanwhile, so this one says it again.
    act(() => void window.dispatchEvent(new Event("blur")));
    fireEvent.keyDown(pane, { key: "c" });

    await waitFor(() => expect(sent("pane_used")).toHaveLength(2));
  });

  it("a project of one window says nothing of its panes, and a press asks nothing", async () => {
    const { said, sent } = core({
      layout: { strip: null, tabs: [tab(0, true, 1)] },
      chats: [1],
      inNoTab: [2],
    });
    render(<App />);
    fireEvent.pointerDown(await screen.findByText("session 1"));

    await userEvent.click(row(2));

    await screen.findByText("session 2");
    await waitFor(() => expect(shape(said().at(-1))).toEqual([[0, [2]]]));
    expect(sent("pane_used")).toEqual([]);
    expect(sent("press_elsewhere")).toEqual([]);
  });

  it("a press on a chat in no pane that the core lands in a tab window's pane changes nothing here", async () => {
    const { said, sent } = core({
      layout: { strip: null, tabs: [tab(0, true, 1), tab(1, true, 2)] },
      chats: [1, 2],
      inNoTab: [3],
      pressed: "window-1",
    });
    render(<App />);
    await screen.findByText("session 1");
    await waitFor(() => expect(shape(said().at(-1))).toEqual([[0, [1]]]));
    const saidBefore = said().length;

    await userEvent.click(row(3));

    await waitFor(() =>
      expect(sent("press_elsewhere").at(-1)?.args).toEqual({ plane: ONE, chat: 3 }),
    );
    // The core's answer is read once it arrives: let it.
    await act(() => new Promise((done) => setTimeout(done, 0)));
    expect(screen.queryByText("session 3")).toBeNull();
    expect(screen.getByText("session 1")).toBeInTheDocument();
    expect(
      said()
        .slice(saidBefore)
        .flatMap((one) => one.tabs.map((t) => chatsOf(t.tree))),
    ).not.toContainEqual([3]);
  });

  it("a press on a chat in no pane that the core leaves to this window lands in its focused pane", async () => {
    const { said, sent } = core({
      layout: { strip: null, tabs: [tab(0, true, 1), tab(1, true, 2)] },
      chats: [1, 2],
      inNoTab: [3],
    });
    render(<App />);
    await screen.findByText("session 1");

    await userEvent.click(row(3));

    await screen.findByText("session 3");
    expect(sent("press_elsewhere").at(-1)?.args).toEqual({ plane: ONE, chat: 3 });
    await waitFor(() => expect(shape(said().at(-1))).toEqual([[0, [3]]]));
  });
});

/**
 * **A chat shown only in a tab window says its notes there alone** (#5225, from #4759). The window
 * showing the chat owns what it has to say, on its pane's band, where the person is reading it.
 * The project window's Notices centre keeps only the chats no window shows.
 */
describe("a note of a chat shown only in a tab window (#5225)", () => {
  const SAID = "Its browsers ended with it.";
  const LAID: LaidOut = { strip: null, tabs: [tab(0, true, 1), tab(1, true, 2)] };
  /** The tab window closed: its chat in front in the project window. */
  const BACK: LaidOut = { strip: null, tabs: [tab(0, false, 1), tab(0, true, 2)] };
  const says = (session: number) => ({ plane: ONE, session, note: SAID });
  /** How many times this window says it: on a pane's band or in the Notices centre. */
  const saidHere = () => screen.queryAllByText(SAID).length;
  const windowOf = async (tabWindow: number | undefined, session: number) => {
    mockWindows(tabWindow === undefined ? "main" : "window-1");
    const wire = core({ tabWindow, layout: LAID, chats: [1, 2], inNoTab: [3] });
    render(<App />);
    await screen.findByText(`session ${session}`);
    return wire;
  };

  it("is said once across the two windows, by the tab window that shows it", async () => {
    const project = await windowOf(undefined, 1);
    project.fire("chat-browsers-ended", says(2));
    // Heard, as the same project's chat with no pane anywhere is: its note is listed.
    project.fire("chat-browsers-ended", says(3));
    await waitFor(() => expect(saidHere()).toBe(1));
    const here = screen.getByText(SAID);
    expect(here.closest("[data-cause]")?.textContent).toContain("3");
    cleanup();
    clearMocks();

    const shows = await windowOf(1, 2);
    shows.fire("chat-browsers-ended", says(2));
    await waitFor(() => expect(saidHere()).toBe(1));
    expect(screen.getByText(SAID).closest('[data-placement="pane"]')).not.toBeNull();
  });

  it("lists no chat of the project in a tab window's own window, which has no Notices centre", async () => {
    const shows = await windowOf(1, 2);
    shows.fire("chat-browsers-ended", says(3));
    await act(async () => {
      await Promise.resolve();
    });
    expect(saidHere()).toBe(0);
  });

  it("goes from the Notices centre as a tab window comes to show its chat, and comes back as it stops", async () => {
    const project = await windowOf(undefined, 1);
    project.fire("chat-browsers-ended", says(3));
    await waitFor(() => expect(saidHere()).toBe(1));

    // The tab window's own report: a press there landed chat 3 in its pane.
    project.fire("layout-reported", {
      plane: ONE,
      layout: { strip: null, tabs: [tab(0, true, 1), tab(1, true, 3)] },
    });
    await waitFor(() => expect(saidHere()).toBe(0));

    project.fire("layout-reported", { plane: ONE, layout: LAID });
    await waitFor(() => expect(saidHere()).toBe(1));
  });

  it("cleared where it stands, is cleared in every window of the project", async () => {
    const user = userEvent.setup();
    const shows = await windowOf(1, 2);
    shows.fire("chat-browsers-ended", says(2));
    const said = await screen.findByText(SAID);
    const band = said.closest('[data-placement="pane"]') as HTMLElement;
    await user.click(within(band).getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(saidHere()).toBe(0));
    // Told to the other windows.
    const told = shows
      .sent("plugin:event|emit")
      .map((one) => one.args as { event: string; payload: unknown })
      .filter((one) => one.event === "note-cleared");
    expect(told).toEqual([
      {
        event: "note-cleared",
        payload: { plane: ONE, session: 2, note: { kind: "start" }, from: "window-1" },
      },
    ]);
    cleanup();
    clearMocks();

    // The project window held it too, off screen, and hears that: it does not say it again when
    // the chat comes back to it.
    const project = await windowOf(undefined, 1);
    project.fire("chat-browsers-ended", says(2));
    project.fire("note-cleared", told[0].payload);
    project.fire(LAYOUT_MOVED, { plane: ONE, layout: BACK });
    await screen.findByText("session 2");
    expect(saidHere()).toBe(0);
  });

  it("nobody having cleared it, says it again on the chat's pane as the chat comes back", async () => {
    const project = await windowOf(undefined, 1);
    project.fire("chat-browsers-ended", says(2));
    expect(saidHere()).toBe(0);
    project.fire(LAYOUT_MOVED, { plane: ONE, layout: BACK });
    await screen.findByText("session 2");
    await waitFor(() => expect(saidHere()).toBe(1));
  });

  /** What this window told the other windows, of one event. */
  const told = (wire: ReturnType<typeof core>, event: string) =>
    wire
      .sent("plugin:event|emit")
      .map((one) => one.args as { event: string; payload: unknown })
      .filter((one) => one.event === event)
      .map((one) => one.payload);

  it("is handed to the window that comes to show its chat, what this one held before", async () => {
    const project = await windowOf(undefined, 1);
    project.fire("chat-browsers-ended", says(3));
    await waitFor(() => expect(saidHere()).toBe(1));

    project.fire("layout-reported", {
      plane: ONE,
      layout: { strip: null, tabs: [tab(0, true, 1), tab(1, true, 3)] },
    });
    await waitFor(() => expect(saidHere()).toBe(0));
    await waitFor(() =>
      expect(told(project, "notes-handed")).toEqual([
        {
          plane: ONE,
          session: 3,
          from: "main",
          start: { notes: [SAID], agentsMd: [] },
          blocks: [],
        },
      ]),
    );
  });

  it("is said on the band of the window it was handed to, once, however often it is handed", async () => {
    const shows = await windowOf(1, 2);
    const handed = {
      plane: ONE,
      session: 2,
      from: "main",
      start: { notes: [SAID], agentsMd: [] },
      blocks: [],
    };
    shows.fire("notes-handed", handed);
    shows.fire("notes-handed", handed);
    await waitFor(() => expect(saidHere()).toBe(1));
    expect(screen.getByText(SAID).closest('[data-placement="pane"]')).not.toBeNull();
    expect(screen.getByText(SAID).textContent).toBe(SAID);
  });

  it("is asked for by a tab window as it first draws, and handed by the window holding it", async () => {
    const shows = await windowOf(1, 2);
    await waitFor(() =>
      expect(told(shows, "notes-asked")).toEqual([{ plane: ONE, from: "window-1" }]),
    );
    cleanup();
    clearMocks();

    const project = await windowOf(undefined, 1);
    project.fire("chat-browsers-ended", says(2));
    await waitFor(() => expect(told(project, "notes-handed")).toHaveLength(1));
    project.fire("notes-asked", { plane: ONE, from: "window-1" });
    await waitFor(() => expect(told(project, "notes-handed").length).toBeGreaterThan(1));
  });

  it("a block cleared in another window is let go here, and the core is told once, by that window", async () => {
    const block = {
      plane: ONE,
      session: 2,
      operation: "write",
      kind: "path",
      ours: false,
      harness: null,
      said: "a write",
      offer: "host",
      target: "api.example.com",
      levels: ["chat"],
      route: null,
      held: false,
      ruled: null,
    };
    const project = await windowOf(undefined, 1);
    project.fire("chat-sandbox-blocked", block);
    project.fire("note-cleared", {
      plane: ONE,
      session: 2,
      note: { kind: "block", block },
      from: "window-1",
    });
    project.fire(LAYOUT_MOVED, { plane: ONE, layout: BACK });
    await screen.findByText("session 2");
    expect(screen.queryByText(/The sandbox blocked a write/)).toBeNull();
    expect(project.sent("forget_sandbox_block")).toEqual([]);
  });

  it("a note cleared in another project's window stays here", async () => {
    const project = await windowOf(undefined, 1);
    project.fire("chat-browsers-ended", says(3));
    await waitFor(() => expect(saidHere()).toBe(1));
    project.fire("note-cleared", {
      plane: "/home/dev/two",
      session: 3,
      note: { kind: "start" },
      from: "window-1",
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(saidHere()).toBe(1);
  });
});
