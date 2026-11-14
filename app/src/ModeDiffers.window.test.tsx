// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App from "./App";
import type { ChatNoticed, ModeDiffers, Shown } from "./bindings";
import { askRows, forgetInboxOpen, inboxOpenAtLaunch } from "./test-inbox";
import { forgetInbox } from "./inboxRules";
import { aBatchOf } from "./chatEvents.testkit";
import { CHAT_EVENTS } from "./chatEvents";
import { aTabForEachChat } from "./laidOut.testkit";

/**
 * **A mode purlis didn't set is a standing fault: a Notice, never an ask** (R-243, T1-44,
 * #6435; Q89, Q270, Q271), on the real window with the core mocked. The attendance fact flips a
 * Claude Code chat to unattended when its armed hook reports a mode purlis didn't start it in,
 * and the core tells the window which two modes, on the chat's `noticed` telling. The pane says
 * so in one Notice with its way out, and Asks and the ✋ are as they were. A chat whose harness
 * is not read for its mode (Codex, opencode) is told no mode, and shows nothing.
 *
 * The other standing faults are a later phase's (#6391): "purlis decides" forbidden is phase
 * 3's (R-324), a broken policy and a ruleset a policy requires are phase 4's (R-149).
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

const CHAT = {
  session: 4,
  name: "claude 4",
  cwd: `${ALPHA}/repo`,
  harness: "claude",
  in_front: true,
  resumed: null,
  fresh: null,
  profile: "claude",
  persona: null,
  unreported: null,
  guessed: null,
  pinned: false,
  label: null,
  from: null,
};

/** A permission prompt chat 4 holds on its hook, as the registry lists it. */
const PERMISSION: Shown = {
  session: 4,
  ask: "permission:4:1",
  says: "Run cargo test",
  options: [
    { id: "allow", label: "Allow", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: ["claude 4"],
  answer: { via: "hook" },
};

/** What the core noticed of chat 4: nothing of Stuck? or work left, and `mode` as given. */
const told = (mode: ModeDiffers | null): ChatNoticed => ({
  plane: PLANE,
  session: 4,
  stuck: null,
  left_work: null,
  mode,
});

const PLAN: ModeDiffers = { set: "default", reported: "plan" };

/** The core, with chat 4 open on its tab, the registry listing chat 4's permission prompt, and
 *  `first` as what it first answers it noticed of the project's chats. */
function core(first: ChatNoticed[] = []) {
  mockIPC(
    (cmd) => {
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [CHAT];
      if (cmd === "reopened_layout") return aTabForEachChat([CHAT], () => "alpha");
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: [],
          persona: null,
          unfiled: [],
          workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats: [CHAT] }],
        };
      if (cmd === "plane_doctor") return { rows: [], app_rows: [], full: false, path: "" };
      if (cmd === "asks_waiting")
        return { plane: PLANE, asks: [PERMISSION], with_asking_chat: [] };
      if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
      if (cmd === "inbox_updates" || cmd === "note_inbox_updates") return [];
      if (cmd === "chat_states" || cmd === "running_sessions") return [];
      if (cmd === "chats_noticed") return first;
      if (cmd === "chats_that_would_not_start" || cmd === "owed_restarts") return [];
      return null;
    },
    { shouldMockEvents: true },
  );
}

const modeNotices = () => [...document.querySelectorAll('[data-cause^="mode-differs:"]')];
const hand = () =>
  within(screen.getByRole("tablist", { name: "Attention" })).getByRole("tab", { name: /^Asks/ });
const handCount = () => hand().querySelector('[data-part="count"]');

/** The window open on chat 4's pane, Asks listing its one ask. */
async function opened() {
  render(<App />);
  await screen.findByRole("tabpanel", { name: "Asks" });
  await screen.findByTestId("pane");
  await waitFor(() => expect(askRows()).toHaveLength(1));
  await waitFor(() => expect(handCount()).toHaveAttribute("data-count", "1"));
}

const tell = (noticed: ChatNoticed) =>
  act(() => emit(CHAT_EVENTS, aBatchOf("noticed", noticed)));

/** Long enough for anything the window would read again on a telling to have come back. */
const settle = () =>
  act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 120));
  });

beforeEach(() => inboxOpenAtLaunch());

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInboxOpen();
  forgetInbox();
});

describe("mode differs is a Notice and never an ask", () => {
  it("a hook-reported mode purlis didn't set raises the Notice and leaves Asks unchanged", async () => {
    core();
    await opened();
    expect(modeNotices()).toHaveLength(0);

    await tell(told(PLAN));

    await waitFor(() => expect(modeNotices()).toHaveLength(1));
    const notice = modeNotices()[0] as HTMLElement;
    expect(notice).toHaveAttribute("data-cause", "mode-differs:4:plan");
    expect(notice).toHaveTextContent(
      "Claude Code reports this chat runs in plan mode, but purlis started it in default mode.",
    );
    expect(notice).toHaveTextContent("purlis counts this chat as unattended from now on.");
    // Its way out.
    expect(within(notice).getByRole("button", { name: /Dismiss/ })).toBeInTheDocument();

    await settle();
    // Asks is as it was: the one ask, and no row of the fault beside it.
    expect(askRows()).toHaveLength(1);
    expect(askRows()[0]).toHaveTextContent("Run cargo test");
    expect(askRows().some((row) => row.textContent?.includes("plan mode"))).toBe(false);
    expect(handCount()).toHaveAttribute("data-count", "1");
  });

  it("is drawn from the core's first answer too, and names a mode nobody measured as unknown", async () => {
    core([told({ set: "acceptEdits", reported: "unknown" })]);
    await opened();

    await waitFor(() => expect(modeNotices()).toHaveLength(1));
    expect(modeNotices()[0]).toHaveTextContent(
      "Claude Code reports a permission mode for this chat that purlis doesn't know. purlis started it in accept edits mode.",
    );
    expect(askRows()).toHaveLength(1);
  });

  it("shows nothing for a chat told no mode: the mode set, or a harness not read for one", async () => {
    core();
    await opened();

    await tell(told(null));
    await settle();

    expect(modeNotices()).toHaveLength(0);
    expect(askRows()).toHaveLength(1);
  });

  it("Dismiss puts it away until another mode is reported", async () => {
    core();
    await opened();
    await tell(told(PLAN));
    const notice = await waitFor(() => {
      expect(modeNotices()).toHaveLength(1);
      return modeNotices()[0] as HTMLElement;
    });

    await userEvent.click(within(notice).getByRole("button", { name: /Dismiss/ }));
    await waitFor(() => expect(modeNotices()).toHaveLength(0));

    // The same fault told again stays away; another mode is a new cause.
    await tell(told(PLAN));
    await settle();
    expect(modeNotices()).toHaveLength(0);
    await tell(told({ set: "default", reported: "bypassPermissions" }));
    await waitFor(() => expect(modeNotices()).toHaveLength(1));
    expect(modeNotices()[0]).toHaveTextContent("runs in bypass permissions mode");
  });

  // The other standing faults, each named for its phase (#6391).
  it.todo("\"purlis decides\" forbidden is a Notice and never an ask (phase 3, R-324, #6391)");
  it.todo("a broken policy is a Notice and never an ask (phase 4, R-243, #6391)");
  it.todo("a ruleset a policy requires is a Notice and never an ask (phase 4, R-149, #6391)");
});
