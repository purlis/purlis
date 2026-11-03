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
import { emit } from "@tauri-apps/api/event";
import App from "./App";
import type { Moved } from "./bindings";
import { forgetDismissals } from "./dismissals";
import { forgetInboxOpen, inboxOpenAtLaunch } from "./test-inbox";

/**
 * **A chat whose program ended keeps its pane** (#4756, spec #4750 stories 21 and 93), against
 * the whole window: its last screen stays, read-only, under an ended band with Resume and Clear
 * pane; a reader hears it once; the keyboard stays where it was; and nothing ends a chat a second
 * time.
 */

/** Each pane says which chat it draws, and whether its screen takes input. */
vi.mock("./SessionPane", () => ({
  SessionPane: ({ session, readOnly }: { session: number; readOnly?: boolean }) => (
    <div data-testid="pane" data-read-only={readOnly ? "" : undefined}>
      session {session}
    </div>
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

const WAITING: Moved = {
  plane: PLANE,
  session: 4,
  state: "waiting",
  needs_you: false,
  queue: [],
  moved_at: 1,
  reports: [],
  refusals: [],
  children: [],
  sequence: 1,
};
/** Its program ended, content: what the core says when the session itself ends. */
const DONE: Moved = { ...WAITING, state: "done", moved_at: 2, sequence: 2 };

function core() {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC(
    (cmd, args) => {
      asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [CHAT];
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
      if (cmd === "owed_restarts") return [];
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

/** Chat 4 on screen, then its program ends. */
async function anEndedChat() {
  const said = core();
  render(<App />);
  await screen.findByTestId("pane");
  await settled();
  await act(() => emit("chat-moved", WAITING));
  return said;
}

const band = () => screen.findByRole("status", { name: /ended/i });

beforeEach(() => inboxOpenAtLaunch());
afterEach(() => {
  cleanup();
  clearMocks();
  forgetDismissals();
  forgetInboxOpen();
});

describe("a chat whose program ended (#4756)", () => {
  it("keeps its pane and its last screen, read-only, under an ended band", async () => {
    const { asked } = await anEndedChat();
    expect(screen.getByTestId("pane")).not.toHaveAttribute("data-read-only");

    await act(() => emit("chat-moved", DONE));

    const ended = await band();
    expect(ended).toHaveTextContent("claude 4 ended. Resume or Clear pane.");
    expect(within(ended).getByRole("button", { name: "Resume" })).toBeInTheDocument();
    expect(within(ended).getByRole("button", { name: "Clear pane" })).toBeInTheDocument();
    expect(screen.getByText("session 4")).toHaveAttribute("data-read-only");
    expect(asked("close_session")).toEqual([]);
  });

  it("leaves the keyboard where it was as the band comes", async () => {
    await anEndedChat();
    const before = document.activeElement;

    await act(() => emit("chat-moved", DONE));
    await band();

    expect(document.activeElement).toBe(before);
  });

  it("clears the pane on Clear pane, and ends nothing", async () => {
    const { asked } = await anEndedChat();
    await act(() => emit("chat-moved", DONE));

    await userEvent.click(within(await band()).getByRole("button", { name: "Clear pane" }));

    expect(await screen.findByText("Nothing in this pane")).toBeInTheDocument();
    expect(screen.queryByTestId("pane")).toBeNull();
    expect(asked("close_session")).toEqual([]);
  });

  it("starts it again on its conversation on Resume, in its pane", async () => {
    const { asked } = await anEndedChat();
    await act(() => emit("chat-moved", DONE));

    await userEvent.click(within(await band()).getByRole("button", { name: "Resume" }));

    await waitFor(() => expect(asked("restart_chat")).toHaveLength(1));
    expect(await screen.findByText("session 9")).toBeInTheDocument();
  });

  it("draws no band while the chat runs or waits", async () => {
    await anEndedChat();

    expect(screen.queryByRole("status", { name: /ended/i })).toBeNull();
  });
});
