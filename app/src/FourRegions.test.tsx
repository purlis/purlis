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
import { emit } from "@tauri-apps/api/event";
import { ASKS_NOTIFIED } from "./askNotices";
import App from "./App";
import { forgetThisLaunch } from "./regions";
import { GLOBAL } from "./windowprefs";
import { onAMac } from "./tabKeys";
import { goToWorkspace, showTheExplorer, stripNamed } from "./test-strips";
import { drawnWith } from "./cascade.testkit";
import { tipOf } from "./ui/tooltip/tooltip.testkit";
import { selectNamed } from "./ui/select/select.testkit";

/**
 * **The window is four regions** (ADR 0038), against the whole app, because three of
 * them only mean anything together: the explorer picks where the next chat goes, the bar
 * starts it there, and the right-hand side is where the queue went.
 */

/**
 * **How often `PlaneView` renders** (#1160): counted where it calls `usePlaneChanged` for the
 * instructions a chat started on, which it does once per render and nothing else does at all.
 * The hook itself is the real one.
 */
const planeViewRenders = vi.hoisted(() => ({ count: 0 }));
vi.mock("./planeChanged", async (importOriginal) => {
  const real = await importOriginal<typeof import("./planeChanged")>();
  return {
    ...real,
    usePlaneChanged: (...args: Parameters<typeof real.usePlaneChanged>) => {
      if (args[1] === real.INSTRUCTIONS) planeViewRenders.count += 1;
      return real.usePlaneChanged(...args);
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
const ALPHA = `${PLANE}/workspaces/alpha`;
const BETA = `${PLANE}/workspaces/beta`;
const CUT = `${ALPHA}/.worktrees/svc`;

const START_OPTIONS = {
  profiles: [
    {
      name: "claude",
      kind: "claude",
      shown: "claude",
      source: "built-in",
      is_default: true,
      approval: null,
    },
  ],
  refused: [],
  personas: ["steward"],
  persona: "steward",
  persona_profiles: {},
  ignore_fix: null,
  declares_none: true,
};

function chat(session: number, name: string, cwd: string | null) {
  return {
    session,
    name,
    cwd,
    harness: "claude",
    in_front: false,
    resumed: null,
    fresh: null,
    profile: "claude",
    persona: "steward",
    unreported: null,
    guessed: null,
  };
}

function piece(name: string, on: Record<string, unknown> = {}) {
  return { piece: name, path: `${CUT}/${name}`, branch: name, wired: true, stale: false, ...on };
}

/** The core, filing chats by the directory each one works in, as it really does.
 *
 *  `cut` is asked afresh on every listing, so a test can remove a worktree mid-run. */
function core(
  waiting: number[] = [],
  cut: () => ReturnType<typeof piece>[] = () => [piece("one"), piece("two")],
) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  const chats: ReturnType<typeof chat>[] = [];
  // Branches cut from the window (GL-1), listed after the ones `cut` names.
  const madeHere: string[] = [];
  let next = 0;
  const answering: Parameters<typeof mockIPC>[0] = (cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: a });
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    // A picked branch's files are drawn in Explorer's Files section (#1677), with its changes.
    if (cmd === "branch_tree") return { entries: [], more: 0 };
    if (cmd === "branch_status") return { changes: [], folders: [], more: 0, base: "main" };
    if (cmd === "opened_chats") return [];
    if (cmd === "start_chat") {
      const answer = () => {
        chats.push(chat(++next, String(a.name), a.cwd as string | null));
        return { session: next };
      };
      // A start the test is holding open, as a checkout off the main thread holds one.
      return startHeld === undefined ? answer() : startHeld.then(answer);
    }
    // The operator has pinned every workspace, so every one is on the strip and can be
    // clicked there: the strip draws what is pinned and the one you are in (ADR 0054).
    if (cmd === "plane_pins") return { project: false, workspaces: ["alpha", "beta"], missing: [] };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        personas: ["steward"],
        persona: "steward",
        unfiled: chats.filter((one) => one.cwd === null || !one.cwd.startsWith(`${PLANE}/`)),
        workspaces: [
          {
            name: "alpha",
            path: ALPHA,
            vision: "Ship it",
            todos: [],
            chats: chats.filter((one) => one.cwd?.startsWith(ALPHA)),
          },
          {
            name: "beta",
            path: BETA,
            vision: "Retire the importer",
            todos: [],
            chats: chats.filter((one) => one.cwd?.startsWith(BETA)),
          },
        ],
      };
    if (cmd === "workspace_panels")
      return {
        workspace: a.workspace,
        repos: a.workspace === "alpha" ? ["svc"] : [],
        paths: a.workspace === "alpha" ? { svc: `${ALPHA}/svc` } : {},
        absent: [],
        refused: [],
        todos: todos.map(({ slug, title }) => ({ slug, title, stamp: "2026-10-10" })),
        todos_refused: null,
        personas: ["steward"],
        persona: "steward",
        sessions: records.map(({ slug, title }) => ({
          path: `${ALPHA}/sessions/${slug}.md`,
          title,
          when: "2026-10-10 16:50",
          persona: "steward",
          harness: "claude",
          resumable: false,
        })),
        contributed: [
          ownPanel("todos", "Todos", 10, todos),
          ownPanel("memory", "Memory", 15, memories),
          ownPanel("sessions", "Sessions", 20, records),
        ],
      };
    if (cmd === "extension_panels") return extensionPanels;
    if (cmd === "extensions_on") return ["stats"];
    if (cmd === "workspace_repos")
      return {
        workspace: a.workspace,
        repos: a.workspace === "alpha" ? repos : [],
        cache_refused: null,
      };
    if (cmd === "worktree_list")
      return a.workspace === "alpha" ? [...cut(), ...madeHere.map((name) => piece(name))] : [];
    if (cmd === "worktree_add") {
      const name = (a.branch as string | null) ?? "chat-1";
      madeHere.push(name);
      return { piece: name, path: `${CUT}/${name}`, branch: name, warnings: [] };
    }
    if (cmd === "start_options") return START_OPTIONS;
    if (cmd === "chat_states")
      return waiting.map((session) => ({ session, state: "waiting", queue: waiting, sequence: 1 }));
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "alerts_everywhere") return alerts;
    if (cmd === "asks_waiting") return { plane: PLANE, asks: waitingAsks };
    return null;
  };
  mockIPC(answering, { shouldMockEvents: mockEvents });
  return { asked };
}

/** One of purlis's own panels, as `workspace_panels` contributes it. */
function ownPanel(
  id: string,
  title: string,
  order: number,
  rows: { slug: string; title: string }[],
) {
  return {
    key: `charter/${id}`,
    title,
    order,
    mark: "todo",
    from: null,
    about: null,
    blocks: [
      {
        kind: "list",
        rows: rows.map(({ slug, title: text }) => ({
          key: slug,
          text,
          note: null,
          mark: "todo",
          tone: "plain",
          detail: null,
          runs: null,
          actions: [],
        })),
        empty: { headline: "Nothing here", body: null, offer: null },
      },
    ],
  };
}

/** The focused workspace's open todos. A test sets it before `core()`. */
let todos: { slug: string; title: string }[] = [];

/** The focused workspace's memories, and its session records. A test changes them, then says
 *  the plane changed. */
let memories: { slug: string; title: string }[] = [];
let records: { slug: string; title: string }[] = [];

/** The panels approved extensions contribute. A test sets it before `core()`. */
let extensionPanels: unknown[] = [];

/** While set, `start_chat` answers only once it settles: a start still in flight. */
let startHeld: Promise<void> | undefined;

/** What the core says is wrong in every project it holds. A test sets it before `core()`. */
let alerts: unknown = [];

/** What git says of alpha's clones. A test sets it before `core()`. */
let repos: unknown[] = [];

/** Whether the core's events are mocked, so a test can send one (#1694). Set before `core()`. */
let mockEvents = false;

/** What the asks registry derives for the project (#1690). A test sets it before `core()`. */
let waitingAsks: unknown[] = [];

/** A permission prompt chat `session` holds on its hook, as the registry lists it. */
const asking = (session: number, says: string) => ({
  session,
  ask: `ask-${session}`,
  says,
  options: [
    { id: "allow", label: "Allow", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: [`steward ${session}`],
  answer: { via: "hook" },
});

/** One clone's git answer, clean unless the test says otherwise. */
function repoState(name: string, on: Record<string, unknown> = {}) {
  return {
    name,
    branch: "main",
    unborn: false,
    detached: null,
    upstream: null,
    ahead: 0,
    behind: 0,
    tracked: 0,
    untracked: 0,
    unreadable: null,
    ci: null,
    change: null,
    sigil: null,
    fetched_seconds_ago: null,
    not_fetched: "nothing fetched yet",
    ...on,
  };
}

beforeEach(() => {
  startHeld = undefined;
  todos = [];
  memories = [{ slug: "m1", title: "The deploy key rotates" }];
  records = [];
  extensionPanels = [];
  globalThis.localStorage.clear();
  // Every test is a launch: what an earlier one toggled is not this one's arrangement.
  forgetThisLaunch();
  alerts = [{ plane: PLANE, alerts: [], stopped: null }];
  repos = [];
  waitingAsks = [];
  mockEvents = false;
});
afterEach(() => {
  cleanup();
  clearMocks();
});

/** Opens a chat where the operator is standing: New tab, a row, Start. */
async function openAChat() {
  await userEvent.click(await screen.findByRole("button", { name: "New tab" }));
  await userEvent.click(await screen.findByRole("button", { name: "Start chat" }));
}

/** Focuses a workspace from the title bar's ⌄ (#2916), which is the axis (ADR 0036). */
async function focus(workspace: string) {
  await goToWorkspace(workspace);
}

/** Where the last chat was started. */
const startedIn = (asked: { cmd: string; args: Record<string, unknown> }[]) =>
  asked.filter((one) => one.cmd === "start_chat").map((one) => one.args.cwd);

/** A side's key, as a person presses it: ⌘B for the left, ⌥⌘B for the right (Ctrl off a Mac).
 *  Since the status line's toggles went (qw357) a side is put away by its key, its activity
 *  bar's open view or the palette's row. */
function sideKey(side: "left" | "right") {
  fireEvent.keyDown(document.body, {
    key: "b",
    code: "KeyB",
    altKey: side === "right",
    ...(onAMac() ? { metaKey: true } : { ctrlKey: true }),
  });
}
/** Whether a side is put away: its activity bar stands and none of its views is the open one. */
function away(region: "Navigation" | "Attention"): boolean {
  // The left column stands while its side is away (#3144), none of its tabs chosen.
  const bar = screen.queryByRole("tablist", { name: region });
  return (
    bar === null ||
    within(bar)
      .getAllByRole("tab")
      .every((tab) => tab.getAttribute("aria-selected") !== "true")
  );
}

describe("the four regions", () => {
  it("draws every region, and each one is named for what it holds", async () => {
    core();
    render(<App />);

    // The left side opens on its Chats view (#1673, B-2), with the Explorer one press away.
    expect(await screen.findByRole("tabpanel", { name: "Chats" })).toBeInTheDocument();
    expect(await showTheExplorer()).toBeInTheDocument();
    expect(await screen.findByRole("tabpanel", { name: "Memory" })).toBeInTheDocument();
    expect(await screen.findByLabelText("Repository state")).toBeInTheDocument();
    expect(stripNamed("Tabs")).toBeInTheDocument();
  });

  it("has nothing called Workspaces: the workspaces are the title bar's", async () => {
    // Two `nav[aria-label="Workspaces"]` broke a spec when the second appeared. The old left
    // sidebar was the second one; the explorer is not.
    core();
    render(<App />);
    // The strip itself, and not the explorer: the explorer draws as soon as the project does,
    // and the strip only once the plane has been read — so waiting on the explorer would ask
    // this question before the thing it is about exists.
    await screen.findByRole("toolbar", { name: "Pinned workspaces" });
    await screen.findByTestId("clone-svc");

    // The workspaces are the title bar's (#2916), and nothing else is called Workspaces.
    expect(screen.queryAllByLabelText("Workspaces")).toHaveLength(0);
  });

  it("starts a chat in the workspace when nothing in the explorer is picked", async () => {
    const { asked } = core();
    render(<App />);
    await screen.findByTestId("clone-svc");

    await openAChat();

    expect(startedIn(asked)).toEqual([ALPHA]);
  });

  it("starts a chat in the worktree the explorer picked", async () => {
    // What makes the left a SELECTOR rather than a second listing. The path is the one the
    // core spelled in `worktree_list`; the window never joins one together.
    const { asked } = core();
    render(<App />);
    await screen.findByTestId("clone-svc");
    await showTheExplorer();

    await userEvent.click(await screen.findByRole("treeitem", { name: /^two/ }));
    await openAChat();

    expect(startedIn(asked)).toEqual([`${CUT}/two`]);
  });

  it("starts a chat in a clone picked from its menu, on the path the core spelled", async () => {
    // charter-app#174: a clone is picked one level up from a piece, from the heading's menu,
    // and the explorer marks it the way it marks a picked piece.
    const { asked } = core();
    render(<App />);
    const clone = await screen.findByTestId("clone-svc");
    await showTheExplorer();
    const heading = within(clone).getByRole("treeitem", { name: /^svc/ });

    fireEvent.contextMenu(heading);
    await userEvent.click(await screen.findByRole("menuitem", { name: "Start new chats here" }));
    await waitFor(() => expect(heading).toHaveAttribute("aria-current", "true"));
    await openAChat();

    expect(startedIn(asked)).toEqual([`${ALPHA}/svc`]);
  });

  it("opens the picker for a new tab in the clone from the Changes view's row too", async () => {
    const { asked } = core();
    render(<App />);
    const row = await screen.findByTestId("repo-svc");

    fireEvent.contextMenu(row);
    await userEvent.click(await screen.findByRole("menuitem", { name: "New tab" }));
    await userEvent.click(await screen.findByRole("button", { name: "Start chat" }));

    expect(startedIn(asked)).toEqual([`${ALPHA}/svc`]);
  });

  it("starts a chat in a clone on a new branch of its own unless the box is cleared (GL-1)", async () => {
    const { asked } = core();
    render(<App />);
    const boxes = () =>
      asked.filter((one) => one.cmd === "start_chat").map((one) => one.args.boxes);

    fireEvent.contextMenu(await screen.findByTestId("repo-svc"));
    await userEvent.click(await screen.findByRole("menuitem", { name: "New tab" }));
    await userEvent.click(await screen.findByRole("button", { name: "Start chat" }));
    await waitFor(() => expect(boxes()).toHaveLength(1));
    fireEvent.contextMenu(await screen.findByTestId("repo-svc"));
    await userEvent.click(await screen.findByRole("menuitem", { name: "New tab" }));
    await userEvent.click(await screen.findByRole("checkbox", { name: /new branch in svc/ }));
    await userEvent.click(await screen.findByRole("button", { name: "Start chat" }));
    await waitFor(() => expect(boxes()).toHaveLength(2));
    await openAChat();

    expect(boxes()).toEqual([
      { show_footer: false, new_branch: true, without_sandbox: null },
      { show_footer: false, new_branch: false, without_sandbox: null },
      // The workspace's own directory is in no repo: nothing to cut, and nothing asked for.
      { show_footer: false, new_branch: false, without_sandbox: null },
    ]);
  });

  it("cuts a new branch from a repo's menu, and new chats start on it (GL-1)", async () => {
    const { asked } = core();
    render(<App />);
    await showTheExplorer();
    const heading = within(await screen.findByTestId("clone-svc")).getByRole("treeitem", {
      name: /^svc/,
    });

    fireEvent.contextMenu(heading);
    await userEvent.click(await screen.findByRole("menuitem", { name: "New branch…" }));
    await userEvent.type(
      within(await screen.findByRole("dialog", { name: "New branch" })).getByRole("textbox", {
        name: /^Name/,
      }),
      "spike{Enter}",
    );
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "New branch" })).not.toBeInTheDocument(),
    );
    await screen.findByTestId("piece-svc-spike");
    await openAChat();

    expect(asked.find(({ cmd }) => cmd === "worktree_add")?.args).toMatchObject({
      plane: PLANE,
      workspace: "alpha",
      repo: "svc",
      branch: "spike",
    });
    expect(startedIn(asked)).toEqual([`${CUT}/spike`]);
  });

  it("starts one chat however often Start is pressed while it is starting (GL-1)", async () => {
    // Off the main thread, a start takes as long as its checkout, and a second press of
    // Start in that time used to start a second chat.
    let release = () => {};
    startHeld = new Promise((done) => (release = done));
    const { asked } = core();
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "New tab" }));
    await userEvent.click(await screen.findByRole("button", { name: "Start chat" }));
    const starting = await screen.findByRole("button", { name: "Starting…" });
    expect(starting).toBeDisabled();
    fireEvent.click(starting);
    release();
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Start a chat" })).not.toBeInTheDocument(),
    );

    expect(asked.filter((one) => one.cmd === "start_chat")).toHaveLength(1);
  });

  it("starts only that tab in the clone, and the next New tab where it always did", async () => {
    // The review of #174: `New tab in svc` had set the explorer's pick, which made it the
    // same row as `Start new chats in svc` under another name.
    const { asked } = core();
    render(<App />);
    fireEvent.contextMenu(await screen.findByTestId("repo-svc"));
    await userEvent.click(await screen.findByRole("menuitem", { name: "New tab" }));
    await userEvent.click(await screen.findByRole("button", { name: "Start chat" }));
    await waitFor(() => expect(startedIn(asked)).toHaveLength(1));

    await openAChat();

    expect(startedIn(asked)).toEqual([`${ALPHA}/svc`, ALPHA]);
  });

  it("does not carry a pick into another workspace", async () => {
    // A pick belongs to the workspace it was made in. Carrying it across would point the
    // next chat at a directory in the workspace the operator has just left.
    const { asked } = core();
    render(<App />);
    await screen.findByTestId("clone-svc");
    await showTheExplorer();
    await userEvent.click(await screen.findByRole("treeitem", { name: /^two/ }));

    await focus("beta");
    await openAChat();

    expect(startedIn(asked)).toEqual([BETA]);
  });

  it("brings the pick back when its workspace is focused again", async () => {
    // Set aside, not thrown away — the same courtesy the chat strip does with the tab that
    // was in front on it.
    const { asked } = core();
    render(<App />);
    await screen.findByTestId("clone-svc");
    await showTheExplorer();
    await userEvent.click(await screen.findByRole("treeitem", { name: /^two/ }));

    await focus("beta");
    await focus("alpha");
    await openAChat();

    expect(startedIn(asked)).toEqual([`${CUT}/two`]);
  });

  it("drops a pick whose worktree git no longer lists", async () => {
    // The worktree was removed while it was picked. Starting the next chat in a directory
    // that is not there would make the operator read a refusal charter could see coming.
    let cut = [piece("one"), piece("two")];
    const { asked } = core([], () => cut);

    render(<App />);
    await screen.findByTestId("clone-svc");
    await showTheExplorer();
    await userEvent.click(await screen.findByRole("treeitem", { name: /^two/ }));

    cut = [piece("one")];
    await focus("beta");
    await focus("alpha");
    await screen.findByTestId("clone-svc");
    await openAChat();

    expect(startedIn(asked)).toEqual([ALPHA]);
  });

  it("puts the needs-you queue in the title bar, and not on the right (charter-app#249)", async () => {
    waitingAsks = [asking(7, "Run cargo test")];
    core([7]);
    render(<App />);

    const hand = await screen.findByRole("button", { name: "1 thing waits on you" });
    expect(screen.getByTestId("title-bar")).toContainElement(hand);
    expect(
      within(await screen.findByRole("tabpanel", { name: "Memory" })).queryByLabelText("Needs you"),
    ).toBeNull();
  });

  it("puts a region away and brings it back", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });
    const chats = screen.getByTestId("chats-section");

    sideKey("left");
    await waitFor(() =>
      expect(screen.queryByRole("tabpanel", { name: "Chats" })).not.toBeInTheDocument(),
    );
    expect(away("Navigation")).toBe(true);
    // The centre is still there: a window with no panes is not a state a key can reach.
    expect(stripNamed("Tabs")).toBeInTheDocument();

    sideKey("left");
    expect(await screen.findByRole("tabpanel", { name: "Chats" })).toBeInTheDocument();
    // Hidden while it was away, never unmounted (#1673): the same list came back.
    expect(screen.getByTestId("chats-section")).toBe(chats);
  });

  it("can put the right-hand side away too", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Inbox" });

    sideKey("right");

    await waitFor(() =>
      expect(screen.queryByRole("tabpanel", { name: "Inbox" })).not.toBeInTheDocument(),
    );
    // The Panel's right half is not the sidebar's (#1738).
    expect(screen.getByRole("tabpanel", { name: "Memory" })).toBeInTheDocument();
  });

  it("gives every side its way back on its own activity bar, and none on the status line", async () => {
    // The operator, 2026-10-13: *"bottom toggle icons are redundant now … they can be removed
    // fully"* (qw357). They were on the status line since charter-app#193; a side's activity
    // bar stands while the side is away, so the way back is where the side is.
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });

    expect(screen.getByTestId("status-line").querySelector("[aria-pressed]")).toBeNull();
    // Nor on the chat strip, which is the editor's tab bar now (DS-6).
    expect(document.querySelector("header.bar [aria-pressed]")).toBeNull();

    // The right bar stands while its side is away, and says how to come back.
    const tab = () =>
      within(screen.getByRole("tablist", { name: "Attention" })).getByRole("tab", {
        name: "Inbox",
      });
    sideKey("right");
    await waitFor(() => expect(away("Attention")).toBe(true));
    expect(tipOf(tab())).toEqual(expect.stringContaining("Show the Inbox view"));
    await userEvent.click(tab());
    expect(await screen.findByRole("tabpanel", { name: "Inbox" })).toBeInTheDocument();
    expect(away("Attention")).toBe(false);

    // The left side's key puts it away and brings it back too.
    sideKey("left");
    await waitFor(() => expect(away("Navigation")).toBe(true));
    sideKey("left");
    expect(await screen.findByRole("tabpanel", { name: "Chats" })).toBeInTheDocument();
  });

  it("keeps a chat in another workspace running while the explorer is used", async () => {
    // #125's guarantee, one scope down: nothing in this region ends anything.
    const { asked } = core();
    render(<App />);
    await screen.findByTestId("clone-svc");
    await openAChat();
    await showTheExplorer();

    await userEvent.click(await screen.findByRole("treeitem", { name: /^one/ }));
    await focus("beta");
    await focus("alpha");

    expect(asked.filter((one) => one.cmd === "close_session")).toEqual([]);
    expect(screen.getAllByTestId("pane")).toHaveLength(1);
  });
});

/**
 * **The left side's activity bar, in the window** (#1673, ADR 0038 as amended 2026-10-10): what
 * a person presses and sees. The bar's own rules are `RegionFrame.test.tsx`'s; the press's are
 * `regions.test.ts`'s.
 */
describe("the left side's activity bar", () => {
  const bar = () => screen.getByRole("tablist", { name: "Navigation" });
  const tab = (name: string) => within(bar()).getByRole("tab", { name });
  /** The chord with the platform's command key: ⌘ on a Mac, Ctrl elsewhere. */
  const chord = (key: string, shift = false) =>
    fireEvent.keyDown(document.body, {
      key,
      shiftKey: shift,
      ...(onAMac() ? { metaKey: true } : { ctrlKey: true }),
    });

  it("switches the side between Chats and Explorer, one at a time", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });
    expect(tab("Chats")).toHaveAttribute("aria-selected", "true");
    expect(screen.queryByRole("navigation", { name: "Explorer" })).toBeNull();

    await userEvent.click(tab("Explorer"));

    expect(await screen.findByRole("navigation", { name: "Explorer" })).toBeInTheDocument();
    expect(screen.queryByRole("tabpanel", { name: "Chats" })).toBeNull();
    expect(tab("Explorer")).toHaveAttribute("aria-selected", "true");
  });

  it("puts the side away on a press of the open view, and brings it back on the next (R121, #3329)", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });
    const chats = screen.getByTestId("chats-section");

    await userEvent.click(tab("Chats"));

    await waitFor(() => expect(away("Navigation")).toBe(true));
    expect(screen.queryByRole("tabpanel", { name: "Chats" })).toBeNull();
    // The icon says it is no longer the open one; the view is hidden, never unmounted.
    expect(tab("Chats")).toHaveAttribute("aria-selected", "false");
    expect(screen.getByTestId("chats-section")).toBe(chats);

    await userEvent.click(tab("Chats"));

    expect(await screen.findByRole("tabpanel", { name: "Chats" })).toBeInTheDocument();
    expect(away("Navigation")).toBe(false);
    expect(screen.getByTestId("chats-section")).toBe(chats);
  });

  it("closes and reopens the side from the keyboard, which stays on the icon (#3329)", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });
    act(() => tab("Chats").focus());

    await userEvent.keyboard("{Enter}");

    await waitFor(() => expect(away("Navigation")).toBe(true));
    expect(tab("Chats")).toHaveAttribute("aria-selected", "false");
    // Heard as collapsed: the tab of the view the side comes back on says it is not out.
    expect(tab("Chats")).toHaveAttribute("aria-expanded", "false");
    expect(tab("Explorer")).not.toHaveAttribute("aria-expanded");
    expect(tab("Chats")).toHaveFocus();

    await userEvent.keyboard(" ");

    expect(await screen.findByRole("tabpanel", { name: "Chats" })).toBeInTheDocument();
    expect(tab("Chats")).toHaveAttribute("aria-selected", "true");
    expect(tab("Chats")).toHaveAttribute("aria-expanded", "true");
    expect(tab("Chats")).toHaveFocus();
  });

  it("counts the chats that need you on the Chats tab, plainly, while the side is out or away", async () => {
    waitingAsks = [asking(7, "Run cargo test")];
    core([7]);
    render(<App />);
    await screen.findByRole("button", { name: "1 thing waits on you" });
    await waitFor(() => expect(tab("Chats")).toHaveAccessibleDescription("1 chat needs you"));
    // Only what waits in the Inbox is drawn hot (R120); the Chats count is plain (R119).
    const count = tab("Chats").querySelector('[data-part="count"]');
    expect(count).toHaveTextContent("1");
    expect(count).not.toHaveAttribute("data-tone");

    // A press puts the side away (R121, #3329); the count stays on the icon.
    await userEvent.click(tab("Chats"));

    await waitFor(() => expect(away("Navigation")).toBe(true));
    expect(tab("Chats")).toHaveAccessibleDescription("1 chat needs you");
  });

  it("draws no count when nothing needs you", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });

    expect(tab("Chats").querySelector('[data-part="count"]')).toBeNull();
  });

  it("answers its keys: the side away and back, then Explorer and Chats shown", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });

    chord("b");
    await waitFor(() => expect(screen.queryByRole("tabpanel", { name: "Chats" })).toBeNull());
    chord("b");
    expect(await screen.findByRole("tabpanel", { name: "Chats" })).toBeInTheDocument();

    chord("E", true);
    expect(await screen.findByRole("navigation", { name: "Explorer" })).toBeInTheDocument();
    // The key gives the view the keyboard, as an editor's does.
    await waitFor(() =>
      expect(document.activeElement?.closest('[role="tabpanel"]')).toHaveAccessibleName("Explorer"),
    );

    chord("C", true);
    expect(await screen.findByRole("tabpanel", { name: "Chats" })).toBeInTheDocument();
  });

  it("comes back as it was left when the project is opened again", async () => {
    core();
    const { unmount } = render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });
    await userEvent.click(tab("Explorer"));
    await screen.findByRole("navigation", { name: "Explorer" });
    unmount();

    core();
    render(<App />);

    expect(await screen.findByRole("navigation", { name: "Explorer" })).toBeInTheDocument();
  });
});

/**
 * **Search and Changes on the left** (#1676, B-2, B-7): two more views on the navigation
 * region's bar. Search is the Search tab's content in the side; Changes is what the bottom
 * region drew, which is gone, so the terminals have the window's whole height.
 */
describe("a side's views, bounded", () => {
  it("draws every view as the containing block of what is in it, clipping what hangs out", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });

    // A visually hidden word in a row far down the list, placed absolutely, made the page
    // taller than the window when nothing positioned stood between it and the page.
    const views = [...document.querySelectorAll('[role="tabpanel"].region-view')];
    expect(views.length).toBeGreaterThan(1);
    for (const view of views) {
      expect(drawnWith(view, "position")).toBe("relative");
      expect(drawnWith(view, "overflow")).toBe("hidden");
    }
    expect(drawnWith(screen.getByTestId("chats-section"), "position")).toBe("relative");
  });
});

describe("the Search and Changes views", () => {
  const bar = () => screen.getByRole("tablist", { name: "Navigation" });
  const tab = (name: string) => within(bar()).getByRole("tab", { name });

  it("holds every view of the left side, in the bar's order", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });

    expect(
      within(bar())
        .getAllByRole("tab")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual(["Chats", "Explorer", "Search"]);
  });

  it("shows Search on its key, with the box given the keyboard", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });

    fireEvent.keyDown(document.body, {
      key: "F",
      shiftKey: true,
      ...(onAMac() ? { metaKey: true } : { ctrlKey: true }),
    });

    const view = await screen.findByRole("tabpanel", { name: "Search" });
    const box = within(view).getByRole("searchbox", { name: "Search the files" });
    await waitFor(() => expect(box).toHaveFocus());
    // A view in the side, not a tab in the centre: the tab strip gained nothing.
    expect(within(stripNamed("Tabs")).queryByRole("tab", { name: /^Search/ })).toBeNull();
  });

  it("keeps what was typed when the side is switched away and back", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });
    await userEvent.click(tab("Search"));
    const box = within(await screen.findByRole("tabpanel", { name: "Search" })).getByRole(
      "searchbox",
    );
    // `change` rather than typed keys: jsdom lays nothing out, so the panels' separator takes
    // a pointer press anywhere in the group (as `ChatsList.window.test.tsx` meets it too).
    fireEvent.change(box, { target: { value: "needle" } });

    await userEvent.click(tab("Chats"));
    await userEvent.click(tab("Search"));

    expect(
      within(await screen.findByRole("tabpanel", { name: "Search" })).getByRole("searchbox"),
    ).toHaveValue("needle");
  });

  it("searches where the window is when its tab opens it, as its key does", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });
    const scope = () =>
      selectNamed("Where to search", screen.getByRole("tabpanel", { name: "Search" }));
    await userEvent.click(tab("Search"));
    await waitFor(() => expect(scope()).toHaveTextContent("Workspace alpha"));

    await userEvent.click(tab("Chats"));
    await focus("beta");
    await userEvent.click(tab("Search"));

    await waitFor(() => expect(scope()).toHaveTextContent("Workspace beta"));
  });

  it("shows Changes on ⌃⇧G, holding the repos, their branches and pipelines", async () => {
    repos = [repoState("svc", { tracked: 3, untracked: 1 })];
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });

    fireEvent.keyDown(document.body, { key: "G", shiftKey: true, ctrlKey: true });

    const view = await screen.findByRole("tabpanel", { name: "Changes" });
    // In the Panel's left half, where it lives since #1739.
    expect(screen.getByTestId("region-panel")).toContainElement(view);
    const state = within(view).getByLabelText("Repository state");
    await waitFor(() => expect(within(state).getByTestId("repo-svc")).toHaveTextContent("main"));
    expect(
      within(state).getByRole("row", { name: "changes: 3 changed, 1 untracked" }),
    ).toBeInTheDocument();
    expect(within(state).getByTestId("ci-svc")).toHaveTextContent("not fetched");
  });

  it("has no bottom region any more: what it showed is in Changes alone", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });

    expect(screen.queryByTestId("region-bottom")).toBeNull();
    expect(screen.getAllByLabelText("Repository state")).toHaveLength(1);
    expect(screen.getByLabelText("Repository state").closest('[role="tabpanel"]')).toHaveAttribute(
      "data-view",
      "changes",
    );
    expect(screen.queryByRole("button", { name: "State" })).toBeNull();
  });
});

/**
 * **Changes in the Panel, the Lower icons' badges, and every view's key** (#1739, P-2, P-6,
 * P-9): Changes moved to the Panel's left half, after Repos; a Lower icon carries its view's
 * count as the sidebars' icons do, so it is there while its half is away; and a view's key opens
 * it wherever it lives.
 */
describe("the Lower icons' badges and the views' keys", () => {
  const lower = () => screen.getByRole("tablist", { name: "Panel, left half" });
  const lowerTab = (name: string) => within(lower()).getByRole("tab", { name });
  /** A key as this platform types it: ⌘ on a Mac, Ctrl elsewhere. */
  const command = (key: string, more: Record<string, boolean> = {}) =>
    fireEvent.keyDown(document.body, {
      key,
      ...more,
      ...(onAMac() ? { metaKey: true } : { ctrlKey: true }),
    });

  it("draws Changes as a Lower icon after Repos, the left bar's own being Chats, Explorer and Search", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Repos" });

    expect(
      within(lower())
        .getAllByRole("tab")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual(["Repos", "Changes"]);
    await userEvent.click(lowerTab("Changes"));
    expect(
      within(screen.getByTestId("region-panel")).getByRole("tabpanel", { name: "Changes" }),
    ).toBeInTheDocument();
  });

  it("counts the uncommitted files on Changes' Lower icon, while its half is away too", async () => {
    repos = [
      repoState("svc", { tracked: 3, untracked: 1 }),
      repoState("tool", { tracked: 1 }),
      // A tree purlis could not read is not counted as clean, nor as anything.
      repoState("gone", { unreadable: "permission denied" }),
    ];
    core();
    render(<App />);
    await waitFor(() =>
      expect(lowerTab("Changes")).toHaveAccessibleDescription("5 uncommitted files"),
    );

    // The open Lower icon pressed: the half is put away, and the count stays on its icon.
    await userEvent.click(lowerTab("Repos"));

    expect(screen.queryByRole("tabpanel", { name: "Repos" })).toBeNull();
    expect(lowerTab("Changes").querySelector('[data-part="count"]')).toHaveTextContent("5");
    expect(lowerTab("Changes")).toHaveAccessibleDescription("5 uncommitted files");
    // Not a question for the person: the plain count's colours, never the needs-you ones.
    expect(
      lowerTab("Changes").querySelector('[data-part="count"][data-tone="needs-you"]'),
    ).toBeNull();
  });

  it("draws no count on Changes when every tree is clean", async () => {
    repos = [repoState("svc")];
    core();
    render(<App />);
    await userEvent.click(await waitFor(() => lowerTab("Changes")));
    await within(await screen.findByRole("tabpanel", { name: "Changes" })).findByTestId("repo-svc");

    expect(lowerTab("Changes").querySelector('[data-part="count"]')).toBeNull();
  });

  it("draws how far behind and ahead the repos are on Repos' icon, once one is behind (P-9)", async () => {
    repos = [
      repoState("svc", { upstream: "origin/main", behind: 3, ahead: 1 }),
      repoState("tool", { upstream: "origin/main", ahead: 2 }),
      // What purlis could not read says nothing about how far it is.
      repoState("gone", { unreadable: "permission denied", behind: 9 }),
    ];
    core();
    render(<App />);
    await waitFor(() =>
      expect(lowerTab("Repos")).toHaveAccessibleDescription("3 commits behind, 3 ahead"),
    );
    await userEvent.click(lowerTab("Repos"));

    // Drawn by the stylesheet from `data-count`, out of the item's name (axe, #2917).
    expect(lowerTab("Repos").querySelector('[data-part="count"]')).toHaveAttribute(
      "data-count",
      "3↓3↑",
    );
    expect(lowerTab("Repos")).toHaveAccessibleDescription("3 commits behind, 3 ahead");
  });

  it("draws nothing on Repos' icon while nothing is behind, however far ahead", async () => {
    repos = [repoState("svc", { upstream: "origin/main", ahead: 4, tracked: 1 })];
    core();
    render(<App />);
    // Changes' count arrives with the same answer, so Repos' has had its chance to be drawn.
    await waitFor(() =>
      expect(lowerTab("Changes")).toHaveAccessibleDescription("1 uncommitted file"),
    );

    expect(lowerTab("Repos").querySelector('[data-part="count"]')).toBeNull();
    expect(lowerTab("Repos")).not.toHaveAttribute("aria-describedby");
  });

  it("opens every view on its own key where it lives: a sidebar, or the Panel's half (P-6)", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });
    const keys: [string, () => void, string][] = [
      ["Explorer", () => command("E", { shiftKey: true }), "region-left"],
      ["Search", () => command("F", { shiftKey: true }), "region-left"],
      [
        "Changes",
        () => fireEvent.keyDown(document.body, { key: "G", shiftKey: true, ctrlKey: true }),
        "region-panel",
      ],
      ["Chats", () => command("C", { shiftKey: true }), "region-left"],
      ["Inbox", () => command("I", { shiftKey: true }), "region-right"],
    ];

    for (const [name, press, where] of keys) {
      press();
      const view = await screen.findByRole("tabpanel", { name });
      expect(screen.getByTestId(where), name).toContainElement(view);
    }
  });

  it("opens Changes in its half on ⌃⇧G though ⌘J hid the Panel, the sidebar left on its view", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Repos" });
    command("E", { shiftKey: true });
    await screen.findByRole("tabpanel", { name: "Explorer" });
    command("j");
    await waitFor(() => expect(screen.queryByRole("tabpanel", { name: "Repos" })).toBeNull());

    fireEvent.keyDown(document.body, { key: "G", shiftKey: true, ctrlKey: true });

    const view = await screen.findByRole("tabpanel", { name: "Changes" });
    expect(screen.getByTestId("region-panel")).toContainElement(view);
    expect(lowerTab("Changes")).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel", { name: "Explorer" })).toBeInTheDocument();
  });

  it("opens Changes in its half from the palette's row too, which says where it is", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Repos" });

    await userEvent.keyboard("{F2}");
    const palette = await screen.findByRole("dialog", { name: "Command palette" });
    await userEvent.type(within(palette).getByRole("combobox"), "show the changes");
    const row = await within(palette).findByRole("option", { name: /Show the Changes view/ });
    expect(row).toHaveTextContent(/in the Panel/);
    await userEvent.click(row);

    const view = await screen.findByRole("tabpanel", { name: "Changes" });
    expect(screen.getByTestId("region-panel")).toContainElement(view);
  });
});

/**
 * **The Panel, in the window** (spec #1736, P-1 to P-14; #1737): a full-width region under the
 * sidebars and the editor, its left half switched by the left bar's Lower icons, Repos in it by
 * default, out of Explorer. What a press or ⌘J decides is `regions.test.ts`'s; where the Panel is
 * drawn is `RegionFrame.test.tsx`'s; this is what a person presses and sees.
 */
describe("the Panel", () => {
  const lower = () => screen.getByRole("tablist", { name: "Panel, left half" });
  const repos = () => within(lower()).getByRole("tab", { name: "Repos" });
  const reposView = () => screen.getByRole("navigation", { name: "Repos" });
  /** ⌘J on a Mac, Ctrl+J elsewhere. */
  const panelKey = () =>
    fireEvent.keyDown(document.body, {
      key: "j",
      ...(onAMac() ? { metaKey: true } : { ctrlKey: true }),
    });

  it("opens under both sidebars and the editor, on Repos, its icon below the left bar's divider", async () => {
    core();
    render(<App />);

    const view = await screen.findByRole("tabpanel", { name: "Repos" });
    expect(screen.getByTestId("region-panel")).toContainElement(view);
    expect(repos()).toHaveAttribute("aria-selected", "true");
    expect(lower().closest('[data-scope="activity-bar"][data-part="root"]')).toContainElement(
      screen.getByRole("tablist", { name: "Navigation" }),
    );
    expect(await within(view).findByTestId("clone-svc")).toBeInTheDocument();
  });

  it("leaves Explorer with Files alone, and Repos and branches is the Repos view's", async () => {
    core();
    render(<App />);
    const explorer = await showTheExplorer();
    await within(reposView()).findByTestId("clone-svc");

    expect(
      within(explorer)
        .getAllByRole("heading")
        .map((one) => one.textContent),
    ).toEqual([expect.stringMatching(/^Files/)]);
    expect(within(explorer).queryByRole("tree", { name: "Repos and branches" })).toBeNull();
    expect(within(reposView()).getByRole("tree", { name: "Repos and branches" })).toBeVisible();
  });

  it("drives Explorer's Files from a branch picked in Repos, as before", async () => {
    const { asked } = core();
    render(<App />);
    await showTheExplorer();

    await userEvent.click(await within(reposView()).findByRole("treeitem", { name: /^one/ }));

    expect(
      await within(screen.getByRole("navigation", { name: "Explorer" })).findByRole("tree", {
        name: /^Files of one/,
      }),
    ).toBeInTheDocument();
    // And where the next chat starts, which is what a pick always was.
    await openAChat();
    expect(startedIn(asked)).toEqual([`${CUT}/one`]);
  });

  it("puts its half away on a press of the open Lower icon, the Panel with it, and back", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Repos" });
    const tree = reposView();

    await userEvent.click(repos());

    expect(screen.queryByRole("tabpanel", { name: "Repos" })).toBeNull();
    expect(lower()).toBeInTheDocument();
    // The right half takes the Panel's whole width (#1738, P-5), until it goes too.
    expect(screen.getByRole("tabpanel", { name: "Memory" })).toBeInTheDocument();
    expect(document.querySelector('[aria-controls="region-upper"]')).not.toHaveClass("edge-gone");
    const right = screen.getByRole("tablist", { name: "Panel, right half" });
    await userEvent.click(within(right).getByRole("tab", { name: "Memory" }));
    expect(document.querySelector('[aria-controls="region-upper"]')).toHaveClass("edge-gone");

    await userEvent.click(repos());

    expect(await screen.findByRole("tabpanel", { name: "Repos" })).toBeInTheDocument();
    // Hidden while it was away, never unmounted: the same tree came back, folds and all.
    expect(reposView()).toBe(tree);
  });

  it("is hidden and shown by ⌘J", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Repos" });

    panelKey();
    await waitFor(() => expect(screen.queryByRole("tabpanel", { name: "Repos" })).toBeNull());
    // The sidebars are not the Panel's.
    expect(screen.getByRole("tabpanel", { name: "Chats" })).toBeInTheDocument();

    panelKey();
    expect(await screen.findByRole("tabpanel", { name: "Repos" })).toBeInTheDocument();
  });

  it("is hidden and shown from the palette, its row saying which it does and its key", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Repos" });

    await userEvent.keyboard("{F2}");
    const palette = await screen.findByRole("dialog", { name: "Command palette" });
    await userEvent.type(within(palette).getByRole("combobox"), "the panel");
    await userEvent.click(await within(palette).findByRole("option", { name: /Hide the panel/ }));

    await waitFor(() => expect(screen.queryByRole("tabpanel", { name: "Repos" })).toBeNull());

    await userEvent.keyboard("{F2}");
    const again = await screen.findByRole("dialog", { name: "Command palette" });
    await userEvent.type(within(again).getByRole("combobox"), "the panel");
    await userEvent.click(await within(again).findByRole("option", { name: /Show the panel/ }));

    expect(await screen.findByRole("tabpanel", { name: "Repos" })).toBeInTheDocument();
  });

  it("comes back as it was left when the project is opened again", async () => {
    core();
    const { unmount } = render(<App />);
    await screen.findByRole("tabpanel", { name: "Repos" });
    await userEvent.click(repos());
    await waitFor(() => expect(screen.queryByRole("tabpanel", { name: "Repos" })).toBeNull());
    unmount();

    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });

    expect(screen.queryByRole("tabpanel", { name: "Repos" })).toBeNull();
    expect(repos()).toHaveAttribute("aria-selected", "false");
  });
});

/**
 * **The Panel's right half, in the window** (#1738, P-2, P-5, P-11, P-14): Memory, Todos and
 * Sessions, out of the right sidebar, switched by the right bar's Lower icons.
 */
describe("the Panel's right half", () => {
  const lower = () => screen.getByRole("tablist", { name: "Panel, right half" });
  const lowerTab = (name: string) => within(lower()).getByRole("tab", { name });
  const half = (side: string) =>
    screen
      .getByTestId("region-panel")
      .querySelector(`.panel-half[data-side="${side}"]`) as HTMLElement;

  it("opens on Memory beside Repos, its icons below the right bar's divider (P-14)", async () => {
    core();
    render(<App />);

    const memory = await screen.findByRole("tabpanel", { name: "Memory" });
    expect(half("right")).toContainElement(memory);
    expect(await within(memory).findByText("The deploy key rotates")).toBeInTheDocument();
    expect(within(half("left")).getByRole("tabpanel", { name: "Repos" })).toBeInTheDocument();
    expect(
      within(lower())
        .getAllByRole("tab")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual(["Memory", "Todos", "Sessions"]);
    expect(lower().closest('[data-scope="activity-bar"][data-part="root"]')).toContainElement(
      screen.getByRole("tablist", { name: "Attention" }),
    );
  });

  it("switches to Todos and Sessions on their Lower icons, the sidebar left as it is", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Memory" });

    await userEvent.click(lowerTab("Todos"));

    const todos = await screen.findByRole("tabpanel", { name: "Todos" });
    expect(half("right")).toContainElement(todos);
    expect(within(todos).getByTestId("panel-todos")).toBeInTheDocument();
    expect(screen.getByRole("tabpanel", { name: "Inbox" })).toBeInTheDocument();
  });

  it("heads the half with the view's title, its own +, and a chevron that puts it away (P-11)", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Memory" });
    const head = half("right").querySelector(".half-head") as HTMLElement;

    expect(within(head).getByRole("heading", { name: "Memory" })).toBeInTheDocument();
    expect(await within(head).findByRole("button", { name: /memory/i })).toBeInTheDocument();
    expect(within(half("right")).queryAllByRole("tab")).toEqual([]);

    await userEvent.click(
      within(head).getByRole("button", { name: "Put the panel's right half away" }),
    );

    expect(screen.queryByRole("tabpanel", { name: "Memory" })).toBeNull();
    expect(lowerTab("Memory")).toHaveAttribute("aria-selected", "false");
    // The left half has the Panel to itself (P-5).
    expect(half("right")).toHaveAttribute("hidden");
    expect(half("left")).not.toHaveAttribute("hidden");
    expect(screen.getByRole("tabpanel", { name: "Repos" })).toBeInTheDocument();
  });

  it("comes back as it was left when the project is opened again", async () => {
    core();
    const { unmount } = render(<App />);
    await screen.findByRole("tabpanel", { name: "Memory" });
    await userEvent.click(lowerTab("Sessions"));
    await screen.findByRole("tabpanel", { name: "Sessions" });
    unmount();

    core();
    render(<App />);

    expect(await screen.findByRole("tabpanel", { name: "Sessions" })).toBeInTheDocument();
  });
});

/**
 * **The right side's activity bar, in the window** (#1678, ADR 0038 as amended 2026-10-10): the
 * "for you" side's views, one at a time, and what a person presses and sees.
 */
/**
 * **A view carries one title** (#3330, as a view tab does since #3147): the region's head names
 * the view and holds its actions, and the view draws no heading of its own that says it again.
 * A guard over every view the window opens in a sidebar or a half of the Panel.
 */
describe("one title per view (#3330)", () => {
  const VIEWS = [
    "Chats",
    "Search",
    "Inbox",
    "Personas",
    "Vaults",
    "Memory",
    "Todos",
    "Sessions",
    "Repos",
    "Changes",
  ];
  const said = (element: Element) => (element.textContent ?? "").trim().toLowerCase();

  it.each(VIEWS)("draws %s with one title, its region's", async (name) => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });
    if (screen.queryByRole("tabpanel", { name }) === null)
      await userEvent.click(screen.getByRole("tab", { name }));
    const view = await screen.findByRole("tabpanel", { name });
    const region = view.closest(".slot-body, .panel-half") as HTMLElement;
    expect(region).not.toBeNull();

    // No section head of its own: its title and actions are the region's.
    expect(view.querySelector('[data-scope="panel-section"][data-part="head"]')).toBeNull();
    // And no heading in the region says the view's name twice.
    const named = within(region)
      .queryAllByRole("heading")
      .filter((heading) => said(heading) === name.toLowerCase());
    expect(named.length).toBeLessThanOrEqual(1);
  });
});

describe("the right side's activity bar", () => {
  const bar = () => screen.getByRole("tablist", { name: "Attention" });
  const tab = (name: string) => within(bar()).getByRole("tab", { name });
  /** A Lower icon of the right side: Memory, Todos and Sessions are in its half (#1738). */
  const lowerTab = (name: string) =>
    within(screen.getByRole("tablist", { name: "Panel, right half" })).getByRole("tab", { name });
  /** ⌥⌘B on a Mac, Ctrl+Alt+B elsewhere. */
  const toggleKey = () =>
    fireEvent.keyDown(document.body, {
      key: "b",
      code: "KeyB",
      altKey: true,
      ...(onAMac() ? { metaKey: true } : { ctrlKey: true }),
    });

  it("opens on the Inbox, and switches to Personas and Vaults one at a time (#1738)", async () => {
    core();
    render(<App />);

    const inbox = await screen.findByRole("tabpanel", { name: "Inbox" });
    expect(screen.getByTestId("region-right")).toContainElement(inbox);
    expect(tab("Inbox")).toHaveAttribute("aria-selected", "true");
    expect(
      within(bar())
        .getAllByRole("tab")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual(["Inbox", "Personas", "Vaults"]);

    await userEvent.click(tab("Personas"));

    const shown = await screen.findByRole("tabpanel", { name: "Personas" });
    expect(shown.querySelector('.panels[data-view="personas"]')).not.toBeNull();
    expect(screen.queryByRole("tabpanel", { name: "Inbox" })).toBeNull();
    // The left side is not touched by the right's bar, nor the Panel's right half.
    expect(screen.getByRole("tabpanel", { name: "Chats" })).toBeInTheDocument();
    expect(screen.getByRole("tabpanel", { name: "Memory" })).toBeInTheDocument();
  });

  it("puts the side away on a press of the open view, keeps the bar, and brings it back", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Inbox" });
    await userEvent.click(tab("Personas"));
    const personas = (await screen.findByRole("tabpanel", { name: "Personas" })).firstChild;

    await userEvent.click(tab("Personas"));

    expect(screen.queryByRole("tabpanel", { name: "Personas" })).toBeNull();
    expect(bar()).toBeInTheDocument();
    expect(away("Attention")).toBe(true);

    await userEvent.click(tab("Personas"));

    expect(await screen.findByRole("tabpanel", { name: "Personas" })).toBeInTheDocument();
    // Hidden while it was away, never unmounted.
    expect(screen.getByRole("tabpanel", { name: "Personas" }).firstChild).toBe(personas);
  });

  it("counts the open todos on the Todos Lower icon, while the half is away too", async () => {
    todos = [
      { slug: "a", title: "Review the rollout plan" },
      { slug: "b", title: "Rotate the deploy key" },
    ];
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Memory" });
    await waitFor(() => expect(lowerTab("Todos")).toHaveAccessibleDescription("2 open todos"));
    expect(lowerTab("Todos").querySelector('[data-part="count"]')).toHaveTextContent("2");
    // Plain, not the needs-you colours: a todo is yours to do, not a chat asking.
    expect(lowerTab("Todos").querySelector('[data-part="count"]')).not.toHaveAttribute("data-tone");

    await userEvent.click(lowerTab("Memory"));

    expect(screen.queryByRole("tabpanel", { name: "Memory" })).toBeNull();
    expect(lowerTab("Todos")).toHaveAccessibleDescription("2 open todos");
  });

  it("puts the Inbox first, counting the asks in the needs-you tone, while the side is away too", async () => {
    waitingAsks = [asking(3, "Run cargo test"), asking(4, "Run npm test")];
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Inbox" });
    await waitFor(() => expect(tab("Inbox")).toHaveAccessibleDescription("2 things wait on you"));
    expect(tab("Inbox").querySelector('[data-part="count"]')).toHaveAttribute(
      "data-tone",
      "needs-you",
    );

    await userEvent.click(tab("Inbox"));

    expect(screen.queryByRole("tabpanel", { name: "Inbox" })).toBeNull();
    expect(tab("Inbox")).toHaveAccessibleDescription("2 things wait on you");
  });

  it("opens the Inbox on ⌘⇧I, with the keyboard on its first ask and not on a button", async () => {
    waitingAsks = [asking(3, "Run cargo test")];
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Memory" });
    await waitFor(() => expect(tab("Inbox")).toHaveAccessibleDescription("1 thing waits on you"));

    fireEvent.keyDown(document.body, {
      key: "I",
      shiftKey: true,
      ...(onAMac() ? { metaKey: true } : { ctrlKey: true }),
    });

    const inbox = await screen.findByRole("tabpanel", { name: "Inbox" });
    const first = within(inbox).getByRole("listitem");
    await waitFor(() => expect(first).toHaveFocus());
    expect(within(first).getByText("Run cargo test")).toBeInTheDocument();
  });

  it("opens the Inbox from the title bar's hand", async () => {
    waitingAsks = [asking(3, "Run cargo test")];
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Memory" });
    const hand = await screen.findByRole("button", { name: "1 thing waits on you" });

    await userEvent.click(hand);

    const inbox = await screen.findByRole("tabpanel", { name: "Inbox" });
    expect(within(inbox).getByRole("region", { name: "steward 3" })).toBeInTheDocument();
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("tells the core while the Inbox is open, so no notification is sent about it (#1694)", async () => {
    waitingAsks = [asking(3, "Run cargo test")];
    const { asked } = core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Inbox" });
    const told = () => asked.filter((one) => one.cmd === "inbox_shown").map((one) => one.args.open);
    // The side opens on the Inbox since #1738.
    await waitFor(() => expect(told().at(-1)).toBe(true));
    expect(asked.find((one) => one.cmd === "inbox_shown")?.args.plane).toBe(PLANE);

    await userEvent.click(tab("Personas"));

    await waitFor(() => expect(told().at(-1)).toBe(false));

    await userEvent.click(tab("Inbox"));

    await waitFor(() => expect(told().at(-1)).toBe(true));
  });

  it("lands a clicked notification on the Inbox at its chat's group (#1694)", async () => {
    mockEvents = true;
    waitingAsks = [asking(3, "Run cargo test"), asking(4, "Run npm test")];
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Memory" });
    await waitFor(() => expect(tab("Inbox")).toHaveAccessibleDescription("2 things wait on you"));
    // Sent while the window was behind another app; the click brings it to the front.
    const behind = vi.spyOn(document, "hasFocus").mockReturnValue(false);
    await emit(ASKS_NOTIFIED, { plane: PLANE, session: 4 });
    behind.mockRestore();
    fireEvent.focus(window);

    const inbox = await screen.findByRole("tabpanel", { name: "Inbox" });
    const group = within(inbox).getByRole("region", { name: "steward 4" });
    await waitFor(() => expect(within(group).getByRole("listitem")).toHaveFocus());
  });

  it("lands a notification about several chats on the first still waiting (#1989)", async () => {
    mockEvents = true;
    // Chat 2 was answered since: the first of the notification's chats still waiting is 4.
    waitingAsks = [asking(3, "Run cargo test"), asking(4, "Run npm test")];
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Memory" });
    await waitFor(() => expect(tab("Inbox")).toHaveAccessibleDescription("2 things wait on you"));
    const behind = vi.spyOn(document, "hasFocus").mockReturnValue(false);
    await emit(ASKS_NOTIFIED, { plane: PLANE, session: 2, sessions: [2, 4, 3] });
    behind.mockRestore();
    fireEvent.focus(window);

    const inbox = await screen.findByRole("tabpanel", { name: "Inbox" });
    const group = within(inbox).getByRole("region", { name: "steward 4" });
    await waitFor(() => expect(within(group).getByRole("listitem")).toHaveFocus());
  });

  it("draws no count when nothing is left to do", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Memory" });

    expect(lowerTab("Todos").querySelector('[data-part="count"]')).toBeNull();
  });

  it("answers ⌥⌘B: the side away, and back on the view it had", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Inbox" });
    await userEvent.click(tab("Vaults"));

    toggleKey();
    await waitFor(() => expect(screen.queryByRole("tabpanel", { name: "Vaults" })).toBeNull());
    // The left side stays, and the Panel's right half is not the sidebar's.
    expect(screen.getByRole("tabpanel", { name: "Chats" })).toBeInTheDocument();
    expect(screen.getByRole("tabpanel", { name: "Memory" })).toBeInTheDocument();

    toggleKey();
    expect(await screen.findByRole("tabpanel", { name: "Vaults" })).toBeInTheDocument();
  });

  it("gives an approved extension's panel a view of its own, after purlis's", async () => {
    extensionPanels = [
      {
        key: "ext/stats/burn",
        title: "Burn rate",
        order: 12,
        mark: "dot",
        from: "stats",
        about: null,
        blocks: [{ kind: "note", text: "Forty tokens a minute", tone: "plain" }],
      },
    ];
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Inbox" });

    const burn = await within(bar()).findByRole("tab", { name: "Burn rate" });
    expect(within(bar()).getAllByRole("tab").at(-1)).toBe(burn);
    // Not stacked into Memory: the extension never crowds purlis's views.
    expect(
      within(screen.getByRole("tabpanel", { name: "Memory" })).queryByText("Forty tokens a minute"),
    ).toBeNull();

    await userEvent.click(burn);

    expect(
      within(await screen.findByRole("tabpanel", { name: "Burn rate" })).getByText(
        "Forty tokens a minute",
      ),
    ).toBeInTheDocument();
  });

  it("comes back on the view it was left on when the project is opened again", async () => {
    core();
    const { unmount } = render(<App />);
    await screen.findByRole("tabpanel", { name: "Inbox" });
    await userEvent.click(tab("Personas"));
    await screen.findByRole("tabpanel", { name: "Personas" });
    unmount();

    core();
    render(<App />);

    expect(await screen.findByRole("tabpanel", { name: "Personas" })).toBeInTheDocument();
  });
});

/**
 * **Moving a view between its sidebar and its half of the Panel** (#1740, P-3, P-4): "Move to
 * panel" and "Move to sidebar" on the view's icon, its own side only; the Inbox has neither.
 */
describe("moving a view", () => {
  const bar = (name: string) => screen.getByRole("tablist", { name });
  const icon = (name: string, view: string) => within(bar(name)).getByRole("tab", { name: view });
  /** The menu on a view's icon, opened as a right-click opens it. */
  const menuOf = async (tab: HTMLElement) => {
    fireEvent.contextMenu(tab);
    return within(await screen.findByRole("menu"));
  };

  it("offers Move to panel on a sidebar's icon, and nothing that crosses to the other side", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });

    const menu = await menuOf(icon("Navigation", "Search"));

    expect(menu.getAllByRole("menuitem").map((row) => row.getAttribute("aria-label"))).toEqual([
      "Move to panel",
    ]);
  });

  it("moves Search to the Panel's left half, open there, its icon among the Lower icons", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });

    await userEvent.click((await menuOf(icon("Navigation", "Search"))).getByRole("menuitem"));

    const search = await within(bar("Panel, left half")).findByRole("tab", { name: "Search" });
    expect(search).toHaveAttribute("aria-selected", "true");
    expect(within(bar("Navigation")).queryByRole("tab", { name: "Search" })).toBeNull();
    expect(screen.getByTestId("region-panel")).toContainElement(
      screen.getByRole("tabpanel", { name: "Search" }),
    );
    // After the half's own views.
    expect(
      within(bar("Panel, left half"))
        .getAllByRole("tab")
        .map((one) => one.ariaLabel),
    ).toEqual(["Repos", "Changes", "Search"]);
  });

  it("keeps what was typed in Search when it moves to the Panel, the same view, not a new one (#1752)", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });
    await userEvent.click(icon("Navigation", "Search"));
    const view = await screen.findByRole("tabpanel", { name: "Search" });
    const box = within(view).getByRole("searchbox");
    // `change` rather than typed keys: jsdom lays nothing out (see "keeps what was typed").
    fireEvent.change(box, { target: { value: "needle" } });

    await userEvent.click(
      (await menuOf(icon("Navigation", "Search"))).getByRole("menuitem", { name: "Move to panel" }),
    );

    await within(bar("Panel, left half")).findByRole("tab", { name: "Search" });
    const moved = screen.getByRole("tabpanel", { name: "Search" });
    expect(screen.getByTestId("region-panel")).toContainElement(moved);
    // The very elements it had: moved, not drawn again.
    expect(moved).toBe(view);
    expect(within(moved).getByRole("searchbox")).toBe(box);
    expect(box).toHaveValue("needle");
  });

  it("moves a half's view up with Move to sidebar, and back down again", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Memory" });

    const up = await menuOf(icon("Panel, right half", "Memory"));
    await userEvent.click(up.getByRole("menuitem", { name: "Move to sidebar" }));

    const memory = await within(bar("Attention")).findByRole("tab", { name: "Memory" });
    expect(memory).toHaveAttribute("aria-selected", "true");
    expect(within(bar("Panel, right half")).queryByRole("tab", { name: "Memory" })).toBeNull();

    const down = await menuOf(memory);
    await userEvent.click(down.getByRole("menuitem", { name: "Move to panel" }));

    expect(
      await within(bar("Panel, right half")).findByRole("tab", { name: "Memory" }),
    ).toBeInTheDocument();
    expect(within(bar("Attention")).queryByRole("tab", { name: "Memory" })).toBeNull();
  });

  it("offers no move on the Inbox's icon, pinned to the right sidebar", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Inbox" });

    fireEvent.contextMenu(icon("Attention", "Inbox"));

    expect(screen.queryByRole("menu")).toBeNull();
    expect(screen.queryByRole("menuitem", { name: /^Move to/ })).toBeNull();
  });

  it("opens the menu from the keyboard too, on the icon that has it", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });
    icon("Navigation", "Explorer").focus();

    await userEvent.keyboard("{Shift>}{F10}{/Shift}");

    expect(
      within(await screen.findByRole("menu")).getByRole("menuitem", { name: "Move to panel" }),
    ).toBeInTheDocument();
  });

  it("moves an extension's panel to the right half, as any view of its side", async () => {
    extensionPanels = [
      {
        key: "ext/stats/burn",
        title: "Burn rate",
        order: 12,
        mark: "dot",
        from: "stats",
        about: null,
        blocks: [{ kind: "note", text: "Forty tokens a minute", tone: "plain" }],
      },
    ];
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Inbox" });
    const burn = await within(bar("Attention")).findByRole("tab", { name: "Burn rate" });

    await userEvent.click((await menuOf(burn)).getByRole("menuitem", { name: "Move to panel" }));

    const there = await within(bar("Panel, right half")).findByRole("tab", { name: "Burn rate" });
    expect(there).toHaveAttribute("aria-selected", "true");
    expect(within(bar("Attention")).queryByRole("tab", { name: "Burn rate" })).toBeNull();
    expect(
      within(screen.getByRole("tabpanel", { name: "Burn rate" })).getByText(
        "Forty tokens a minute",
      ),
    ).toBeInTheDocument();
  });

  it("is where the view is when the project is opened again", async () => {
    core();
    const { unmount } = render(<App />);
    await screen.findByRole("tabpanel", { name: "Repos" });
    await userEvent.click((await menuOf(icon("Panel, left half", "Repos"))).getByRole("menuitem"));
    await within(bar("Navigation")).findByRole("tab", { name: "Repos" });
    unmount();

    core();
    render(<App />);

    // Open in the left sidebar, where it was moved to, and not in the Panel.
    const view = await screen.findByRole("tabpanel", { name: "Repos" });
    expect(screen.getByTestId("region-panel")).not.toContainElement(view);
    expect(within(bar("Navigation")).getByRole("tab", { name: "Repos" })).toBeInTheDocument();
    expect(within(bar("Panel, left half")).queryByRole("tab", { name: "Repos" })).toBeNull();
  });
});

/**
 * **The arrangement, against the whole window** (`regions.ts`).
 *
 * The four regions used to be the shape of `PlaneView`'s JSX. They are a stored document now,
 * and these are the assertions that say the window really reads it — a layout that is only data
 * in one module is not configurable.
 */
describe("the window the stored arrangement asks for", () => {
  /** What the window reads before it renders: the layout file, handed to the page as it is
   *  created (`windowprefs.ts`). */
  const arrange = (regions: unknown) => {
    (globalThis as Record<string, unknown>)[GLOBAL] = {
      layout: {
        path: "layout.json",
        found: true,
        document: { version: 1, regions },
        trouble: null,
      },
      theme: { path: "theme.json", found: false, document: null, trouble: null },
    };
  };
  afterEach(() => {
    Reflect.deleteProperty(globalThis, GLOBAL);
  });

  const inSlot = (side: string) => within(screen.getByTestId(`region-${side}`));

  it("draws a region on the side the document names, with no JSX moved", async () => {
    arrange([
      { id: "explorer", side: "right", order: 0, collapsed: false },
      { id: "aside", side: "left", order: 0, collapsed: false },
    ]);
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Inbox" });

    expect(inSlot("left").getByRole("tabpanel", { name: "Inbox" })).toBeInTheDocument();
    expect(inSlot("right").getByTestId("chats-section")).toBeInTheDocument();
  });

  it("stacks two regions in one slot in the order the document gives them", async () => {
    arrange([
      { id: "explorer", side: "left", order: 0, collapsed: false },
      { id: "aside", side: "left", order: -1, collapsed: false },
    ]);
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Inbox" });

    const drawn = inSlot("left").getAllByRole("tabpanel");
    expect(drawn.map((one) => one.dataset.view)).toEqual(["inbox", "chats"]);
  });

  it("draws a file that still places the bottom region, without it and without a word", async () => {
    // Every file before #1676 placed it. What it drew is the Changes view now.
    arrange([
      { id: "explorer", side: "left", order: 0, collapsed: false },
      { id: "aside", side: "right", order: 0, collapsed: false },
      { id: "bottom", side: "bottom", order: 0, collapsed: true },
    ]);
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });

    expect(screen.queryByTestId("region-bottom")).toBeNull();
    expect(screen.getAllByLabelText("Repository state")).toHaveLength(1);
  });

  /** The right half holding only an extension's panel, and the left half away: the Panel is
   *  on screen only while that extension contributes its panel this launch. */
  const onlyBurnInThePanel = () =>
    arrange([
      {
        id: "aside",
        side: "right",
        order: 0,
        collapsed: false,
        moved: ["memory", "todos", "sessions"],
      },
      { id: "panel-left", side: "left", order: 0, collapsed: true },
      {
        id: "panel-right",
        side: "right",
        order: 0,
        collapsed: false,
        moved: ["panel:ext/stats/burn"],
      },
    ]);
  const panelKey = () =>
    fireEvent.keyDown(document.body, {
      key: "j",
      ...(onAMac() ? { metaKey: true } : { ctrlKey: true }),
    });

  it("brings the Panel out on the first ⌘J where its only half left holds a panel no extension contributes now (#1752)", async () => {
    // That half is not drawn, so nothing of the Panel is on screen: ⌘J shows it rather than
    // hiding what nobody sees.
    onlyBurnInThePanel();
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Inbox" });
    expect(screen.queryByRole("tabpanel", { name: "Repos" })).toBeNull();

    panelKey();

    expect(await screen.findByRole("tabpanel", { name: "Repos" })).toBeInTheDocument();
  });

  it("hides the Panel on the first ⌘J while that panel is contributed and drawn (#1752)", async () => {
    extensionPanels = [
      {
        key: "ext/stats/burn",
        title: "Burn rate",
        order: 12,
        mark: "dot",
        from: "stats",
        about: null,
        blocks: [{ kind: "note", text: "Forty tokens a minute", tone: "plain" }],
      },
    ];
    onlyBurnInThePanel();
    core();
    render(<App />);
    expect(await screen.findByRole("tabpanel", { name: "Burn rate" })).toBeInTheDocument();

    panelKey();

    await waitFor(() => expect(screen.queryByRole("tabpanel", { name: "Burn rate" })).toBeNull());
    expect(screen.queryByRole("tabpanel", { name: "Repos" })).toBeNull();
  });

  it("launches with a region away, and its slot is still in the group", async () => {
    // A slot that is put away is collapsed and never removed — charter-app#141's throw. What
    // the operator loses is the content, which is unmounted so it is out of the tab order too.
    arrange([
      { id: "explorer", side: "left", order: 0, collapsed: true },
      { id: "aside", side: "right", order: 0, collapsed: false },
    ]);
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Memory" });

    // Its views are hidden, and still mounted (#1673): collapsing never unmounts a view.
    expect(screen.queryByRole("navigation", { name: "Explorer" })).not.toBeInTheDocument();
    expect(screen.queryByRole("tabpanel", { name: /Chats|Explorer/ })).not.toBeInTheDocument();
    expect(screen.getByTestId("explorer")).toBeInTheDocument();
    expect(screen.getByTestId("region-left")).toBeInTheDocument();
    expect(away("Navigation")).toBe(true);
  });

  it("writes what the operator did back where the next launch will read it", async () => {
    const { asked } = core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Chats" });

    sideKey("left");

    await vi.waitFor(() => expect(asked.some((one) => one.cmd === "write_layout")).toBe(true));
    const written = asked.filter((one) => one.cmd === "write_layout").at(-1);
    const arrangement = [
      { id: "navigation", side: "left", order: 0, collapsed: true },
      { id: "aside", side: "right", order: 0, collapsed: false },
      // The Panel's halves (#1737), open as the window drew them.
      { id: "panel-left", side: "left", order: 0, collapsed: false },
      { id: "panel-right", side: "right", order: 0, collapsed: false },
    ];
    expect(JSON.parse(String(written?.args.text))).toEqual({
      version: 2,
      // The machine's, which a project with none of its own starts from, and the project's own
      // (#1673).
      regions: arrangement,
      projects: { [PLANE]: { regions: arrangement } },
      // The machine's text sizes share the file (charter-app#283), written as they stand.
      text: { window: 14, terminal: 13 },
    });
  });
});

/**
 * **The status line**, against the whole window, because every question about it is a question
 * about where it is relative to everything else (`StatusLine.tsx` for what is on it and why).
 *
 * The operator asked for the project's directory to move out of the top-right corner into a
 * one-line bar at the very bottom. These are the assertions that it went there and that it is
 * not a fifth region; the component's own rules are in `StatusLine.test.tsx`.
 */
describe("the status line", () => {
  it("sits below every region, and outside the group that draws them", async () => {
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Memory" });

    const line = screen.getByTestId("status-line");
    const frame = document.querySelector(".region-frame");
    if (frame === null) throw new Error("no region frame");
    // Under the regions, not beside them.
    expect(frame.compareDocumentPosition(line) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    // And not inside the panel group at all: it is the window's chrome, the way the project
    // strip above the regions is, and `StatusLine.tsx` argues why it is not in the arrangement.
    expect(document.querySelector(".regions")?.contains(line)).toBe(false);
  });

  it("carries the project, by name with its directory behind it, and not in the corner of the bar", async () => {
    core();
    render(<App />);
    await screen.findByTestId("status-line");

    // Its name on the line and its whole directory one hover or Tab away (qw357).
    const project = within(screen.getByTestId("status-line")).getByTestId("status-project");
    expect(project).toHaveTextContent(PLANE.split("/").at(-1) as string);
    expect(project).toHaveAttribute("data-directory", PLANE);
    // The bar is the editor's chat strip, its show-more and its `+`, and nothing else.
    expect(document.querySelector("header.bar .status-project")).toBeNull();
  });

  it("draws the chat strip at the top of the editor, and the workspaces in the title bar", async () => {
    // DS-6 (#1836, ADR 0036 as amended 2026-10-12): only the session strip moved. Where it sits
    // on screen is `centre-tabs.e2e.ts`'s; this is the tree that puts it there.
    core();
    render(<App />);
    const editor = await screen.findByRole("region", { name: "Editor" });
    const chats = await waitFor(() => {
      const strip = document.querySelector<HTMLElement>('[data-strip="Tabs"]');
      expect(strip).not.toBeNull();
      return strip as HTMLElement;
    });

    expect(editor).toContainElement(chats);
    // Above the panes and outside the pane menu's trigger, which is the panes' own box.
    const bar = chats.closest("header.bar");
    const panes = editor.querySelector(".panes");
    expect(bar?.parentElement).toBe(panes?.parentElement);
    expect((bar as Element).compareDocumentPosition(panes as Element)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
    // The workspaces are the title bar's (#2916), in no region.
    const workspaces = screen.getByRole("toolbar", { name: "Pinned workspaces" });
    expect(workspaces.closest('[data-testid="title-bar"]')).not.toBeNull();
    expect(workspaces.closest('[role="region"]')).toBeNull();
  });

  it("says which workspace the window is on, and follows the focus", async () => {
    core();
    render(<App />);
    await screen.findByTestId("clone-svc");

    const line = screen.getByTestId("status-line");
    expect(line).toHaveTextContent("alpha");

    await focus("beta");

    await waitFor(() => expect(line).toHaveTextContent("beta"));
  });

  it("counts what the workspace holds, off the answers the regions already asked for", async () => {
    // No second `workspace_panels` and no second `worktree_list`: the line reads the state the
    // three regions share (`workspacePanels.ts`), which is what keeps `git status` per clone
    // from running twice for one focused workspace.
    const { asked } = core();
    render(<App />);
    await screen.findByTestId("clone-svc");

    // The mock's `alpha` holds one clone with two worktrees cut off it, on a plane of two
    // workspaces.
    const line = screen.getByTestId("status-line");
    await waitFor(() => expect(line).toHaveTextContent(/branches\s*2/));
    expect(line).toHaveTextContent(/ws\s*2/);

    const listings = asked.filter(
      (one) => one.cmd === "worktree_list" && one.args.workspace === "alpha",
    );
    expect(listings).toHaveLength(1);
  });

  it("cannot be put away, because it is not a region", async () => {
    // Every side has a way to put it away. The status line has none — it is the frame, and a
    // status line an operator can lose is one they will lose.
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Memory" });

    sideKey("left");
    sideKey("right");
    await waitFor(() => expect(away("Navigation") && away("Attention")).toBe(true));

    expect(screen.queryByRole("button", { name: "Status" })).toBeNull();
    expect(screen.getByTestId("status-line")).toBeInTheDocument();
  });

  it("lists the project's alerts as Notices in the Inbox, counted on the status line's button", async () => {
    // The window asks about every project at once (#1695): the command names no plane, so
    // nothing can wire it to the one in front by mistake; each Inbox lists its own.
    alerts = [
      {
        plane: PLANE,
        alerts: [
          {
            severity: "warn",
            subject: "reinit",
            detail: "1 workspace is behind the current layout: beta",
            way: { kind: "fix", id: "workspace-reinit" },
          },
        ],
        stopped: null,
      },
    ];
    const { asked } = core();
    render(<App />);

    const button = await screen.findByRole("button", { name: "Notices: 1" });
    await userEvent.click(button);

    const inbox = await screen.findByRole("tabpanel", { name: "Inbox" });
    const notices = within(inbox).getByRole("region", { name: "Notices" });
    expect(notices).toHaveTextContent("1 workspace is behind the current layout: beta");
    expect(within(notices).getByRole("button", { name: "Update workspace layout" })).toBeVisible();
    for (const one of asked.filter((a) => a.cmd === "alerts_everywhere"))
      expect(one.args).toEqual({});
    // No drawer over the window any more.
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("drops the count when purlis stopped looking in a project, and draws no zero", async () => {
    alerts = [{ plane: PLANE, alerts: [], stopped: "charter.toml is not valid TOML" }];
    core();
    render(<App />);

    const button = await screen.findByRole("button", { name: "Notices: not counted" });
    expect(button).toBeEnabled();
    expect(button).not.toHaveTextContent("0");
  });

  it("draws no alerts section in the right-hand region any more", async () => {
    core();
    render(<App />);
    await screen.findByRole("button", { name: "Notices: none" });

    expect(screen.queryByTestId("panel-alerts")).toBeNull();
  });
});

describe("a write the panels draw, and the window around them (#1160)", () => {
  const settle = () => new Promise((done) => setTimeout(done, 50));
  /** What the core says a memory or a session record of alpha concerns (`planechange.rs`). */
  const alphaChanged = () =>
    emit("plane-changed", {
      plane: PLANE,
      changes: [],
      answers: [{ answer: "panels", workspace: "alpha" }, { answer: "views" }],
    });

  it("draws a memory saved in the workspace in front without drawing the window again", async () => {
    mockEvents = true;
    const { asked } = core();
    render(<App />);
    const memory = await screen.findByRole("tabpanel", { name: "Memory" });
    await within(memory).findByText("The deploy key rotates");
    await settle();
    expect(planeViewRenders.count).toBeGreaterThan(0);
    const before = planeViewRenders.count;
    const asks = asked.length;

    memories = [...memories, { slug: "m2", title: "The train leaves at noon" }];
    await alphaChanged();

    expect(await within(memory).findByText("The train leaves at noon")).toBeInTheDocument();
    await settle();
    // Before #1160 it was 4: drawn once for the count and once for the answer, each twice.
    expect(planeViewRenders.count - before).toBe(0);
    // And no git reader: the plane read alone.
    expect(asked.slice(asks).map((one) => one.cmd)).not.toContain("workspace_repos");
    expect(asked.slice(asks).map((one) => one.cmd)).not.toContain("worktree_list");
  });

  it("draws a session record without drawing the window again", async () => {
    mockEvents = true;
    core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Memory" });
    await userEvent.click(within(lowerRight()).getByRole("tab", { name: "Sessions" }));
    const sessions = await screen.findByRole("tabpanel", { name: "Sessions" });
    await settle();
    const before = planeViewRenders.count;

    records = [{ slug: "2026-10-10-1650-ship", title: "Shipped the train" }];
    await alphaChanged();

    expect(await within(sessions).findByText("Shipped the train")).toBeInTheDocument();
    await settle();
    // Before #1160 it was twice: once for the count, once for the answer. Then once, for the
    // palette's `session.open` rows; those are built beside the panels now (`sessionOffers`).
    expect(planeViewRenders.count - before).toBe(0);
    // And the new record's rows reach the palette all the same.
    await userEvent.keyboard("{F2}");
    const palette = await screen.findByRole("dialog", { name: "Command palette" });
    await userEvent.type(within(palette).getByRole("combobox"), "Shipped");
    expect(
      within(palette)
        .getAllByRole("option")
        .map((row) => row.querySelector(".palette-title")?.textContent),
    ).toEqual(
      expect.arrayContaining([
        "Open session record: Shipped the train",
        "Resume session: Shipped the train",
      ]),
    );
  });

  it("does not draw the window again for a persona's memory, which only the panels list", async () => {
    mockEvents = true;
    core();
    render(<App />);
    const memory = await screen.findByRole("tabpanel", { name: "Memory" });
    await within(memory).findByText("The deploy key rotates");
    await settle();
    const before = planeViewRenders.count;

    // A memory of a persona: every workspace's panels and the root's (`planechange.rs`), and an
    // answer here that says what it said before.
    await emit("plane-changed", {
      plane: PLANE,
      changes: [],
      answers: [{ answer: "panels", workspace: null }, { answer: "views" }],
    });
    await settle();
    await settle();
    expect(planeViewRenders.count - before).toBe(0);
  });
});

const lowerRight = () => screen.getByRole("tablist", { name: "Panel, right half" });
