// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type {
  ChatBlocks,
  ChatLog,
  ChatLogRow,
  FinishedTurn,
  InboxUpdate,
  OpenChat,
  SandboxNetwork,
  SessionRecordRow,
  UpdateNoted,
} from "./bindings";
import { aTabForEachChat } from "./laidOut.testkit";
import { forgetInbox } from "./inboxRules";
import { chatList, pressMenuPath } from "./menus.testkit";
import { forgetProjectViews } from "./projectViews";
import { askSettingsLink } from "./settings/links";
import { findTree } from "./ui/tree/tree.testkit";
import { showThePanel, stripNamed } from "./test-strips";

/**
 * **The Chat log opens from each of its five entry points** (R-226, T1-37) against the whole
 * window, with a stand-in core: the chat row, a Blocked log row, a block result (the turn's
 * finish row in Updates), the session record and the marker. Each lands on the right chat's
 * Chat log: chat 7's ("talk"), never chat 3's, which is in front. Opening is a window
 * navigation: each presses the chat's own Chat log row (`chat.log:<session>`), and a way that
 * points at a block opens it filtered to blocks (R-301).
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane" tabIndex={-1}>
      session {session}
    </div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

beforeEach(() => globalThis.localStorage.clear());
afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
  // What one test's window kept of Updates and of the side it showed, which the next starts
  // without.
  forgetInbox();
  forgetProjectViews();
});

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

function chat(over: Partial<OpenChat> & Pick<OpenChat, "session" | "name">): OpenChat {
  return {
    cwd: ALPHA,
    harness: "claude",
    in_front: false,
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
    ...over,
  };
}

/** Chat 3 is in front; chat 7, the one every entry point names, is in a tab behind it. */
const CHATS: OpenChat[] = [
  chat({ session: 3, name: "3", in_front: true }),
  chat({ session: 7, name: "talk", label: "talk", persona: "devops" }),
];

/** Chat `session`'s own start row, which tells its Chat log from another chat's. */
const started = (session: number): ChatLogRow => ({
  kind: "started",
  at: "2026-11-13T09:00:00Z",
  cause: "start",
  protection: {
    layers: [{ layer: "sandbox", level: "floor", tier: "project" }],
    charter_commit: `c0ffee${session}`,
    charter_dirty: false,
    answers_in_force: 0,
  },
});

/** Chat 7's block of api.example.com. */
const BLOCK: ChatLogRow = {
  kind: "blocked",
  at: "2026-11-13T09:10:00Z",
  tool: "Bash",
  source: "sandbox",
  qualifier: null,
  target_kind: null,
  target: null,
  harness: "claude",
  reason: null,
};

/** The Blocked log: chat 7's block, with the number of the chat it was, open here. */
const NETWORK: SandboxNetwork = {
  on: true,
  open: [],
  blocked: [
    {
      target: "api.example.com:443",
      looked_up: null,
      said: "blocked by the sandbox",
      chat: "talk",
      session: 7,
      at: 1_790_000_000,
      times: 1,
      reached: false,
      levels: [],
    },
  ],
};

/** Chat 7's turn that ended with its block unresolved, as the core counts it. */
const TURN: FinishedTurn = {
  session: 7,
  key: "01TURNUNRESOLVED",
  at: 1_790_000_100,
  failed: false,
  blocked: 1,
};

/** A session record chat 7 wrote. */
const RECORD = "workspaces/alpha/sessions/20261113-100000-ship-the-widget.md";
const RECORDED: SessionRecordRow = {
  path: RECORD,
  title: "Ship the widget",
  when: "2026-11-13 10:00",
  persona: "devops",
  harness: "claude",
  resumable: false,
};

/** The Sessions panel, listing {@link RECORDED}, as `panels::sessions_panel` draws it. */
const SESSIONS_PANEL = {
  key: "charter/sessions",
  title: "Sessions",
  order: 30,
  mark: "note",
  from: null,
  about: null,
  blocks: [
    {
      kind: "list",
      rows: [
        {
          key: RECORD,
          text: RECORDED.title,
          note: `${RECORDED.when} · devops · claude`,
          mark: "note",
          tone: "plain",
          detail: null,
          runs: `session.open:${RECORD}`,
          actions: [],
        },
      ],
      empty: { headline: "No session records yet", body: null, offer: null },
    },
  ],
};

/** Chat 7's turn ended, waiting on the person: what makes the window ask for its finishes. */
const WAITING = {
  plane: PLANE,
  session: 7,
  state: "waiting",
  needs_you: false,
  queue: [],
  sequence: 1,
  moved_at: 1,
  reports: [],
  refusals: [],
  children: [],
};

/** The project's settings files, empty: Settings draws its pages from them. */
const settingsFile = (which: "shared" | "local") => ({
  which,
  file: which === "shared" ? "charter.toml" : "charter.local.toml",
  exists: which === "shared",
  text: "",
  refusals: [],
  parsed: true,
  fields: [],
});

/** The stand-in core: chat 7 has two blocks, a Blocked log row, a blocked turn and a record. */
function core() {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  const seen = new Set<string>();
  const kept: InboxUpdate[] = [];
  const shown = () => [...kept].sort((one, other) => other.at - one.at);
  mockIPC(
    (cmd, args) => {
      const given = (args ?? {}) as Record<string, unknown>;
      asked.push({ cmd, args: given });
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          workspaces: [{ name: "alpha", path: ALPHA, vision: "Ship it", todos: [], chats: CHATS }],
          personas: ["steward", "devops"],
          persona: "steward",
          unfiled: [],
        };
      if (cmd === "opened_chats") return CHATS;
      if (cmd === "reopened_layout") return aTabForEachChat(CHATS, () => "alpha");
      if (cmd === "chat_states") return [WAITING];
      if (
        [
          "reopened_views",
          "chats_that_would_not_start",
          "running_sessions",
          "extension_views",
          "extension_commands",
          "extension_panels",
          "extensions_on",
          "sandbox_grants",
        ].includes(cmd)
      )
        return [];
      if (cmd === "inbox_seen") return [...seen];
      if (cmd === "see_inbox") {
        for (const key of given.keys as string[]) seen.add(key);
        return [...seen];
      }
      if (cmd === "inbox_updates") return shown();
      if (cmd === "note_inbox_updates") {
        for (const one of given.noted as UpdateNoted[])
          if (!kept.some((was) => was.key === one.key)) kept.push({ ...one, read: false });
        return shown();
      }
      if (cmd === "settle_inbox_updates") return shown();
      if (cmd === "finished_turns")
        return (given.sessions as number[]).includes(TURN.session) ? [TURN] : [];
      if (cmd === "chat_log") {
        const session = Number(given.session);
        const read: ChatLog = {
          known: true,
          rows: session === 7 ? [started(7), BLOCK] : [started(session)],
        };
        return read;
      }
      if (cmd === "chat_blocks")
        return (given.asked as { session: number; after: string | null }[]).map(
          ({ session }): ChatBlocks => ({
            session,
            blocks: session === 7 ? 2 : 0,
            unverified: 0,
            key: session === 7 ? `blocks:01K6CHAT7:01B` : null,
          }),
        );
      if (cmd === "sandbox_network") return NETWORK;
      if (cmd === "project_settings")
        return { shared: settingsFile("shared"), local: settingsFile("local") };
      if (cmd === "workspace_panels")
        return {
          workspace: "alpha",
          repos: [],
          paths: {},
          absent: [],
          refused: [],
          todos: [],
          todos_refused: null,
          personas: ["steward", "devops"],
          persona: "steward",
          sessions: [RECORDED],
          contributed: [SESSIONS_PANEL],
        };
      if (cmd === "workspace_repos") return { workspace: "alpha", repos: [], cache_refused: null };
      if (cmd === "session_record")
        return {
          row: RECORDED,
          place: "alpha",
          body: "# Ship the widget\n\n## Goal\n\nShip it.\n",
          persona_hosts: [],
          resume_holds: false,
          persona_hosts_locked: null,
          persona_hosts_wait: false,
          dispatches: [],
          // The chat that wrote it, by its number here: it is open.
          session: 7,
        };
      return null;
    },
    { shouldMockEvents: true },
  );
  return {
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map(({ args }) => args),
  };
}

/** The tab in front's name. */
const selected = () =>
  within(stripNamed("Tabs")).getByRole("tab", { selected: true }).textContent ?? "";

/** **It landed on chat 7's Chat log**: its tab is in front, and its timeline is chat 7's own,
 *  read from the core for chat 7. With `blocks`, it shows the blocks alone. */
async function landedOnTalksLog(
  asked: (cmd: string) => Record<string, unknown>[],
  blocks: boolean,
) {
  await waitFor(() => expect(selected()).toContain("Chat log · talk"));
  const log = await screen.findByRole("list", { name: "Chat log of talk" });
  await waitFor(() => expect(asked("chat_log")).toContainEqual({ plane: PLANE, session: 7 }));
  expect(asked("chat_log")).not.toContainEqual({ plane: PLANE, session: 3 });
  const filter = await screen.findByRole("button", { name: "Blocks only" });
  if (blocks) {
    expect(filter).toHaveAttribute("aria-pressed", "true");
    await waitFor(() =>
      expect(
        within(log)
          .getAllByRole("listitem")
          .map((one) => one.dataset.kind),
      ).toEqual(["blocked"]),
    );
  } else {
    expect(filter).toHaveAttribute("aria-pressed", "false");
    await waitFor(() => expect(log).toHaveTextContent("c0ffee7"));
  }
}

/** Chat `name`'s row in the sidebar. */
async function rowOf(name: string): Promise<HTMLElement> {
  const tree = await findTree("Chats of this project");
  return waitFor(() => {
    const found = Array.from(tree.querySelectorAll<HTMLElement>('[role="row"]')).find(
      (one) => one.querySelector(".session")?.textContent === name,
    );
    if (found === undefined) throw new Error(`no row is named ${name}`);
    return found;
  });
}

describe("the Chat log opens from each of its five entry points", () => {
  it("the Chat log opens from each of its five entry points: the chat row", async () => {
    const { asked } = core();
    render(<App />);
    await screen.findAllByTestId("pane");

    await chatList("talk");
    await pressMenuPath("Open", "Chat log");

    await landedOnTalksLog(asked, false);
  });

  it("the Chat log opens from each of its five entry points: a Blocked log row", async () => {
    const { asked } = core();
    render(<App />);
    await screen.findAllByTestId("pane");

    act(() =>
      askSettingsLink(PLANE, {
        group: "project.sandbox.network",
        setting: "project.sandbox.network.blocked",
      }),
    );
    const log = await screen.findByRole("list", { name: "Blocked log" }, { timeout: 3000 });
    const [row] = within(log).getAllByRole("listitem");
    await userEvent.click(within(row).getByRole("button", { name: "Open the chat log of talk" }));

    await landedOnTalksLog(asked, true);
  });

  it("the Chat log opens from each of its five entry points: a block result", async () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    const { asked } = core();
    render(<App />);
    await screen.findAllByTestId("pane");

    await userEvent.click(
      within(await screen.findByRole("tablist", { name: "Attention" })).getByRole("tab", {
        name: /^Updates\b/,
      }),
    );
    const inbox = await screen.findByRole("tabpanel", { name: "Updates" });
    const result = await within(inbox).findByText(/· 1 blocked this turn/, undefined, {
      timeout: 3000,
    });
    const row = result.closest<HTMLElement>('[role="row"]');
    expect(row).not.toBeNull();
    await userEvent.click(
      within(row as HTMLElement).getByRole("button", { name: "Open the chat log of talk" }),
    );

    await landedOnTalksLog(asked, true);
  });

  it("the Chat log opens from each of its five entry points: the session record", async () => {
    const { asked } = core();
    render(<App />);
    await screen.findAllByTestId("pane");

    const panel = await showThePanel("Sessions");
    await userEvent.click(await within(panel).findByRole("row", { name: /Ship the widget/ }));
    // The record's own view: Updates, beside it, may name the chat too.
    const record = (await screen.findByTestId("session-record")).closest<HTMLElement>(
      '[role="tabpanel"]',
    );
    expect(record).not.toBeNull();
    await userEvent.click(
      await within(record as HTMLElement).findByRole("button", {
        name: "Open the chat log of talk",
      }),
    );

    await landedOnTalksLog(asked, false);
  });

  it("the Chat log opens from each of its five entry points: the marker", async () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    const { asked } = core();
    render(<App />);
    await screen.findAllByTestId("pane");

    const row = await rowOf("talk");
    await userEvent.click(
      await within(row).findByRole("button", { name: "2 new blocked" }, { timeout: 3000 }),
    );

    await landedOnTalksLog(asked, true);
  });
});
