// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **PROTOTYPE, NOT FOR MERGE: Asks scoped to Chat, Workspace, Project.** The scope chips, their
 * counts and the count's one slide, with the controls that move them (`AsksScopes.prototype.tsx`).
 * Light and dark are the toolbar's Appearance.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { AsksScopesPrototype, type Counts } from "./AsksScopes.prototype";

const meta = {
  title: "Prototype/Asks scopes",
  component: AsksScopesPrototype,
} satisfies Meta<typeof AsksScopesPrototype>;

export default meta;
type Story = StoryObj<typeof meta>;

const SOME: Counts = {
  chat: 1,
  workspaceRest: 2,
  otherWorkspaces: 1,
  others: { website: 2, infra: 0 },
};

/** Something waits in every scope: the chips read 1, 3, 4, and the ✋ 6. */
export const SomethingWaitsEverywhere: Story = {
  args: { start: SOME },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("radio", { name: "Chat, 1 waiting on you" })).toBeInTheDocument();
    await userEvent.click(canvas.getByRole("button", { name: "Chat +1" }));
    await waitFor(() =>
      expect(canvas.getByRole("radio", { name: "Chat, 2 waiting on you" })).toBeInTheDocument(),
    );
    await expect(
      canvas.getByRole("radio", { name: "Workspace, 4 waiting on you" }),
    ).toBeInTheDocument();
  },
};

/** Nothing in the chat: Chat opens empty and offers the workspace's, and never switches. */
export const TheChatIsEmpty: Story = {
  args: { start: { ...SOME, chat: 0 } },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("radio", { name: "Chat" })).toBeChecked();
    await userEvent.click(canvas.getByRole("button", { name: "2 waiting in this workspace ›" }));
    await waitFor(() =>
      expect(canvas.getByRole("radio", { name: "Workspace, 2 waiting on you" })).toBeChecked(),
    );
  },
};

/** One workspace: no Workspace chip. */
export const OneWorkspace: Story = {
  args: { start: SOME },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("switch", { name: "One workspace" }));
    await waitFor(() =>
      expect(canvas.queryByRole("radio", { name: /^Workspace/ })).not.toBeInTheDocument(),
    );
  },
};

/** Nothing waits anywhere: every chip, tab and the ✋ show no number. */
export const NothingWaits: Story = {
  args: {
    start: { chat: 0, workspaceRest: 0, otherWorkspaces: 0, others: { website: 0, infra: 0 } },
  },
};
