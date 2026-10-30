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
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import App from "./App";
import type { OpenChat } from "./bindings";
import type { WindowSaid } from "./windows";
import { stripNamed } from "./test-strips";
import { findResult } from "./actionResult.testkit";
import {
  keepUnsaved,
  takesHandedWork,
  unsavedNow,
  type HandedWork,
  type UnsavedWork,
} from "./unsavedWork";
import { forgetHandOver } from "./unsavedWindows";

/**
 * A project tab split into an OS window of its own, and moved back (charter#126; ADR 0033,
 * amended 2026-09-26).
 *
 * What is pinned here is the window's half: which rows it offers, what it tells the core, what
 * it draws when it is a split window, and how the needs-you list and the quit warning reach
 * across windows. Which window holds which project, and what a closed window hands back, is the
 * core's (`planes.rs`, `windows.rs`) and tested there.
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
const TWO = "/home/dev/two";
const THREE = "/home/dev/three";

function chat(session: number, name: string): OpenChat {
  return {
    session,
    name,
    cwd: null,
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
  };
}

function core(
  over: {
    launch?: string | null;
    restore?: { windows: { planes: string[]; active: number | null }[]; dropped: string[] };
    handed?: { planes: string[]; active: number | null } | null;
    chats?: Record<string, OpenChat[]>;
    /** The chats a project's sidebar lists outside every workspace. */
    unfiled?: Record<string, OpenChat[]>;
    windows?: string[];
    holder?: Record<string, string>;
    refuseMove?: string;
  } = {},
) {
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
    const plane = (given.plane as string | undefined) ?? "";
    if (cmd === "plane_at_launch")
      return over.launch
        ? { plane: over.launch, from: over.launch, why: null }
        : { plane: null, from: null, why: null };
    if (cmd === "planes_to_restore") return over.restore ?? { windows: [], dropped: [] };
    if (cmd === "projects_handed") return over.handed ?? null;
    if (cmd === "charter_windows") return over.windows ?? ["main"];
    if (cmd === "show_window_holding") return over.holder?.[plane] ?? null;
    if (cmd === "move_projects") {
      if (over.refuseMove !== undefined) throw over.refuseMove;
      return (given.to as string | null) ?? "window-1";
    }
    if (cmd === "open_plane") return { plane: given.path, ask: null };
    if (cmd === "plane_sidebar")
      return {
        root: plane,
        workspaces: [],
        personas: [],
        persona: null,
        unfiled: over.unfiled?.[plane] ?? [],
      };
    if (cmd === "opened_chats") return over.chats?.[plane] ?? [];
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
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
  return {
    asked,
    sent: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args),
    fire,
    listening: (event: string) => (listeners.get(event) ?? []).length > 0,
  };
}

const projectTabs = () =>
  within(stripNamed("Projects"))
    .getAllByRole("tab")
    .map(
      (tab) =>
        `${tab.getAttribute("aria-label")}${
          tab.getAttribute("aria-selected") === "true" ? "*" : ""
        }`,
    );

function projectTab(name: string): HTMLElement {
  const tab = within(stripNamed("Projects"))
    .getAllByRole("tab")
    .find((one) => one.getAttribute("aria-label") === name);
  if (!tab) throw new Error(`no project tab called ${name}`);
  return tab;
}

/** Right-clicks a project tab and presses a row on its menu. */
async function fromTheMenu(project: string, row: string) {
  fireEvent.contextMenu(projectTab(project));
  const menu = await screen.findByRole("menu");
  await userEvent.click(within(menu).getByRole("menuitem", { name: row }));
}

/** What another window tells this one. */
function said(label: string, over: Partial<WindowSaid> = {}): WindowSaid {
  return { label, needing: [], quiet: [], ending: [], settled: true, ...over };
}

describe("a project moved into a window of its own", () => {
  it("leaves the window it was in without ending anything", async () => {
    const { sent } = core({
      restore: { windows: [{ planes: [ONE, TWO], active: 0 }], dropped: [] },
    });
    render(<App />);
    await vi.waitFor(() => expect(projectTabs()).toEqual(["one*", "two"]));

    await fromTheMenu("two", "Move to a new window");

    await vi.waitFor(() => expect(projectTabs()).toEqual(["one*"]));
    expect(sent("move_projects")).toEqual([{ projects: [TWO], front: TWO, to: null }]);
    // **Nothing was closed**: its chats go on running, and the new window draws them.
    expect(sent("close_plane")).toEqual([]);
    expect(sent("close_session")).toEqual([]);
    // And the core is told what this window holds now, so its events stop coming here.
    await vi.waitFor(() =>
      expect(sent("window_holds_planes").pop()).toEqual({ held: { planes: [ONE], active: 0 } }),
    );
  });

  it("keeps its tab when the core refuses, and says why", async () => {
    core({
      restore: { windows: [{ planes: [ONE, TWO], active: 0 }], dropped: [] },
      refuseMove: "purlis could not make a window: no display",
    });
    render(<App />);
    await vi.waitFor(() => expect(projectTabs()).toEqual(["one*", "two"]));

    await fromTheMenu("two", "Move to a new window");

    expect(await findResult("purlis could not make a window: no display")).toBeVisible();
    expect(projectTabs()).toEqual(["one*", "two"]);
  });

  it("is drawn by a split window, which restores nothing and asks the launch nothing", async () => {
    mockWindows("window-1");
    const { sent } = core({
      handed: { planes: [TWO], active: 0 },
      chats: { [TWO]: [chat(1, "two.1")] },
    });
    render(<App />);

    await vi.waitFor(() => expect(projectTabs()).toEqual(["two*"]));
    expect(await screen.findByRole("tab", { name: /two\.1/ })).toBeInTheDocument();
    // A split window is not a launch: the working directory, the relaunch question and the
    // remembered window set are all the main window's.
    expect(sent("plane_at_launch")).toEqual([]);
    expect(sent("relaunch_ask")).toEqual([]);
    expect(sent("relaunch")).toEqual([]);
    expect(sent("planes_to_restore")).toEqual([]);
  });

  it("listens only for what is sent to every window or to it", async () => {
    // Tauri delivers every event to a listener whose target is `Any`, including one the core
    // sent to another window. A split window listening that way would answer the main window's
    // quit and take in projects moved to the main window.
    mockWindows("window-1");
    const { asked } = core({
      handed: { planes: [TWO], active: 0 },
      chats: { [TWO]: [chat(1, "two.1")] },
    });
    render(<App />);
    await vi.waitFor(() => expect(projectTabs()).toEqual(["two*"]));
    await screen.findByRole("tab", { name: /two\.1/ });

    const listening = asked.filter((one) => one.cmd === "plugin:event|listen");
    const events = new Set(listening.map((one) => one.args.event));
    for (const event of ["quit-asked", "projects-arrived", "open-plane", "chat-moved", "run-offer"])
      expect(events).toContain(event);
    for (const one of listening)
      expect({ event: one.args.event, target: one.args.target }).toEqual({
        event: one.args.event,
        target: { kind: "AnyLabel", label: "window-1" },
      });
  });

  it("moves back to the main window from the split window, which then holds nothing", async () => {
    mockWindows("window-1");
    const { sent } = core({ handed: { planes: [TWO], active: 0 } });
    render(<App />);
    await vi.waitFor(() => expect(projectTabs()).toEqual(["two*"]));

    await fromTheMenu("two", "Move to the main window");

    expect(sent("move_projects")).toEqual([{ projects: [TWO], front: TWO, to: "main" }]);
    // Holding nothing, a split window says so, and the core closes it (`windows.rs`).
    await vi.waitFor(() =>
      expect(sent("window_holds_planes").pop()).toEqual({ held: { planes: [], active: null } }),
    );
    expect(sent("close_plane")).toEqual([]);
  });

  it("does not offer the main window's projects a way back to where they already are", async () => {
    core({ restore: { windows: [{ planes: [ONE, TWO], active: 0 }], dropped: [] } });
    render(<App />);
    await vi.waitFor(() => expect(projectTabs()).toEqual(["one*", "two"]));

    fireEvent.contextMenu(projectTab("two"));
    const menu = await screen.findByRole("menu");

    expect(within(menu).queryByRole("menuitem", { name: /to the main window/ })).toBeNull();
  });

  it("arrives in the window it was moved to, in front", async () => {
    const { fire } = core({ launch: ONE });
    render(<App />);
    await vi.waitFor(() => expect(projectTabs()).toEqual(["one*"]));

    fire("projects-arrived", { planes: [TWO], front: TWO });

    await vi.waitFor(() => expect(projectTabs()).toEqual(["one", "two*"]));
  });

  it("comes back behind what is showing when a split window is closed", async () => {
    const { fire } = core({ launch: ONE });
    render(<App />);
    await vi.waitFor(() => expect(projectTabs()).toEqual(["one*"]));

    fire("projects-arrived", { planes: [TWO, THREE], front: null });

    await vi.waitFor(() => expect(projectTabs()).toEqual(["one*", "two", "three"]));
  });

  it("is not taken into a second window when it is opened again there", async () => {
    // One project is one tab in one window: opening it from another window's opener brings the
    // window holding it to the front instead.
    const { sent } = core({ launch: ONE, holder: { [TWO]: "window-1" } });
    render(<App />);
    await vi.waitFor(() => expect(projectTabs()).toEqual(["one*"]));

    await userEvent.click(screen.getByRole("button", { name: "Open a project…" }));
    const person = userEvent.setup();
    await person.type(await screen.findByLabelText("Or type a path"), TWO);
    await person.click(screen.getByRole("button", { name: "Open project" }));

    await vi.waitFor(() => expect(sent("show_window_holding")).toContainEqual({ plane: TWO }));
    expect(projectTabs()).toEqual(["one"]);
  });
});

describe("a cold launch that remembers two windows", () => {
  it("puts the second window's projects back in a window of their own", async () => {
    const { sent } = core({
      restore: {
        windows: [
          { planes: [ONE], active: 0 },
          { planes: [TWO, THREE], active: 1 },
        ],
        dropped: [],
      },
    });
    render(<App />);

    await vi.waitFor(() =>
      expect(sent("move_projects")).toEqual([{ projects: [TWO, THREE], front: THREE, to: null }]),
    );
    // Each one still went through the gate, here, before it was moved.
    expect(sent("open_plane").map((one) => one.path)).toEqual([ONE, TWO, THREE]);
    expect(projectTabs()).toEqual(["one*"]);
  });
});

describe("needs you, across windows", () => {
  const go = (session: number) => ({
    id: `needs.show:${session}`,
    title: "Show the chat",
    available: true,
    reason: "",
    does: { verb: "showChat" as const, session },
  });

  it("counts a chat asking in another window, and goes there to show it", async () => {
    const { fire, asked } = core({
      launch: ONE,
      windows: ["main", "window-1"],
      holder: { [TWO]: "window-1" },
    });
    render(<App />);
    await vi.waitFor(() => expect(projectTabs()).toEqual(["one*"]));

    fire(
      "window-said",
      said("window-1", {
        needing: [
          { session: 3, name: "two.3", workspace: "alpha", go: go(3), plane: TWO, project: "two" },
        ],
      }),
    );

    // The hand opens the Inbox in front, which lists the chat in the other window as a group
    // of its own (#1695: the hand's list retired, and nothing it named is lost), and its Go to
    // chat goes there.
    const hand = await screen.findByRole("button", { name: "1 chat needs you" });
    await userEvent.click(hand);
    const inbox = await screen.findByRole("tabpanel", { name: "Inbox" });
    // This project's scope is empty and says what waits wider (qw359): one press lists it.
    await userEvent.click(await within(inbox).findByRole("button", { name: "Show all projects" }));
    const group = await within(inbox).findByRole("region", { name: "two.3 · alpha · two" });
    expect(within(group).getByText("In another window")).toBeInTheDocument();
    await userEvent.click(within(group).getByRole("button", { name: /^Go to chat two\.3/ }));

    await vi.waitFor(() =>
      expect(asked.find((one) => one.cmd === "plugin:event|emit_to")?.args).toMatchObject({
        target: { kind: "AnyLabel", label: "window-1" },
        event: "run-offer",
        payload: { plane: TWO, offer: go(3) },
      }),
    );
    expect(asked.some((one) => one.cmd === "show_window_holding" && one.args.plane === TWO)).toBe(
      true,
    );
  });

  it("lists every chat that cannot say it waits, here and in another window, in the Inbox the faint hand opens", async () => {
    const { fire, asked } = core({
      launch: ONE,
      windows: ["main", "window-1"],
      holder: { [TWO]: "window-1" },
      unfiled: { [ONE]: [{ ...chat(2, "shell 2"), unreported: "a shell cannot say it waits" }] },
    });
    render(<App />);
    await vi.waitFor(() => expect(projectTabs()).toEqual(["one*"]));
    fire(
      "window-said",
      said("window-1", { quiet: [{ name: "codex 4", project: "two", plane: TWO, session: 4 }] }),
    );

    // The faint hand names them; a press opens the Inbox, which lists each one (#1695: the
    // hand's list that named them is retired, and nothing it named is lost).
    const hand = await screen.findByRole("button", {
      name: "Nothing has asked for you, but 2 chats can't tell purlis they're waiting",
    });
    await userEvent.click(hand);
    const inbox = await screen.findByRole("tabpanel", { name: "Inbox" });
    const here = await vi.waitFor(() => {
      const found = inbox.querySelector<HTMLElement>(`[data-cause="chat-quiet:${ONE}#2"]`);
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(here).toHaveTextContent("shell 2 can't tell purlis it's waiting.");
    const there = inbox.querySelector<HTMLElement>(`[data-cause="chat-quiet:${TWO}#4"]`);
    expect(there).toHaveTextContent("codex 4 in two can't tell purlis it's waiting.");

    await userEvent.click(within(there as HTMLElement).getByRole("button", { name: "Go to chat" }));
    await vi.waitFor(() =>
      expect(asked.find((one) => one.cmd === "plugin:event|emit_to")?.args).toMatchObject({
        target: { kind: "AnyLabel", label: "window-1" },
        event: "run-offer",
        payload: { plane: TWO, offer: { does: { verb: "showChat", session: 4 } } },
      }),
    );
  });

  it("drops what a window said once that window has gone", async () => {
    const { fire } = core({ launch: ONE, windows: ["main", "window-1"] });
    render(<App />);
    await vi.waitFor(() => expect(projectTabs()).toEqual(["one*"]));
    fire(
      "window-said",
      said("window-1", {
        needing: [
          { session: 3, name: "two.3", workspace: "alpha", go: go(3), plane: TWO, project: "two" },
        ],
      }),
    );
    await screen.findByRole("button", { name: "1 chat needs you" });

    fire("windows-changed", ["main"]);

    await vi.waitFor(() =>
      expect(screen.queryByRole("button", { name: "1 chat needs you" })).toBeNull(),
    );
  });
});

describe("quitting with two windows", () => {
  const ending = {
    key: `${TWO}#1`,
    project: "two",
    name: "two.1",
    harness: null,
    cwd: null,
    state: "running" as const,
  };

  it("warns about the chats in the other window too", async () => {
    const { fire, sent } = core({ launch: ONE, windows: ["main", "window-1"] });
    render(<App />);
    await vi.waitFor(() => expect(projectTabs()).toEqual(["one*"]));
    fire("window-said", said("window-1", { ending: [ending] }));

    fire("quit-asked", null);

    const dialog = within(await screen.findByRole("alertdialog"));
    expect(within(dialog.getByRole("list")).getByText("two.1")).toBeInTheDocument();
    expect(sent("quit")).toEqual([]);
  });

  it("warns rather than quitting while another window has not said what it has open", async () => {
    const { fire, sent } = core({ launch: ONE, windows: ["main", "window-1"] });
    render(<App />);
    await vi.waitFor(() => expect(projectTabs()).toEqual(["one*"]));

    fire("quit-asked", null);

    expect(await screen.findByRole("alertdialog")).toBeInTheDocument();
    expect(sent("quit")).toEqual([]);
  });

  it("quits at once when no window has anything open", async () => {
    const { fire, sent } = core({ launch: ONE, windows: ["main", "window-1"] });
    render(<App />);
    await vi.waitFor(() => expect(projectTabs()).toEqual(["one*"]));
    fire("window-said", said("window-1"));

    await vi.waitFor(() => {
      fire("quit-asked", null);
      expect(sent("quit").length).toBeGreaterThan(0);
    });
  });
});

describe("a view's unsaved work across windows (#4028)", () => {
  afterEach(() => {
    for (const one of unsavedNow()) keepUnsaved(one.key, undefined);
    forgetHandOver();
  });

  const KIND = "test-work";
  /** A view's unsaved work in `plane`, as a view registers it: of a kind any window takes. */
  function work(plane: string, data = "text as typed"): UnsavedWork {
    const one: UnsavedWork = {
      key: `${plane}\u0000deploy`,
      plane,
      view: { from: null, view: "skill-studio", key: "" },
      name: "deploy",
      kind: KIND,
      handOver: () => data,
      save: vi.fn(() => Promise.resolve(undefined)),
      discard: vi.fn(() => keepUnsaved(one.key, undefined)),
    };
    keepUnsaved(one.key, one);
    return one;
  }
  takesHandedWork(KIND, (key, data) => void work(key.split("\u0000")[0], String(data)));
  const handed = (plane: string, data = "text as typed"): HandedWork => ({
    kind: KIND,
    key: `${plane}\u0000deploy`,
    plane,
    name: "deploy",
    data,
  });
  /** What this window sent to the window `label` on `event`. */
  const sentTo = (sent: (cmd: string) => Record<string, unknown>[], label: string, event: string) =>
    sent("plugin:event|emit_to")
      .filter(
        (one) =>
          one.event === event && (one.target as { label?: string } | undefined)?.label === label,
      )
      .map((one) => one.payload);

  it("goes with a project moved out of the main window, unasked", async () => {
    const { fire, listening, sent } = core({
      restore: { windows: [{ planes: [ONE, TWO], active: 0 }], dropped: [] },
    });
    render(<App />);
    await waitFor(() => expect(projectTabs()).toEqual(["one*", "two"]));
    await waitFor(() => expect(listening("unsaved-claim")).toBe(true));
    work(TWO);

    await fromTheMenu("two", "Move to a new window");

    await waitFor(() => expect(projectTabs()).toEqual(["one*"]));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(sent("move_projects")).toEqual([{ projects: [TWO], front: TWO, to: null }]);
    // It is no longer this window's,
    expect(unsavedNow()).toEqual([]);
    // and is handed to the window the project went to, once that window holds it.
    fire("unsaved-claim", { label: "window-1", plane: TWO });
    expect(sentTo(sent, "window-1", "unsaved-handed")).toEqual([[handed(TWO)]]);
  });

  it("goes with a project moved out of a split window once the main window has it", async () => {
    mockWindows("window-1");
    const { fire, sent } = core({ handed: { planes: [TWO], active: 0 } });
    render(<App />);
    await waitFor(() => expect(projectTabs()).toEqual(["two*"]));
    work(TWO);

    await fromTheMenu("two", "Move to the main window");

    await waitFor(() =>
      expect(sentTo(sent, "main", "unsaved-moving")).toEqual([
        { from: "window-1", plane: TWO, works: [handed(TWO)] },
      ]),
    );
    // Not moved until the main window says it holds the work.
    expect(sent("move_projects")).toEqual([]);
    fire("unsaved-carried", { to: "window-1", plane: TWO });

    await waitFor(() =>
      expect(sent("move_projects")).toEqual([{ projects: [TWO], front: TWO, to: "main" }]),
    );
    expect(screen.queryByRole("alertdialog")).toBeNull();
    await waitFor(() => expect(unsavedNow()).toEqual([]));
  });

  it("is asked about before a move where the main window does not say it has it", async () => {
    mockWindows("window-1");
    const { sent } = core({ handed: { planes: [TWO], active: 0 } });
    render(<App />);
    await waitFor(() => expect(projectTabs()).toEqual(["two*"]));
    const deploy = work(TWO);

    await fromTheMenu("two", "Move to the main window");

    const asked = await screen.findByRole(
      "alertdialog",
      { name: "Save your changes to deploy?" },
      { timeout: 5000 },
    );
    expect(sent("move_projects")).toEqual([]);
    await userEvent.click(within(asked).getByRole("button", { name: "Keep editing" }));
    expect(sent("move_projects")).toEqual([]);
    expect(deploy.discard).not.toHaveBeenCalled();
  });

  it("comes back to the main window with the projects of a split window closed", async () => {
    const { fire, listening } = core({ launch: ONE });
    render(<App />);
    await waitFor(() => expect(projectTabs()).toEqual(["one*"]));
    await waitFor(() => expect(listening("unsaved-mirror")).toBe(true));
    // What the split window last said it holds: the page goes with the window.
    fire("unsaved-mirror", { label: "window-1", works: [handed(TWO, "typed there")] });

    fire("projects-arrived", { planes: [TWO], front: null });

    await waitFor(() =>
      expect(unsavedNow().map((one) => [one.plane, one.handOver()])).toEqual([
        [TWO, "typed there"],
      ]),
    );
  });

  it("is taken in by a split window, which tells the main window what it holds", async () => {
    mockWindows("window-1");
    const { fire, listening, sent } = core({ handed: { planes: [TWO], active: 0 } });
    render(<App />);
    await waitFor(() => expect(projectTabs()).toEqual(["two*"]));
    await waitFor(() =>
      expect(sentTo(sent, "main", "unsaved-claim")).toContainEqual({
        label: "window-1",
        plane: TWO,
      }),
    );
    expect(listening("unsaved-handed")).toBe(true);

    // Only what is of a project it holds.
    fire("unsaved-handed", [handed(TWO, "typed in main"), handed(THREE, "not here")]);

    expect(unsavedNow().map((one) => one.handOver())).toEqual(["typed in main"]);
    await waitFor(() =>
      expect(sentTo(sent, "main", "unsaved-mirror").at(-1)).toEqual({
        label: "window-1",
        works: [handed(TWO, "typed in main")],
      }),
    );
  });

  it("is asked about at a quit in the split window that holds it, where Keep editing stays", async () => {
    const { fire, listening, sent } = core({ launch: ONE, windows: ["main", "window-1"] });
    render(<App />);
    await waitFor(() => expect(projectTabs()).toEqual(["one*"]));
    await waitFor(() => expect(listening("unsaved-mirror")).toBe(true));
    fire("window-said", said("window-1"));
    fire("unsaved-mirror", { label: "window-1", works: [handed(TWO)] });
    /** The split window asks, and answers. */
    const answers = async (asks: number, go: boolean) => {
      await waitFor(() =>
        expect(sentTo(sent, "window-1", "unsaved-ask-at-quit")).toHaveLength(asks),
      );
      fire("unsaved-asking-at-quit", { label: "window-1" });
      fire("unsaved-answered-at-quit", { label: "window-1", go });
    };

    fire("quit-asked", null);
    await answers(1, false);
    // Twice: this mock hands the quit to Strict Mode's let-go listener too, which is a second
    // quit asked while the first is up, and not taken. Then the answer's.
    await waitFor(() => expect(sent("quit_cancelled")).toHaveLength(2));
    expect(sent("quit")).toEqual([]);

    fire("quit-asked", null);
    await answers(2, true);
    await waitFor(() => expect(sent("quit")).toHaveLength(1));
  });

  it("is a quit not taken where the split window does not say it is asking", async () => {
    const { fire, listening, sent } = core({ launch: ONE, windows: ["main", "window-1"] });
    render(<App />);
    await waitFor(() => expect(projectTabs()).toEqual(["one*"]));
    await waitFor(() => expect(listening("unsaved-mirror")).toBe(true));
    fire("window-said", said("window-1"));
    fire("unsaved-mirror", { label: "window-1", works: [handed(TWO)] });

    fire("quit-asked", null);

    await waitFor(() => expect(sent("quit_cancelled")).toHaveLength(2), { timeout: 5000 });
    expect(sent("quit")).toEqual([]);
  });

  it("asks in the split window, in front, when the main window has it ask at a quit", async () => {
    mockWindows("window-1");
    const { fire, listening, sent } = core({ handed: { planes: [TWO], active: 0 } });
    render(<App />);
    await waitFor(() => expect(projectTabs()).toEqual(["two*"]));
    await waitFor(() => expect(listening("unsaved-ask-at-quit")).toBe(true));
    work(TWO);

    fire("unsaved-ask-at-quit", null);
    const asked = await screen.findByRole("alertdialog", { name: "Save your changes to deploy?" });
    expect(sentTo(sent, "main", "unsaved-asking-at-quit")).toEqual([{ label: "window-1" }]);
    expect(sent("show_window_holding")).toContainEqual({ plane: TWO });
    await userEvent.click(within(asked).getByRole("button", { name: "Keep editing" }));

    await waitFor(() =>
      expect(sentTo(sent, "main", "unsaved-answered-at-quit")).toEqual([
        { label: "window-1", go: false },
      ]),
    );
  });
});
