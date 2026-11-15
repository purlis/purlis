// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A session record's way to its chat's Chat log** (R-226, T1-37): where the chat that wrote
 * the record is open here, "Open the chat log" follows the record's facts and opens it whole.
 * Drawn in every cell of the theme matrix.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SessionRecordView } from "./bindings";
import { BlockedMarkers, type ChatLogFilter } from "./blockedMarkers";
import { SessionRecordTab } from "./SessionRecordTab";
import { expectFocusInSight, tabInto } from "./stories.testkit";

const PATH = "workspaces/alpha/sessions/20261113-100000-ship-the-widget.md";

/** The record chat 7 wrote, as `session_record` answers it: the chat is open here. */
const RECORD: SessionRecordView = {
  row: {
    path: PATH,
    title: "Ship the widget",
    when: "2026-11-13 10:00",
    persona: "devops",
    harness: "claude",
    resumable: false,
  },
  place: "alpha",
  body: "# Ship the widget\n\n## Goal\n\nShip it.\n",
  persona_hosts: [],
  resume_holds: false,
  persona_hosts_locked: null,
  persona_hosts_wait: false,
  dispatches: [],
  session: 7,
};

function Recorded({
  open,
}: {
  /** Opens chat `session`'s Chat log, as the window does (R-226). */
  open: (session: number, filter?: ChatLogFilter) => void;
}) {
  return (
    <div style={{ inlineSize: 520 }}>
      <BlockedMarkers.Provider value={{ of: () => undefined, open }}>
        <SessionRecordTab
          plane="/home/op/project"
          path={PATH}
          chatNamed={(session) => (session === 7 ? "talk" : undefined)}
        />
      </BlockedMarkers.Provider>
    </div>
  );
}

const meta = {
  component: Recorded,
  args: { open: fn() },
  beforeEach: () => {
    mockIPC((cmd, args) => {
      if (cmd === "session_record") return RECORD;
      if (cmd === "workspace_repos")
        return {
          workspace: (args as { workspace: string }).workspace,
          repos: [],
          cache_refused: null,
        };
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<typeof Recorded>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **A session record opens its chat's Chat log** (R-226): pressed from the keyboard, whole. */
export const ASessionRecordOpensItsChatLog: Story = {
  play: async ({ canvas, args, userEvent }) => {
    const way = await canvas.findByRole("button", { name: "Open the chat log of talk" });
    await tabInto(() => way);
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.open).toHaveBeenCalledWith(7, "all");
  },
};
