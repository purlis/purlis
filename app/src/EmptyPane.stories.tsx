// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A pane that shows nothing** (#4755): its headline, how many chats run here, and New chat,
 * alone in its tab and as one side of a split. Drawn in every cell of the theme matrix.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn } from "storybook/test";
import type { Offer } from "./actions";
import { EmptyPane } from "./EmptyPane";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const NEW_CHAT: Offer = {
  id: "chat.new",
  title: "New tab",
  available: true,
  reason: "",
  does: { verb: "chat.new" },
};

function Pane(props: Parameters<typeof EmptyPane>[0]) {
  return (
    <div style={{ inlineSize: 640, blockSize: 360, display: "grid" }}>
      <EmptyPane {...props} />
    </div>
  );
}

const meta = {
  component: Pane,
  args: { running: 3, newChat: NEW_CHAT, onPress: fn(), inSplit: false },
} satisfies Meta<typeof Pane>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **A tab's only pane, with chats running here**: New chat is reached and pressed by the
 *  keyboard. */
export const Alone: Story = {
  play: async ({ canvas, args, userEvent }) => {
    await expect(canvas.getByText("Nothing in this pane")).toBeVisible();
    await expect(canvas.getByText("3 chats are running here.")).toBeVisible();
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "New chat" }), "Tab to New chat");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onPress).toHaveBeenCalledWith(NEW_CHAT);
  },
};

/** **One side of a split**, in a workspace where nothing runs yet. */
export const InASplitWithNothingRunning: Story = {
  args: { running: 0, inSplit: true },
  play: async ({ canvas }) => {
    await expect(canvas.getByText("An empty pane")).toBeVisible();
    await expect(canvas.getByText("Nothing runs in this workspace yet.")).toBeVisible();
  },
};
