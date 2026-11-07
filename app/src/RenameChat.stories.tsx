// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";
import { RenameChat } from "./RenameChat";
import { untilFocused } from "./stories.testkit";

/** **Rename chat** (#5343): from a pane's ⋯, the list row's menu, the palette or F2 on a row. */
const meta: Meta<typeof RenameChat> = {
  title: "RenameChat",
  component: RenameChat,
  // The dialog draws over the page, outside the story's frame.
  parameters: { screenshot: "page" },
  args: { name: "devops 2", renaming: false, onRename: fn(), onCancel: fn() },
};
export default meta;

/** It says it renames the chat everywhere; Enter renames, Esc changes nothing. */
export const Asked: StoryObj<typeof RenameChat> = {
  play: async ({ canvasElement, args }) => {
    const page = within(canvasElement.ownerDocument.body);
    const box = await page.findByLabelText("New name");
    await untilFocused(() => box, "It starts in the box");
    await userEvent.keyboard("builder{Enter}");
    await expect(args.onRename).toHaveBeenCalledWith("builder");
  },
};
