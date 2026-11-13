// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App from "./App";
import type { Moved, OpenChat, Shown } from "./bindings";
import { askRows, forgetInboxOpen, inboxOpenAtLaunch } from "./test-inbox";
import { forgetInbox } from "./inboxRules";
import { onTheWire } from "./chatEvents.testkit";
import { aTabForEachChat } from "./laidOut.testkit";
import { forgetProjectViews } from "./projectViews";

/**
 * **Waiting in its terminal** (R-204, R-292, R-351, T1-35), on the real window with the core
 * mocked as the asks registry. A permission hook that gives up (its ask too big, not its
 * harness's, too late, or the chat at the 32-open cap) leaves the harness's own prompt in the
 * pane, and the core lists the chat as one `terminal` ask (`asking::terminal`): a row in Asks
 * with Go to chat and nothing that answers it. The pane answers it, and the row leaves. A chat
 * stopped at its harness's folder-trust prompt is that row where a give-up fired, and reads
 * Can't tell in All chats where nothing did, never a dialog alone.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

const chatOn = (harness: string): OpenChat => ({
  session: 4,
  name: "steward 4",
  cwd: `${ALPHA}/repo`,
  harness,
  in_front: true,
  resumed: null,
  fresh: null,
  profile: harness,
  persona: null,
  unreported: null,
  card: null,
  guessed: null,
  pinned: false,
  label: null,
  from: null,
});

/** Chat 4 as the core's `asking::terminal` lists it, stopped on its harness's permission prompt. */
const TERMINAL: Shown = {
  session: 4,
  ask: "terminal:4",
  says: "Waiting in its terminal: a permission prompt",
  options: [],
  source: "terminal",
  chain: ["steward 4"],
  answer: { via: "in-its-pane" },
};

/** A permission prompt chat 4 holds on its hook. */
const held = (n: number): Shown => ({
  session: 4,
  ask: `permission:4:${n}`,
  says: `Run step ${n}`,
  options: [
    { id: "allow", label: "Allow", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: ["steward 4"],
  answer: { via: "hook" },
});

/** Chat 4 on the board, waiting on the person: what a give-up leaves it as. */
const WAITING: Moved = {
  plane: PLANE,
  session: 4,
  state: "waiting",
  needs_you: true,
  queue: [4],
  moved_at: 2,
  reports: [],
  refusals: [],
  children: [],
  sequence: 2,
};

/** Chat 4 once its pane answered the prompt: running again, its wait over. */
const ANSWERED: Moved = { ...WAITING, state: "running", needs_you: false, queue: [], sequence: 3 };

/**
 * The core, with chat 4 open on `harness`. `registry` is what the asks registry lists and
 * `states` what the board says of each chat, each read again whenever the window asks.
 */
function core(world: { harness?: string; registry: () => Shown[]; states?: () => Moved[] }) {
  const chats = [chatOn(world.harness ?? "claude")];
  mockIPC(
    (cmd) => {
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return chats;
      if (cmd === "reopened_layout") return aTabForEachChat(chats, () => "alpha");
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: [],
          persona: null,
          unfiled: [],
          workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats }],
        };
      if (cmd === "plane_doctor") return { rows: [], app_rows: [], full: false, path: "" };
      if (cmd === "asks_waiting")
        return { plane: PLANE, asks: world.registry(), with_asking_chat: [] };
      if (cmd === "chat_states") return (world.states ?? (() => []))();
      if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
      if (cmd === "inbox_updates" || cmd === "note_inbox_updates") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "chats_that_would_not_start" || cmd === "owed_restarts") return [];
      return null;
    },
    { shouldMockEvents: true },
  );
}

/** Asks' rows that say chat 4 waits in its terminal. */
const terminalRows = () =>
  askRows().filter((row) => row.textContent?.includes("Waiting in its terminal") === true);

/** The one such row, once Asks lists it. */
const theTerminalRow = () =>
  waitFor(() => {
    const rows = terminalRows();
    expect(rows).toHaveLength(1);
    return rows[0];
  });

/**
 * What answers a row in place: any button but Go to chat, and but what every chat's row offers
 * besides (the queue's Ignore, its ×, and Snooze and Stop, ADR 0121), none of which answers.
 */
const answersIn = (row: HTMLElement) =>
  within(row)
    .queryAllByRole("button")
    .map((button) => (button.getAttribute("aria-label") ?? button.textContent ?? "").trim())
    .filter((name) => !/^(Go to chat|Ignore |Snooze|Stop)/.test(name));

beforeEach(() => inboxOpenAtLaunch());

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInboxOpen();
  forgetInbox();
  forgetProjectViews();
});

describe("each hook give-up case is listed in Asks as Waiting in its terminal", () => {
  // R-204: the core lists the same `terminal` ask whichever case gave up (B2); at the 32-open
  // cap it stands beside the 32 asks the chat's hook still holds.
  const CASES: [string, Shown[]][] = [
    ["TooBig", [TERMINAL]],
    ["NotItsHarness", [TERMINAL]],
    ["TooLate", [TERMINAL]],
    ["the 32-open cap", [...Array.from({ length: 32 }, (_, n) => held(n + 1)), TERMINAL]],
  ];

  it.each(CASES)("%s: one Waiting in its terminal row, with Go to chat", async (_, registry) => {
    core({ registry: () => registry, states: () => [WAITING] });
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Asks" });

    const row = await theTerminalRow();
    expect(row).toHaveTextContent("steward 4");
    expect(within(row).getByRole("button", { name: "Go to chat steward 4" })).toBeInTheDocument();
    expect(answersIn(row)).toEqual([]);
  });
});

describe("a Waiting in its terminal row offers Go to chat only and leaves when answered in the pane", () => {
  it("offers Go to chat and no Allow or word for no, and leaves once the pane answers", async () => {
    // R-292: nothing the window could send answers the harness's own prompt, so the row offers
    // no Allow and no word for no; Go to chat takes the person to the pane that answers it.
    let waiting = true;
    core({
      registry: () => (waiting ? [TERMINAL] : []),
      states: () => [waiting ? WAITING : ANSWERED],
    });
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Asks" });

    const row = await theTerminalRow();
    expect(answersIn(row)).toEqual([]);
    for (const word of [/^Allow/, /^Deny/, /^Don't allow/, /^Not now/, /^Keep/]) {
      expect(within(row).queryByRole("button", { name: word })).toBeNull();
    }
    expect(within(row).queryByRole("textbox")).toBeNull();
    await userEvent.click(within(row).getByRole("button", { name: "Go to chat steward 4" }));
    await screen.findByTestId("pane");

    // The person answers in the pane: its tool runs on, the board moves the chat, and the
    // registry lists it no more.
    waiting = false;
    await act(() => emit(...onTheWire("chat-moved", ANSWERED)));

    await waitFor(() => expect(terminalRows()).toHaveLength(0));
  });
});

describe.each(["claude", "codex"])(
  "a chat at the folder-trust prompt is Waiting in its terminal when a give-up fired and Can't tell in All chats otherwise (%s)",
  (harness) => {
    const allChats = async () => {
      await userEvent.click(
        await screen.findByRole("button", { name: "All chats", pressed: false }),
      );
      const list = await screen.findByRole("grid", { name: "All chats" });
      return await waitFor(() =>
        within(list).getByRole("row", { name: /steward 4, .* · plane · alpha$/ }),
      );
    };

    it("with a give-up fired: one Waiting in its terminal row with Go to chat", async () => {
      // R-351: purlis can tell only where a give-up fired for the chat (Q205), and then the
      // chat is the row, in Asks, and not a dialog.
      core({ harness, registry: () => [TERMINAL], states: () => [WAITING] });
      render(<App />);
      await screen.findByRole("tabpanel", { name: "Asks" });

      const row = await theTerminalRow();
      expect(within(row).getByRole("button", { name: "Go to chat steward 4" })).toBeInTheDocument();
      expect(answersIn(row)).toEqual([]);
      expect(screen.queryByRole("dialog")).toBeNull();
      expect(screen.queryByRole("alertdialog")).toBeNull();
      expect(await allChats()).toHaveAccessibleName(/Waiting on you/);
    });

    it("with none fired: no row, and its All chats row reads Can't tell", async () => {
      // R-351: the prompt came before any hook loaded, so nothing reported the chat; purlis
      // says it can't tell, in All chats, where the person finds it. No dialog stands in.
      core({ harness, registry: () => [] });
      render(<App />);
      await screen.findByRole("tabpanel", { name: "Asks" });
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 120));
      });

      expect(terminalRows()).toHaveLength(0);
      expect(screen.queryByRole("dialog")).toBeNull();
      expect(screen.queryByRole("alertdialog")).toBeNull();
      expect(await allChats()).toHaveAccessibleName(/Can't tell/);
    });
  },
);
