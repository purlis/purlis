// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";
import { RenameTab } from "./RenameTab";
import { untilFocused } from "./stories.testkit";

/**
 * **A tab's Rename, in each of its states** (#5343): a tab with one chat, which renames the
 * chat; the same tab with a name of its own, ticked, with the way back to the chat's name; a
 * split, which names the tab only; and the core's refusal.
 */
const meta: Meta<typeof RenameTab> = {
  title: "RenameTab",
  component: RenameTab,
  // The dialog draws over the page, outside the story's frame.
  parameters: { screenshot: "page" },
  args: {
    renames: { kind: "chat", session: 4 },
    name: "steward 4",
    chat: "steward 4",
    own: false,
    busy: false,
    onRename: fn(),
    onClear: fn(),
    onCancel: fn(),
  },
};
export default meta;

type Story = StoryObj<typeof RenameTab>;

/** The keyboard starts in the box; Tab reaches the tick, and Space names only the tab. */
export const OneChat: Story = {
  play: async ({ canvasElement, args }) => {
    const page = within(canvasElement.ownerDocument.body);
    const box = await page.findByLabelText("New name");
    await untilFocused(() => box, "It starts in the box");
    await userEvent.tab();
    const only = page.getByRole("checkbox", { name: "Only this tab" });
    await untilFocused(() => only, "Tab reaches Only this tab");
    await userEvent.keyboard(" ");
    await expect(only).toBeChecked();
    await expect(page.getByRole("button", { name: "Rename tab" })).toBeEnabled();
    await userEvent.keyboard(" ");
    await expect(only).not.toBeChecked();
    await userEvent.keyboard("{Escape}");
    await expect(args.onCancel).toHaveBeenCalled();
  },
};

/** A tab with a name of its own: it opens ticked, on that name, with Use the chat name again. */
export const OwnName: Story = {
  args: { name: "the build", own: true },
  play: async ({ canvasElement, args }) => {
    const page = within(canvasElement.ownerDocument.body);
    const box = await page.findByLabelText("New name");
    await untilFocused(() => box, "It starts in the box");
    await userEvent.tab();
    await untilFocused(
      () => page.getByRole("checkbox", { name: "Only this tab" }),
      "Tab reaches Only this tab",
    );
    await userEvent.tab();
    await userEvent.tab();
    const clear = page.getByRole("button", { name: "Use the chat name again" });
    await untilFocused(() => clear, "Tab reaches Use the chat name again, after Cancel");
    await userEvent.keyboard("{Enter}");
    await expect(args.onClear).toHaveBeenCalled();
  },
};

/** A split tab: it names the tab only, and has no tick. */
export const Split: Story = {
  args: { renames: { kind: "tab", panes: 2 } },
  play: async ({ canvasElement, args }) => {
    const page = within(canvasElement.ownerDocument.body);
    const box = await page.findByLabelText("New name");
    await untilFocused(() => box, "It starts in the box");
    await userEvent.keyboard("the build{Enter}");
    await expect(args.onRename).toHaveBeenCalledWith({ typed: "the build", only: true });
  },
};

/** A split tab with a name of its own: no tick, and the way back to its first chat's name. */
export const SplitOwnName: Story = {
  args: { renames: { kind: "tab", panes: 3 }, name: "the build", own: true },
  play: async ({ canvasElement, args }) => {
    const page = within(canvasElement.ownerDocument.body);
    const box = await page.findByLabelText("New name");
    await untilFocused(() => box, "It starts in the box");
    await expect(page.queryByRole("checkbox")).toBeNull();
    await userEvent.tab();
    await userEvent.tab();
    const clear = page.getByRole("button", { name: "Use the chat name again" });
    await untilFocused(() => clear, "Tab reaches Use the chat name again, after Cancel");
    await userEvent.keyboard("{Enter}");
    await expect(args.onClear).toHaveBeenCalled();
  },
};

/** The core would not hold the name: its words, beside the box. */
export const Refused: Story = {
  args: { trouble: "That name is 80 characters long, and a chat's name is at most 64." },
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    await expect(await page.findByRole("alert")).toHaveTextContent(/at most 64/);
    await untilFocused(() => page.getByLabelText("New name"), "It starts in the box");
  },
};
