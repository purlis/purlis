// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render as renderBare, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { theFrameNow } from "./frame.testkit";
import App from "./App";
import { KEYS_OWED, type KeysOwed, type Moved } from "./bindings";
import { forgetDismissals } from "./dismissals";
import { onTheWire } from "./chatEvents.testkit";
import { aTabForEachChat } from "./laidOut.testkit";
import { forgetInboxOpen, inboxOpenAtLaunch } from "./test-inbox";
import { windowCopy } from "./windowCopy";

/**
 * **A running chat's sandbox goes stale when model keys arrive** (R-775, T1a-78), against the
 * whole window. The core owes each running chat settled without the keyring rule a restart as the
 * first model key is added (`chat-keys-owed`), or as it takes the chat back at open
 * (`chats_owed_for_model_keys`). The window drives it as it drives every owed restart, at the
 * chat's next pause (`restartsDue`), and the chat's pane shows W140 with Restart now until it
 * restarts.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

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

/** Chat 4 mid-turn, then waiting for you: what the board says as its turn runs and ends. */
const RUNNING: Moved = {
  plane: PLANE,
  session: 4,
  state: "running",
  needs_you: false,
  queue: [],
  moved_at: 1,
  reports: [],
  refusals: [],
  children: [],
  sequence: 1,
};
const WAITING: Moved = { ...RUNNING, state: "waiting", moved_at: 2, sequence: 2 };

const RESTARTED = [{ plane: PLANE, session: 4, columns: 80, rows: 24, now: false }];
const RESTARTED_NOW = [{ ...RESTARTED[0], now: true }];

/** The core: chat 4 open, owed `owed` restarts as the view is drawn, `keys` of them for keys. */
function core({ owed = [], keys = [] }: { owed?: number[]; keys?: number[] } = {}) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC(
    (cmd, args) => {
      asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
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
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "owed_restarts") return owed;
      if (cmd === "chats_owed_for_model_keys") return keys;
      if (cmd === "chats_on_older_sandbox") return null;
      if (cmd === "restart_chat")
        return { chat: { ...CHAT, session: 9, resumed: "c1" }, notices: [], not_yet: null };
      return null;
    },
    { shouldMockEvents: true },
  );
  return {
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map(({ args }) => args),
  };
}

async function settled() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/** The core says the chat moved, and the window draws it on the next frame. */
async function moved(payload: Moved) {
  await emit(...onTheWire("chat-moved", payload));
  theFrameNow();
}

/** The window's write made your keys hold a key: the core owes chat 4 a restart. */
async function keysArrived() {
  await emit(KEYS_OWED, { plane: PLANE, sessions: [4] } satisfies KeysOwed);
}

async function aChat(said: Parameters<typeof core>[0] = {}) {
  const asked = core(said);
  render(<App />);
  await screen.findByTestId("pane");
  await settled();
  return asked;
}

beforeEach(() => inboxOpenAtLaunch());
afterEach(() => {
  forgetInboxOpen();
  cleanup();
  clearMocks();
  forgetDismissals();
});

describe("a running chat whose sandbox went stale as model keys arrived (R-775)", () => {
  it("the_stale_sandbox_notice_shows_with_restart_now_and_clears_at_restart", async () => {
    const { asked } = await aChat();
    await act(() => moved(RUNNING));
    await act(() => keysArrived());

    expect(await screen.findByText(windowCopy.W140)).toBeInTheDocument();
    expect(asked("restart_chat")).toEqual([]);

    await userEvent.click(screen.getByRole("button", { name: "Restart now" }));

    await waitFor(() => expect(asked("restart_chat")).toEqual(RESTARTED_NOW));
    expect(await screen.findByText("session 9")).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByText(windowCopy.W140)).toBeNull());
  });

  it("a_key_added_mid_turn_restarts_the_chat_only_at_its_next_pause", async () => {
    const { asked } = await aChat();
    await act(() => moved(RUNNING));
    await act(() => keysArrived());
    await settled();
    expect(asked("restart_chat")).toEqual([]);
    expect(await screen.findByText(windowCopy.W140)).toBeInTheDocument();

    await act(() => moved(WAITING));

    await waitFor(() => expect(asked("restart_chat")).toEqual(RESTARTED));
    await waitFor(() => expect(screen.queryByText(windowCopy.W140)).toBeNull());
  });

  it("shows W140 on a chat taken back at open that the core owes a restart for model keys", async () => {
    await aChat({ owed: [4], keys: [4] });
    await act(() => moved(RUNNING));

    expect(await screen.findByText(windowCopy.W140)).toBeInTheDocument();
  });
});
