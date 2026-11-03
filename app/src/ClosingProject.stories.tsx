// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The question before a project's chats end** (#3182): its sentence on `danger.wash`, which no
 * other story drew, and a waiting chat's ring, each pictured in every cell, Purlis light's among
 * them. The picture is taken while the question is up: Escape answers it.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";
import { ClosingProject } from "./ClosingProject";
import { expectFocusInSight, pictureNow, untilFocused } from "./stories.testkit";

const meta = {
  title: "ClosingProject",
  component: ClosingProject,
  // The dialog draws over the page, outside the story's frame.
  parameters: { screenshot: "page" },
  args: {
    name: "smart-ide",
    heard: true,
    onClose: fn(),
    onCancel: fn(),
    chats: [
      {
        key: "1",
        name: "steward 1",
        harness: "claude",
        cwd: "~/work/smart-ide",
        workspace: "smart-ide",
        state: "waiting",
      },
      {
        key: "2",
        name: "steward 2",
        harness: "codex",
        cwd: "~/work/smart-ide",
        workspace: "smart-ide",
        state: "failed",
      },
    ],
  },
} satisfies Meta<typeof ClosingProject>;

export default meta;
type Story = StoryObj<typeof meta>;

export const ChatsEnd: Story = {
  play: async ({ args }) => {
    // A dialog is drawn over the page, outside the story's page.
    const page = within(document.body);
    const cancel = await page.findByRole("button", { name: "Cancel" });
    await untilFocused(() => cancel, "Closing a project: the keyboard starts on Cancel");
    expectFocusInSight();
    await pictureNow();
    await userEvent.tab();
    await untilFocused(
      () => page.getByRole("button", { name: "Close and end 2 chats" }),
      "Closing a project: Tab reaches its act",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.onClose).toHaveBeenCalledOnce());
    await userEvent.keyboard("{Shift>}{Tab}{/Shift}");
    await untilFocused(() => cancel, "Closing a project: Shift+Tab goes back to Cancel");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(args.onCancel).toHaveBeenCalledOnce());
  },
};
