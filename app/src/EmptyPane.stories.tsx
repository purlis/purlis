// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A pane that shows nothing** (#4755, #5292): it says it holds nothing yet, offers New chat and
 * New shell, says a chat can be dragged in and how many chats run here. Alone in its tab and as
 * one side of a split it says the same. Drawn in every cell of the theme matrix.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn } from "storybook/test";
import type { Offer } from "./actions";
import { EmptyPane } from "./EmptyPane";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const NEW_CHAT: Offer = {
  id: "chat.new",
  title: "New chat",
  available: true,
  reason: "",
  does: { verb: "chat.new" },
};

const NEW_SHELL: Offer = {
  id: "shell.new",
  title: "New shell",
  available: true,
  reason: "",
  does: { verb: "newShell" },
};

const CREATE_WORKSPACE: Offer = {
  id: "workspace.create",
  title: "Create a workspace…",
  available: true,
  reason: "",
  does: { verb: "createWorkspace" },
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
  args: { running: 3, newChat: NEW_CHAT, newShell: NEW_SHELL, onPress: fn() },
} satisfies Meta<typeof Pane>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **With chats running here**: it says one can be dragged in. New
 *  chat and New shell are reached and pressed by the keyboard. */
export const Alone: Story = {
  play: async ({ canvas, args, userEvent }) => {
    await expect(canvas.getByText("Nothing in this pane yet")).toBeVisible();
    await expect(
      canvas.getByText(
        "Drag a chat here from the sidebar, or start one. 3 chats are running here.",
      ),
    ).toBeVisible();
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "New chat" }), "Tab to New chat");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onPress).toHaveBeenCalledWith(NEW_CHAT);
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "New shell" }), "Tab to New shell");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onPress).toHaveBeenCalledWith(NEW_SHELL);
    // Off both, so the picture holds no ring.
    await userEvent.tab({ shift: true });
    await userEvent.tab({ shift: true });
    await expect(canvas.getByRole("button", { name: "New shell" })).not.toHaveFocus();
    await expect(canvas.getByRole("button", { name: "New chat" })).not.toHaveFocus();
  },
};

/** **In a workspace where nothing runs yet**: no chat to drag, so it says only that. */
export const NothingRunningYet: Story = {
  args: { running: 0 },
  play: async ({ canvas, userEvent }) => {
    await expect(canvas.getByText("Nothing in this pane yet")).toBeVisible();
    await expect(canvas.getByText("Nothing runs in this workspace yet.")).toBeVisible();
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "New chat" }), "Tab to New chat");
    expectFocusInSight();
    await userEvent.tab({ shift: true });
    await expect(canvas.getByRole("button", { name: "New chat" })).not.toHaveFocus();
  },
};

/** **A plane with no workspace yet** (#4758; spec story 11): Create a workspace comes first,
 *  then New chat, both reached by the keyboard. */
export const NoWorkspaceYet: Story = {
  args: { running: 0, createWorkspace: CREATE_WORKSPACE },
  play: async ({ canvas, args, userEvent }) => {
    await expect(canvas.getByText("No workspaces yet")).toBeVisible();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Create a workspace" }),
      "Tab to Create a workspace",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onPress).toHaveBeenCalledWith(CREATE_WORKSPACE);
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "New chat" }), "Tab to New chat");
  },
};
