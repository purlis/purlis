// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { LaidOut, LaidTab, OpenChat } from "./bindings";

/**
 * **A pane's `chat 641` link, as the project window runs it** (#6217): the link's offer goes
 * through the pane's rows (`LayoutPanes`' `onPaneDoes`) and shows that chat, and only that
 * chat. It is not a needs-you item's Go: a failure the linked chat is flagged for is neither
 * gone to nor taken as looked at.
 *
 * The pane is stood in for by a button that presses what a ⌘-click on `chat 3` would press
 * (`PaneLinkChats.tsx`); the link itself is `SessionPane.chatLinks.test.tsx`'s.
 */
vi.mock("./SessionPane", async () => {
  const { showLinkedChatOffer } = await import("./actions");
  return {
    SessionPane: ({
      session,
      menu,
    }: {
      session: number;
      menu?: { onPress: (offer: ReturnType<typeof showLinkedChatOffer>) => void };
    }) => (
      <div data-testid="pane">
        session {session}
        <button type="button" onClick={() => menu?.onPress(showLinkedChatOffer(3))}>
          chat 3, in session {session}
        </button>
      </div>
    ),
  };
});

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const ONE = "/home/dev/one";

function chat(session: number): OpenChat {
  return {
    session,
    name: String(session),
    cwd: null,
    harness: null,
    in_front: false,
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

function tab(front: boolean, session: number): LaidTab {
  return {
    workspace: null,
    pinned: false,
    window: 0,
    front,
    focused: "",
    tree: { kind: "chat", chat: session },
  };
}

/** Chat 3's board: a task of its, whose chat 4 is still open, failed and nobody has looked. */
const FAILED_UNDER_3 = {
  plane: ONE,
  session: 3,
  state: "waiting",
  needs_you: false,
  queue: [],
  sequence: 1,
  moved_at: 1,
  reports: [],
  refusals: [],
  children: [],
  needs: [
    {
      kind: "task_failed",
      id: "failed-under-3",
      chat: 4,
      task: "check staging",
      how: "failed",
      why: "The cluster refused the login.",
    },
  ],
};

/** The project window, chats 1, 3 and 4 each in a tab, 1 in front; what it sent, by command. */
function aProjectWindow() {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  const layout: LaidOut = { strip: null, tabs: [tab(true, 1), tab(false, 3), tab(false, 4)] };
  const chats = [1, 3, 4];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "plugin:event|listen") return 1;
    if (cmd === "plane_at_launch") return { plane: ONE, from: ONE, why: null };
    if (cmd === "planes_to_restore") return { windows: [], dropped: [] };
    if (cmd === "tab_window_handed") return null;
    if (cmd === "charter_windows") return ["main"];
    if (cmd === "plane_sidebar")
      return {
        root: ONE,
        workspaces: [],
        personas: [],
        persona: null,
        unfiled: chats.map(chat),
      };
    if (cmd === "show_window_holding") return "main";
    if (cmd === "opened_chats") return chats.map(chat);
    if (cmd === "reopened_layout") return layout;
    if (cmd === "chat_states") return [FAILED_UNDER_3];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
    if (cmd === "tab_windows_back") return 0;
    return null;
  });
  return (cmd: string) => asked.filter((one) => one.cmd === cmd);
}

describe("a pane's chat link, in the project window", () => {
  it("shows the chat it names, never where its failed task is, and leaves the failure unseen", async () => {
    // In the background, so a chat coming on screen is not looked at by being there (#5340):
    // what is asserted is what the link alone does.
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    const sent = aProjectWindow();
    render(<App />);
    await screen.findByText("session 1");

    await userEvent.click(screen.getByRole("button", { name: "chat 3, in session 1" }));

    // Chat 3 comes to the front: not chat 4, where its failed task is.
    await screen.findByText("session 3");
    expect(screen.queryByText("session 4")).toBeNull();
    // And its failure is still to be looked at.
    await new Promise((settle) => setTimeout(settle, 50));
    expect(sent("task_failure_seen")).toEqual([]);
  });
});
