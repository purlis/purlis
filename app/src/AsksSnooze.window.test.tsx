// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { UNDO_MS } from "./undo";
import type { Moved, Shown } from "./bindings";
import { aBatchOf } from "./chatEvents.testkit";
import { CHAT_EVENTS } from "./chatEvents";
import { heard } from "./frame.testkit";
import { aTabForEachChat } from "./laidOut.testkit";
import { forgetThisLaunch } from "./regions";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";

/**
 * **Snooze all on Asks** (#5647, spec #5638, ADR 0119), on the real window with the core
 * mocked: asked once in a line inside the view, it puts every listed chat's asks away and the
 * ✋ drops them; Asks says how many are snoozed with Show them now; an Undo stands a few
 * seconds; a chat comes back with all its asks when it raises a new one; and the Chats view,
 * which reads the board, still marks a snoozed chat as waiting.
 *
 * The pretend core keeps the record as the core's inbox store does and lists a snoozed chat's
 * asks apart by the same rule (`purlis_core::inboxupdates::away`, tested in Rust).
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const PLANE = "/home/dev/plane";

const chat = (session: number) => ({
  session,
  name: `steward ${session}`,
  cwd: `${PLANE}/workspaces/alpha`,
  harness: null,
  in_front: session === 1,
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
});

/** A permission prompt chat `session` holds on its hook, as the registry lists it. */
const permission = (session: number, ask: string, says: string): Shown => ({
  session,
  ask,
  says,
  options: [
    { id: "allow", label: "Allow", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: [`steward ${session}`],
  answer: { via: "hook" },
});

type Asked = { cmd: string; args: Record<string, unknown> };

/** Chats 1 and 2 in alpha, each holding one ask, both in the board's queue. */
function core() {
  const chats = [chat(1), chat(2)];
  const asked: Asked[] = [];
  let held: Shown[] = [
    permission(1, "ask-1", "Run cargo test"),
    permission(2, "ask-2", "Run npm test"),
  ];
  /** The core's snooze record: each snoozed chat, with the asks it held. */
  let record = new Map<number, string[]>();
  const everyListener = new Map<string, number[]>();
  const away = (session: number) => {
    const had = record.get(session);
    return (
      had !== undefined &&
      held.filter((one) => one.session === session).every((one) => had.includes(one.ask))
    );
  };
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: a });
    if (cmd === "plugin:event|listen") {
      const { event, handler } = args as { event: string; handler: number };
      everyListener.set(event, [...(everyListener.get(event) ?? []), handler]);
      return 1;
    }
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "asks_waiting") {
      // A chat holding an ask it did not have is let go of, as `Store::release` does.
      for (const session of [...record.keys()]) if (!away(session)) record.delete(session);
      return {
        plane: PLANE,
        asks: held.filter((one) => !record.has(one.session)),
        with_asking_chat: [],
        snoozed: held.filter((one) => record.has(one.session)),
      };
    }
    if (cmd === "snooze_inbox_asks") {
      for (const one of a.chats as { session: number; asks: string[] }[])
        record.set(one.session, one.asks);
      return null;
    }
    if (cmd === "wake_inbox_asks") {
      const sessions = a.sessions as number[] | null;
      record =
        sessions === null
          ? new Map()
          : new Map([...record].filter(([session]) => !sessions.includes(session)));
      return null;
    }
    if (cmd === "opened_chats") return chats;
    if (cmd === "reopened_layout") return aTabForEachChat(chats, () => "alpha");
    if (cmd === "plane_pins")
      return { project: false, workspaces: ["alpha"], missing: [], order: ["alpha"] };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
        workspaces: [
          { name: "alpha", path: `${PLANE}/workspaces/alpha`, vision: "", todos: [], chats },
        ],
      };
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
    return null;
  });
  const tell = (event: string, payload: unknown) =>
    act(() => {
      for (const one of everyListener.get(event) ?? []) heard(one, { event, id: 1, payload });
    });
  return {
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args),
    /** Chat `session` raises `ask`, and the core says its asks changed. */
    raise: (ask: Shown) => {
      held = [...held, ask];
      tell("asks-changed", { plane: PLANE });
    },
    /** The board says both chats wait on the person, as it does while they ask. */
    bothWait: () => {
      for (const session of [1, 2]) {
        const moved: Moved = {
          plane: PLANE,
          session,
          state: "waiting",
          needs_you: true,
          queue: [1, 2],
          moved_at: 10 + session,
          sequence: 10 + session,
          reports: [],
          refusals: [],
          children: [],
          needs: null,
          stopped: null,
        };
        tell(CHAT_EVENTS, aBatchOf("moved", moved));
      }
    },
  };
}

beforeEach(() => {
  forgetThisLaunch();
  vi.useFakeTimers({ shouldAdvanceTime: true, toFake: ["setTimeout", "clearTimeout"] });
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
  vi.useRealTimers();
});

const press = userEvent.setup({ advanceTimers: (ms) => vi.advanceTimersByTime(ms) });

const asksView = () => screen.findByRole("tabpanel", { name: "Asks" });
/** The chats Asks lists, by the heading of each group. */
const listed = () =>
  [...document.querySelectorAll('.inbox-chat [role="rowheader"]')].map((h) => h.textContent);
/** The hand on the title bar, by what it counts. */
const hand = (count: number) =>
  screen.queryAllByRole("button", {
    name: count === 1 ? "1 thing waits on you" : `${count} things wait on you`,
  });

async function opened() {
  const one = core();
  render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
  const view = await asksView();
  await waitFor(() => expect(listed()).toEqual(["steward 1", "steward 2"]));
  return { ...one, view };
}

/** Snooze all, asked and confirmed in the view. */
async function snoozeAll(view: HTMLElement) {
  await press.click(within(view).getByRole("button", { name: "Snooze all" }));
  const line = within(view).getByRole("group", { name: "Snooze all" });
  await press.click(within(line).getByRole("button", { name: "Snooze all" }));
}

/** Waits out the Undo. */
const undoGoes = () => act(() => vi.advanceTimersByTime(UNDO_MS + 1));

describe("Snooze all on Asks (#5647)", () => {
  it("asks first in a line inside the view, never a dialog, and Cancel leaves every ask", async () => {
    const { view, asked } = await opened();

    await press.click(within(view).getByRole("button", { name: "Snooze all" }));

    const line = within(view).getByRole("group", { name: "Snooze all" });
    expect(line).toHaveTextContent(
      "Snooze 2 asks? Each chat comes back when it asks something new.",
    );
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.queryByRole("alertdialog")).toBeNull();

    await press.click(within(line).getByRole("button", { name: "Cancel" }));
    expect(within(view).queryByRole("group", { name: "Snooze all" })).toBeNull();
    expect(listed()).toEqual(["steward 1", "steward 2"]);
    expect(asked("snooze_inbox_asks")).toEqual([]);
  });

  it("puts every listed chat's asks away, answering none, and lowers the hand's count", async () => {
    const { view, asked } = await opened();
    await waitFor(() => expect(hand(2).length).toBeGreaterThan(0));

    await snoozeAll(view);

    expect(asked("snooze_inbox_asks")).toEqual([
      {
        plane: PLANE,
        chats: [
          { session: 1, asks: ["ask-1"] },
          { session: 2, asks: ["ask-2"] },
        ],
      },
    ]);
    await waitFor(() => expect(listed()).toEqual([]));
    expect(within(view).getByText("Nothing is waiting on you")).toBeInTheDocument();
    expect(hand(2)).toEqual([]);
    expect(hand(1)).toEqual([]);
    // Nothing was answered.
    expect(asked("answer_ask")).toEqual([]);
  });

  it("says how many are snoozed with Show them now, while the Chats view still marks them waiting", async () => {
    const { view, asked, bothWait } = await opened();
    bothWait();
    await snoozeAll(view);
    await waitFor(() => expect(listed()).toEqual([]));
    undoGoes();

    expect(
      await within(view).findByText("2 asks snoozed until their chats ask again."),
    ).toBeInTheDocument();
    // The Chats view reads the board, which still has both chats waiting.
    const chats = await screen.findByRole("tree", { name: "Chats of this project" });
    await waitFor(() => expect(within(chats).getAllByText("needs you")).toHaveLength(2));

    await press.click(within(view).getByRole("button", { name: "Show them now" }));

    expect(asked("wake_inbox_asks")).toEqual([{ plane: PLANE, sessions: null }]);
    await waitFor(() => expect(listed()).toEqual(["steward 1", "steward 2"]));
    expect(within(view).queryByText(/snoozed until/)).toBeNull();
  });

  it("stands an Undo for a few seconds that brings the asks back", async () => {
    const { view, asked } = await opened();
    await snoozeAll(view);
    const undo = await within(view).findByRole("button", { name: "Undo" });
    expect(within(view).getByText("Snoozed 2 asks.")).toBeInTheDocument();

    await press.click(undo);

    expect(asked("wake_inbox_asks")).toEqual([{ plane: PLANE, sessions: [1, 2] }]);
    await waitFor(() => expect(listed()).toEqual(["steward 1", "steward 2"]));
  });

  it("takes the Undo away after a few seconds", async () => {
    const { view } = await opened();
    await snoozeAll(view);
    await within(view).findByRole("button", { name: "Undo" });

    undoGoes();

    await waitFor(() => expect(within(view).queryByRole("button", { name: "Undo" })).toBeNull());
    expect(listed()).toEqual([]);
  });

  it("brings a chat back with all its asks when it raises a new one, and never on a timer", async () => {
    const { view, raise } = await opened();
    await snoozeAll(view);
    await waitFor(() => expect(listed()).toEqual([]));
    undoGoes();
    // Time alone brings nothing back.
    act(() => vi.advanceTimersByTime(60 * 60 * 1000));
    expect(listed()).toEqual([]);

    raise(permission(1, "ask-1b", "Run cargo build"));

    await waitFor(() => expect(listed()).toEqual(["steward 1"]));
    const back = screen.getByRole("rowgroup", { name: "steward 1" });
    expect(within(back).getByText("Run cargo test")).toBeInTheDocument();
    expect(within(back).getByText("Run cargo build")).toBeInTheDocument();
    expect(
      within(view).getByText("1 ask snoozed until their chats ask again."),
    ).toBeInTheDocument();
  });
});
