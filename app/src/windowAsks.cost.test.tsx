// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";
import { aWindow, rootOf, settled, type Asked } from "./projectSwitch.testkit";
import { SETTLE_MS as ASKS_SETTLE_MS } from "./asks";
import { AFTER_GIT_MOVED_MS } from "./afterGitMoved";
import { AFTER_A_MOVE_MS } from "./ChatGauge";
import { keepTheirOrder } from "./commandOrder";
import { panes } from "./countedPane.testkit";
import { countReactsCommits } from "./reactCommits.testkit";
import { stripNamed } from "./test-strips";

keepTheirOrder();

/**
 * **What the window listens for, and what it asks the core for the things that happen to it
 * most** (FS-G1b, #7895, epic #7889): the Tauri listeners it holds, by event name, and the
 * commands it sends for a focus, for each kind of `plane-changed`, for a turn ending off screen
 * and on screen, for a pane mounted and for a chat started.
 *
 * The slow log of 2026-11-27 found the trivial per-chat reads all inside stalls, made where a
 * pane mounts and some fifteen notices each make a `listen` and a read of their own, and each
 * `plane-changed` counted some forty times before the filters ran. These are the counts the
 * W-tickets lower (W1, #7909: one listener per event per window): each command by its name, a
 * listen and an unlisten by their event's, never a total, so one ask fewer is seen to be.
 *
 * At G1a's fixed load (`windowDraws.cost.test.tsx`), one project of 50 chats over two
 * workspaces, each chat in a tab of its own holding one pane, and the same counts at 10 chats:
 * what the window holds and asks is per window, never per chat. No beat runs and the window's
 * clock is held, so nothing but the one thing measured asks. Held by counts, the same on every
 * machine (ADR 0086).
 *
 * **A count that goes down is a W-ticket's win**: lower the table here in the same pull request,
 * and say which ask went and why. One that goes up is a regression.
 */

countReactsCommits();

vi.mock("./SessionPane", async () => ({
  SessionPane: (await import("./countedPane.testkit")).CountedPane,
}));

// The window's beats do not run, and none has passed: a beat landing inside a measured moment
// asks what it reads.
vi.mock("./whileShown", async (actual) => ({
  ...(await actual<typeof import("./whileShown")>()),
  everyWhileShown: () => () => {},
  aBeatSince: () => false,
}));

// The window's clocks are held still, so no read is caught up for the time a slow runner took.
beforeEach(() => {
  const now = performance.now();
  vi.spyOn(performance, "now").mockReturnValue(now);
  const today = Date.now();
  vi.spyOn(Date, "now").mockReturnValue(today);
});

afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

/** Each kind of `plane-changed` the core sends, by its answer; `null` is a change the core
 *  could not place, which concerns every reader. `panels` is every workspace's. */
const KINDS = [
  "sidebar",
  "panels",
  "rootPanels",
  "instructions",
  "curations",
  "settings",
  "git",
  "views",
  null,
] as const;

const kindName = (kind: (typeof KINDS)[number]) => kind ?? "not known";

/** How many times each command was sent, by name; a listen or an unlisten by its event's. */
type Asks = Record<string, number>;

function asksIn(asked: readonly Asked[]): Asks {
  const out: Asks = {};
  for (const one of asked) {
    const name = one.event === undefined ? one.cmd : `${one.cmd} ${one.event}`;
    out[name] = (out[name] ?? 0) + 1;
  }
  return out;
}

/**
 * **The events a window asked the core to listen for more than once**, by how many times: none,
 * since the window holds one listener per event however many of its parts listen (`here.ts`, W1,
 * #7909).
 */
const listenedMoreThanOnce = (listens: Asks): Asks =>
  Object.fromEntries(Object.entries(listens).filter(([, times]) => times !== 1));

/** What a window holds and asks, each by its name. */
type Measured = {
  /** How many times it asked the core to listen, by event, as it loaded. */
  listensOnLoad: Asks;
  /** The Tauri listeners it holds, by event, once it has settled. */
  listeners: Asks;
  /** What it asks as it is focused again. */
  focus: Asks;
  /** What it asks for each kind of `plane-changed`, by the kind's name. */
  planeChanged: Record<string, Asks>;
  /** What it asks as a chat's turn ends in a workspace not in front: a background chat's. */
  turnEndOffScreen: Asks;
  /** What it asks as the turn of the chat in its pane ends: the chat on screen. */
  turnEndOnScreen: Asks;
  /** What it asks as its pane is given another chat. */
  paneMount: Asks;
  /** What it asks as the person starts a chat. */
  chatStart: Asks;
};

/**
 * **What the window holds and asks now: the baseline** (2026-11-28, on main at 15eab6a312; the
 * listeners, a pane given a chat and a chat started lowered by W1, #7909).
 *
 * - **Listeners:** 75, one for each of 75 events: the window's one listener per event
 *   (`here.ts`, W1). Before it, 219: `plane-changed` was held 24 times, `chat-events` 7,
 *   `asks-changed` 5, and two events once for each chat open and once more (51 each at 50
 *   chats).
 * - **A focus:** 16 asks, among them the asks, the saving rows, the alerts and the sandbox's
 *   standing, `sandbox_state` four times.
 * - **`plane-changed`:** each kind reads what holds its answer. `settings` costs 19 asks
 *   (`sandbox_state` four times, `extensions_on` twice); `git` 5, the last 4.5 s after it; a
 *   change the core could not place 32, every reader's. Nothing on screen holds `rootPanels`.
 * - **A turn's end:** off screen, a chat of the workspace not in front, 4 asks: the finishes once
 *   (`finishRows.ts`), the asks, the signings and the window's word to the others. On screen, the
 *   chat in the pane, 6: the same four and the gauge twice, at once and a moment later
 *   (`ChatGauge.tsx`).
 * - **A pane given another chat** (a press on its tab): 13 asks. The terminal is kept and the
 *   chat's notices are mounted anew, by the chat (`ChatNotices`, #1486): the reads of the
 *   notices' own, the slow log's trivial per-chat reads (`chat_hibernated` twice), and the gauge
 *   twice. Before W1, 33: each notice's ten listens and ten unlistens too, which the window's
 *   one listener per event keeps through the notices' remount.
 * - **A chat started** from a split: 40 asks, the picker's reads, the start, and two panes'
 *   notices, the new chat's and the first chat's, which the split mounts again beside it. Before
 *   W1, 70: the 30 more were the notices' 20 listens and 10 unlistens.
 */
const BASELINE: Omit<Measured, "listensOnLoad"> = {
  listeners: {
    "artifact-pins-moved": 1,
    "asks-changed": 1,
    "asks-notified": 1,
    "branch-changed": 1,
    "browser-ask-ended": 1,
    "chat-browsers-ended": 1,
    "chat-browsers-listed": 1,
    "chat-events": 1,
    "chat-hibernation-changed": 1,
    "chat-input-refused": 1,
    "chat-lost-with-the-host": 1,
    "chat-mcp-failed": 1,
    "chat-move": 1,
    "chat-project-settings-changed": 1,
    "chat-restored": 1,
    "chat-sandbox-blocked": 1,
    "chat-stop": 1,
    "chat-vault-refused": 1,
    "chats-reconnecting": 1,
    "cluster-pool-added": 1,
    "dispatch-arrival": 1,
    "dispatch-away-changed": 1,
    "dispatch-grant-needed": 1,
    "editor-tab-opened": 1,
    "engine-download": 1,
    "engine-sandbox-needed": 1,
    "extension-heard": 1,
    "files-changed": 1,
    "files-unwatched": 1,
    "forge-sign-in-ended": 1,
    "forge-sign-in-trouble": 1,
    "fresh-runners-kept": 1,
    "handoff-arrived": 1,
    "harness-by-hand": 1,
    "kill-switch": 1,
    "layout-moved": 1,
    "layout-reported": 1,
    "note-cleared": 1,
    "notes-asked": 1,
    "notes-handed": 1,
    "open-plane": 1,
    "plane-changed": 1,
    "plane-closed": 1,
    "pool-mints-without-asking": 1,
    "pool-pin-changed": 1,
    "pool-setup-ready": 1,
    "pool-sign-in-confirm": 1,
    "prerequisites-changed": 1,
    "press-here": 1,
    "project-ready": 1,
    "projects-arrived": 1,
    "quiet-changed": 1,
    "quit-asked": 1,
    "report-drafts-changed": 1,
    "run-offer": 1,
    "sandbox-policy-changed": 1,
    "session-bus://answers": 1,
    "session-host-failing": 1,
    "session-host-not-in-the-background": 1,
    "settings-asked": 1,
    "smart-close": 1,
    "task-quiet": 1,
    "theme-drafts-close-handed": 1,
    "theme-folders-changed": 1,
    "tls-pool-added": 1,
    "unsaved-claim": 1,
    "unsaved-mirror": 1,
    "unsaved-moving": 1,
    "update://checked": 1,
    "update://failed": 1,
    "update://installed": 1,
    "vaults-to-move": 1,
    "window-hello": 1,
    "window-said": 1,
    "windows-changed": 1,
  },
  focus: {
    alerts_everywhere: 1,
    asks_waiting: 1,
    chats_level_why: 1,
    chats_on_older_sandbox: 1,
    clones_to_vouch: 1,
    draw_native_window: 1,
    harness_cards: 1,
    plane_fetch: 1,
    plane_saving: 1,
    project_policy_state: 1,
    sandbox_state: 4,
    stale_clones: 1,
    workspace_saving: 1,
  },
  planeChanged: {
    sidebar: {
      dispatch_arrival: 1,
      finished_tasks: 1,
      persona_marks: 1,
      plane_sidebar: 1,
    },
    panels: {
      workspace_panels: 1,
    },
    // Nothing reads `rootPanels` at this load: its reader follows it only while the plane root is
    // in front (`usePlaneRootPanels`, `PlaneView.tsx`), and here a workspace is. Kept, so a reader
    // that comes is seen.
    rootPanels: {},
    instructions: {
      chats_plane_updated: 1,
    },
    curations: {
      ask_persona_offer: 1,
      curation_offers: 1,
    },
    settings: {
      ask_persona_offer: 1,
      asks_waiting: 1,
      chats_level_why: 1,
      chats_on_older_sandbox: 1,
      extension_facts: 1,
      extension_views: 1,
      extensions_on: 2,
      harness_cards: 1,
      project_icons_drawn: 1,
      project_lives_here: 1,
      project_policy_state: 1,
      project_read_only: 1,
      project_theme_drawn: 1,
      sandbox_state: 4,
      theme_in_force: 1,
    },
    git: {
      alerts_everywhere: 1,
      clones_to_vouch: 1,
      plane_saving: 1,
      stale_clones: 1,
      workspace_saving: 1,
    },
    views: {
      artifact_pins: 1,
    },
    "not known": {
      alerts_everywhere: 1,
      artifact_pins: 1,
      ask_persona_offer: 1,
      asks_waiting: 1,
      chats_level_why: 1,
      chats_on_older_sandbox: 1,
      chats_plane_updated: 1,
      clones_to_vouch: 1,
      curation_offers: 1,
      dispatch_arrival: 1,
      extension_facts: 1,
      extension_views: 1,
      extensions_on: 2,
      finished_tasks: 1,
      harness_cards: 1,
      persona_marks: 1,
      plane_saving: 1,
      plane_sidebar: 1,
      project_icons_drawn: 1,
      project_lives_here: 1,
      project_policy_state: 1,
      project_read_only: 1,
      project_theme_drawn: 1,
      sandbox_state: 4,
      stale_clones: 1,
      theme_in_force: 1,
      workspace_panels: 1,
      workspace_saving: 1,
    },
  },
  turnEndOffScreen: {
    asks_waiting: 1,
    finished_turns: 1,
    signing_waiting: 1,
    "plugin:event|emit window-said": 1,
  },
  turnEndOnScreen: {
    asks_waiting: 1,
    chat_usage: 2,
    finished_turns: 1,
    signing_waiting: 1,
    "plugin:event|emit window-said": 1,
  },
  paneMount: {
    chat_hibernated: 2,
    chat_lost_with_the_host: 1,
    chat_skills_missing: 1,
    chat_usage: 2,
    dispatch_arrival: 1,
    mcp_failures: 1,
    persona_grants_held: 1,
    signing_waiting: 1,
    task_quiet: 1,
    tasks_sharing_a_folder: 1,
    window_layout: 1,
  },
  chatStart: {
    asks_waiting: 1,
    chat_hibernated: 4,
    chat_lost_with_the_host: 2,
    chat_order: 1,
    chat_skills_missing: 2,
    chat_usage: 4,
    dispatch_arrival: 1,
    dispatch_grants_needed: 1,
    finished_tasks: 1,
    harness_setup_found: 1,
    held_acts_waiting: 1,
    mcp_failures: 2,
    persona_grants_held: 2,
    plane_sidebar: 1,
    pool_setup_in_the_picker: 1,
    project_settings: 1,
    rules_made: 1,
    signing_waiting: 2,
    start_chat: 1,
    start_details: 1,
    start_options_first: 1,
    start_runs_on: 1,
    task_quiet: 2,
    tasks_sharing_a_folder: 2,
    vault_refusals: 1,
    window_layout: 1,
    "plugin:event|emit window-said": 1,
  },
};

const WORKSPACES = 2;

/**
 * **The longest a moment's own asks wait to be sent**, each longer than the one `settled` looks
 * for: a chat's move, a settings change or a focus has the asks read again after a burst
 * (`asks.ts`); a pane given a chat reads its gauge again a moment later (`ChatGauge.tsx`); and git
 * moving has the saving rows, the alerts and the stale clones read each at its own turn
 * (`afterGitMoved.ts`, H8), the last of them after the others.
 */
const FOLLOWED_FOR = {
  asks: ASKS_SETTLE_MS,
  pane: AFTER_A_MOVE_MS,
  git: AFTER_GIT_MOVED_MS.staleClones,
} as const;

/**
 * Past the wait `after` which the moment's last ask is sent, and until all it asked has been
 * answered: each moment owns the asks it starts, and none lands in the next.
 */
async function askedAll(asked: Asked[], after: number): Promise<void> {
  await act(async () => {
    await new Promise((done) => setTimeout(done, after + ASKS_SETTLE_MS * 4));
  });
  await settled(asked);
}

/** What a window holding `chats` chats listens for, and asks for each of its moments. */
async function asksFor(chats: number): Promise<Measured> {
  const plane = rootOf(0);
  const { asked, listeners, planeChanged, chatMoved } = await aWindow({
    projects: 1,
    chats,
    workspaces: WORKSPACES,
  });
  const tabs = within(stripNamed("Tabs")).getAllByRole("tab");
  // The load is there: a tab for each chat of the workspace in front, and one pane on screen.
  expect(tabs).toHaveLength(chats / WORKSPACES);
  expect(screen.getAllByTestId("pane")).toHaveLength(1);
  expect(tabs[1]).toHaveAttribute("aria-selected", "false");
  // Session 2 is in the workspace not in front, a background chat; session 1 is the pane's.
  const move = (session: number, state: string, at: number) =>
    chatMoved({
      plane,
      session,
      state,
      needs_you: false,
      queue: [],
      sequence: 1_000 + at,
      moved_at: 1_000 + at,
      reports: [],
      refusals: [],
    });
  // Each chat is heard of once before, working, so what is counted is its turn ending; and the
  // window's own pane, moved with it, has read its gauge again.
  act(() => move(2, "working", 0));
  act(() => move(1, "working", 1));
  await askedAll(asked, FOLLOWED_FOR.pane);
  const held = listeners();
  const listensOnLoad = asksIn(asked.filter((one) => one.cmd === "plugin:event|listen"));

  /** The asks the moment started, every one of them answered, the last `after` it. */
  const measured = async (
    moment: () => Promise<void> | void,
    after: number = FOLLOWED_FOR.asks,
  ): Promise<Asks> => {
    const from = asked.length;
    await moment();
    await askedAll(asked, after);
    return asksIn(asked.slice(from));
  };

  const focus = await measured(() => act(() => void fireEvent.focus(window)));

  // Git moved, or what moved is not known and git may have.
  const movesGit = (kind: (typeof KINDS)[number]) => kind === "git" || kind === null;
  const planeChangedAsks: Record<string, Asks> = {};
  for (const kind of KINDS)
    planeChangedAsks[kindName(kind)] = await measured(
      () => planeChanged(plane, kind),
      movesGit(kind) ? FOLLOWED_FOR.git : FOLLOWED_FOR.asks,
    );

  const turnEndOffScreen = await measured(() => act(() => move(2, "done", 2)));
  // The pane's chat: its gauge reads at once and again a moment later (`ChatGauge.tsx`).
  const turnEndOnScreen = await measured(() => act(() => move(1, "done", 3)), FOLLOWED_FOR.pane);

  Object.assign(panes, { started: 0, stopped: 0 });
  const paneMount = await measured(
    () => act(() => void fireEvent.click(tabs[1])),
    FOLLOWED_FOR.pane,
  );
  // The press gave the pane the chat of the tab it brought to the front, and kept its terminal.
  expect(tabs[1]).toHaveAttribute("aria-selected", "true");
  expect(screen.getByTestId("pane")).toHaveTextContent("session 3");
  expect(panes, "terminals for the press").toEqual({ started: 0, stopped: 0 });

  Object.assign(panes, { started: 0, stopped: 0 });
  const chatStart = await measured(async () => {
    fireEvent.click(screen.getAllByRole("button", { name: "Split right" })[0]);
    fireEvent.click(await screen.findByRole("button", { name: "Start chat" }));
    await waitFor(() => expect(screen.getAllByTestId("pane")).toHaveLength(2));
  }, FOLLOWED_FOR.pane);
  // The split started the new chat's terminal and the first chat's again beside it.
  expect(panes, "terminals for the start").toEqual({ started: 2, stopped: 1 });

  // Each moment happened: the focus and the settings change read the asks, once each, as a
  // chat's move does (`windowDraws.cost.test.tsx`); each turn's end read the finishes once
  // (`finishRows.ts`), witnessed apart from the gauge, so a gauge that stops reading is a table's
  // red and not this one; the start was sent once.
  expect(focus.asks_waiting, "asks read for the focus").toBe(1);
  expect(planeChangedAsks.settings.asks_waiting, "asks read for the settings change").toBe(1);
  expect(turnEndOffScreen.finished_turns, "finishes read for the turn's end off screen").toBe(1);
  expect(turnEndOnScreen.finished_turns, "finishes read for the turn's end on screen").toBe(1);
  expect(chatStart.start_chat, "starts sent for the chat").toBe(1);

  return {
    listensOnLoad,
    listeners: held,
    focus,
    planeChanged: planeChangedAsks,
    turnEndOffScreen,
    turnEndOnScreen,
    paneMount,
    chatStart,
  };
}

describe("what the window listens for and asks the core", () => {
  it("holds the listeners and sends the asks the baseline says for each moment, at 50 chats", async () => {
    const atFifty = await asksFor(50);
    // Each apart, so a red run names the one that moved.
    expect
      .soft(listenedMoreThanOnce(atFifty.listensOnLoad), "events listened for more than once")
      .toEqual({});
    expect.soft(atFifty.listeners, "listeners held").toEqual(BASELINE.listeners);
    expect.soft(atFifty.focus, "a focus").toEqual(BASELINE.focus);
    for (const kind of KINDS)
      expect
        .soft(atFifty.planeChanged[kindName(kind)], `plane-changed ${kindName(kind)}`)
        .toEqual(BASELINE.planeChanged[kindName(kind)]);
    expect
      .soft(atFifty.turnEndOffScreen, "a turn's end, off screen")
      .toEqual(BASELINE.turnEndOffScreen);
    expect
      .soft(atFifty.turnEndOnScreen, "a turn's end, on screen")
      .toEqual(BASELINE.turnEndOnScreen);
    expect.soft(atFifty.paneMount, "a pane mounted").toEqual(BASELINE.paneMount);
    expect.soft(atFifty.chatStart, "a chat started").toEqual(BASELINE.chatStart);
  }, 300_000);

  it("holds and asks no more with more chats", async () => {
    const atTen = await asksFor(10);
    cleanup();
    clearMocks();
    const atFifty = await asksFor(50);
    // What the window holds, and what each moment asks, is the same at 50 chats as at 10: before
    // W1 (#7909), the project view held one listener more of two events for each chat more. The
    // listens of each load apart: a module's own listener (`quiet-changed`), made once as it is
    // first loaded, is asked for as the first window loads and held for the page's life.
    expect({ ...atFifty, listensOnLoad: {} }).toEqual({ ...atTen, listensOnLoad: {} });
  }, 600_000);
});
