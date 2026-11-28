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
 * commands it sends for a focus, for each kind of `plane-changed`, for a turn ending, for a
 * pane mounted and for a chat started.
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

/** What a window holds and asks, each by its name. */
type Measured = {
  /** The Tauri listeners it holds, by event, once it has settled. */
  listeners: Asks;
  /** What it asks as it is focused again. */
  focus: Asks;
  /** What it asks for each kind of `plane-changed`, by the kind's name. */
  planeChanged: Record<string, Asks>;
  /** What it asks as a chat's turn ends. */
  turnEnd: Asks;
  /** What it asks as its pane is given another chat. */
  paneMount: Asks;
  /** What it asks as the person starts a chat. */
  chatStart: Asks;
};

/**
 * **What the window holds and asks now: the baseline** (2026-11-28, on main at 15eab6a312).
 *
 * - **Listeners:** 219 over 75 events. `plane-changed` is held 24 times, `chat-events` 7 and
 *   `asks-changed` 5, and two events once for each chat open and once more (51 each at 50 chats,
 *   {@link PER_CHAT}). W1 (#7909) brings each event to 1.
 * - **A focus:** 16 asks, among them the asks, the saving rows, the alerts and the sandbox's
 *   standing, `sandbox_state` four times.
 * - **`plane-changed`:** each kind reads what holds its answer. `settings` costs 19 asks
 *   (`sandbox_state` four times, `extensions_on` twice); `git` 5, the last 4.5 s after it; a
 *   change the core could not place 32, every reader's. Nothing on screen holds `rootPanels`.
 * - **A turn's end:** 4 asks, the finishes once (`finishRows.ts`), the asks, the signings and
 *   the window's word to the others.
 * - **A pane given another chat** (a press on its tab): 33 asks. The terminal is kept and the
 *   chat's notices are mounted anew, by the chat (`ChatNotices`, #1486): ten listens, each after
 *   the last chat's unlisten, and 13 reads of the notices' own, the slow log's trivial per-chat
 *   reads (`chat_hibernated` twice), and the gauge twice.
 * - **A chat started** from a split: 70 asks, the picker's reads, the start, and two panes'
 *   notices, the new chat's and the first chat's, which the split mounts again beside it.
 */
const BASELINE: Measured = {
  listeners: {
    "artifact-pins-moved": 1,
    "asks-changed": 5,
    "asks-notified": 1,
    "branch-changed": 2,
    "browser-ask-ended": 1,
    "chat-browsers-ended": 1,
    "chat-browsers-listed": 2,
    "chat-events": 7,
    "chat-hibernation-changed": 3,
    "chat-input-refused": 1,
    "chat-lost-with-the-host": 1,
    "chat-mcp-failed": 1,
    "chat-move": 1,
    "chat-project-settings-changed": 1,
    "chat-restored": 1,
    "chat-sandbox-blocked": 2,
    "chat-stop": 1,
    "chat-vault-refused": 51,
    "chats-reconnecting": 3,
    "cluster-pool-added": 1,
    "dispatch-arrival": 1,
    "dispatch-away-changed": 1,
    "dispatch-grant-needed": 51,
    "editor-tab-opened": 1,
    "engine-download": 1,
    "engine-sandbox-needed": 1,
    "extension-heard": 2,
    "files-changed": 2,
    "files-unwatched": 2,
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
    "plane-changed": 24,
    "plane-closed": 1,
    "pool-mints-without-asking": 1,
    "pool-pin-changed": 1,
    "pool-setup-ready": 1,
    "pool-sign-in-confirm": 1,
    "prerequisites-changed": 1,
    "press-here": 1,
    "project-ready": 2,
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
  turnEnd: {
    asks_waiting: 1,
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
    "plugin:event|listen asks-changed": 1,
    "plugin:event|listen chat-hibernation-changed": 2,
    "plugin:event|listen chat-input-refused": 1,
    "plugin:event|listen chat-lost-with-the-host": 1,
    "plugin:event|listen chat-mcp-failed": 1,
    "plugin:event|listen dispatch-arrival": 1,
    "plugin:event|listen plane-changed": 2,
    "plugin:event|listen task-quiet": 1,
    "plugin:event|unlisten asks-changed": 1,
    "plugin:event|unlisten chat-hibernation-changed": 2,
    "plugin:event|unlisten chat-input-refused": 1,
    "plugin:event|unlisten chat-lost-with-the-host": 1,
    "plugin:event|unlisten chat-mcp-failed": 1,
    "plugin:event|unlisten dispatch-arrival": 1,
    "plugin:event|unlisten plane-changed": 2,
    "plugin:event|unlisten task-quiet": 1,
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
    "plugin:event|listen asks-changed": 2,
    "plugin:event|listen chat-hibernation-changed": 4,
    "plugin:event|listen chat-input-refused": 2,
    "plugin:event|listen chat-lost-with-the-host": 2,
    "plugin:event|listen chat-mcp-failed": 2,
    "plugin:event|listen chat-vault-refused": 1,
    "plugin:event|listen dispatch-arrival": 1,
    "plugin:event|listen dispatch-grant-needed": 1,
    "plugin:event|listen plane-changed": 3,
    "plugin:event|listen task-quiet": 2,
    "plugin:event|unlisten asks-changed": 1,
    "plugin:event|unlisten chat-hibernation-changed": 2,
    "plugin:event|unlisten chat-input-refused": 1,
    "plugin:event|unlisten chat-lost-with-the-host": 1,
    "plugin:event|unlisten chat-mcp-failed": 1,
    "plugin:event|unlisten dispatch-arrival": 1,
    "plugin:event|unlisten plane-changed": 2,
    "plugin:event|unlisten task-quiet": 1,
  },
};

/**
 * **The listeners the window holds once for each chat it has open**, by event: the project
 * view's `useRefusedAmong` and `useHeldAmong` (`PlaneView.tsx`) listen once for each of its
 * chats, beside the asks' own. A chat more is one more of each; W1 (#7909) ends them.
 */
const PER_CHAT: Asks = { "chat-vault-refused": 1, "dispatch-grant-needed": 1 };

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

/** What a window holding `chats` chats listens for, and asks for each of the five moments. */
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
  const move = (state: string, at: number) =>
    chatMoved({
      plane,
      session: 2,
      state,
      needs_you: false,
      queue: [],
      sequence: 1_000 + at,
      moved_at: 1_000 + at,
      reports: [],
      refusals: [],
    });
  // The chat is heard of once before, working, so what is counted is its turn ending; and the
  // window's own pane, mounted with it, has read its gauge again.
  act(() => move("working", 0));
  await askedAll(asked, FOLLOWED_FOR.pane);
  const held = listeners();

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

  const turnEnd = await measured(() => act(() => move("done", 1)));

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
  // chat's move does (`windowDraws.cost.test.tsx`); the turn's end read the finishes once
  // (`finishRows.ts`); the start was sent once.
  expect(focus.asks_waiting, "asks read for the focus").toBe(1);
  expect(planeChangedAsks.settings.asks_waiting, "asks read for the settings change").toBe(1);
  expect(turnEnd.finished_turns, "finishes read for the turn's end").toBe(1);
  expect(chatStart.start_chat, "starts sent for the chat").toBe(1);

  return {
    listeners: held,
    focus,
    planeChanged: planeChangedAsks,
    turnEnd,
    paneMount,
    chatStart,
  };
}

describe("what the window listens for and asks the core", () => {
  it("holds the listeners and sends the asks the baseline says for each moment, at 50 chats", async () => {
    const atFifty = await asksFor(50);
    // Each apart, so a red run names the one that moved.
    expect.soft(atFifty.listeners, "listeners held").toEqual(BASELINE.listeners);
    expect.soft(atFifty.focus, "a focus").toEqual(BASELINE.focus);
    for (const kind of KINDS)
      expect
        .soft(atFifty.planeChanged[kindName(kind)], `plane-changed ${kindName(kind)}`)
        .toEqual(BASELINE.planeChanged[kindName(kind)]);
    expect.soft(atFifty.turnEnd, "a turn's end").toEqual(BASELINE.turnEnd);
    expect.soft(atFifty.paneMount, "a pane mounted").toEqual(BASELINE.paneMount);
    expect.soft(atFifty.chatStart, "a chat started").toEqual(BASELINE.chatStart);
  }, 300_000);

  it("holds and asks no more with more chats, but for the listeners held for each chat", async () => {
    const atTen = await asksFor(10);
    cleanup();
    clearMocks();
    const atFifty = await asksFor(50);
    // What each moment asks is the same at 50 chats as at 10.
    expect({ ...atFifty, listeners: {} }).toEqual({ ...atTen, listeners: {} });
    // The listeners held grow by PER_CHAT for each chat more, and by nothing else.
    const events = new Set([...Object.keys(atTen.listeners), ...Object.keys(atFifty.listeners)]);
    const grown: Asks = {};
    for (const event of events) {
      const more = (atFifty.listeners[event] ?? 0) - (atTen.listeners[event] ?? 0);
      if (more !== 0) grown[event] = more / (50 - 10);
    }
    expect(grown, "listeners more for each chat more").toEqual(PER_CHAT);
  }, 600_000);
});
