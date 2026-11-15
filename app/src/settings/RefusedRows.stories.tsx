// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A Blocked log row's way to its chat's Chat log** (R-226, T1-37): a row whose chat is open
 * here offers "Open the chat log", which opens it at its blocks; a row of a chat that is not open
 * here offers none. Drawn in every cell of the theme matrix.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { BlockedLately } from "../bindings";
import { BlockedMarkers, type ChatLogFilter } from "../blockedMarkers";
import { expectFocusInSight, tabInto } from "../stories.testkit";
import { RefusedRows } from "./NetworkLists";

const blocked = (over: Partial<BlockedLately>): BlockedLately => ({
  target: "db.example.com:5432",
  looked_up: null,
  said: "a connection to an internet host this project does not allow",
  chat: "fix the build",
  session: 4,
  at: 1_790_000_000,
  times: 3,
  reached: false,
  levels: [],
  ...over,
});

function BlockedLog({
  rows,
  open,
}: {
  rows: readonly BlockedLately[];
  /** Opens chat `session`'s Chat log, as the window does (R-226). */
  open: (session: number, filter?: ChatLogFilter) => void;
}) {
  return (
    <div style={{ inlineSize: 480 }}>
      <BlockedMarkers.Provider value={{ of: () => undefined, open }}>
        <RefusedRows plane="/home/op/project" rows={rows} label="Blocked log" />
      </BlockedMarkers.Provider>
    </div>
  );
}

const meta = {
  component: BlockedLog,
  args: { rows: [], open: fn() },
  beforeEach: () => {
    mockIPC(() => null);
    return () => clearMocks();
  },
} satisfies Meta<typeof BlockedLog>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **A Blocked log row opens its chat's Chat log** (R-226): pressed from the keyboard, at its
 * blocks; the row of a chat not open here offers no way to it.
 */
export const ABlockedLogRowOpensItsChatLog: Story = {
  args: {
    rows: [
      blocked({}),
      blocked({ target: "cache.example.com:6379", chat: "closed yesterday", session: null }),
    ],
  },
  play: async ({ canvas, args, userEvent }) => {
    const way = await canvas.findByRole("button", { name: "Open the chat log of fix the build" });
    await expect(canvas.getAllByRole("button", { name: /^Open the chat log/ })).toHaveLength(1);
    await tabInto(() => way);
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.open).toHaveBeenCalledWith(4, "blocks");
  },
};
