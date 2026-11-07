// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a write on the person's own workspace files asks first** (#5376): a new file's name,
 * a delete's one confirmation, and the question before renaming a file a chat is writing, each
 * pictured in every cell while it is up. The core's refusal is drawn beside the box, as the
 * person is still answering.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";
import { WorkspaceWriteDialog } from "./WorkspaceWrites";
import { expectFocusInSight, pictureNow, untilFocused } from "./stories.testkit";

const notes = { workspace: "alpha", repo: "", piece: null, path: "notes" };
const plan = { ...notes, path: "notes/plan.md" };

const meta = {
  title: "WorkspaceWrites",
  component: WorkspaceWriteDialog,
  // The dialog draws over the page, outside the story's frame.
  parameters: { screenshot: "page" },
  args: {
    asking: { does: { verb: "newWorkspaceEntry", at: notes, folder: false }, busy: false },
    onName: fn(),
    onGoOn: fn(),
    onTrash: fn(),
    onClose: fn(),
  },
} satisfies Meta<typeof WorkspaceWriteDialog>;

export default meta;
type Story = StoryObj<typeof meta>;

export const NewFile: Story = {
  play: async ({ args }) => {
    const page = within(document.body);
    const box = await page.findByRole("textbox", { name: "File name" });
    await untilFocused(() => box, "New file: the keyboard starts in the name's box");
    expectFocusInSight();
    await userEvent.keyboard("today.md");
    await pictureNow();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.onName).toHaveBeenCalledWith("today.md"));
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(args.onClose).toHaveBeenCalledOnce());
  },
};

export const RefusedName: Story = {
  args: {
    asking: {
      does: { verb: "newWorkspaceEntry", at: notes, folder: true },
      busy: false,
      trouble: "'drafts' is already taken in this folder. Choose another name.",
    },
  },
  play: async ({ args }) => {
    const page = within(document.body);
    expect(await page.findByRole("alert")).toHaveTextContent("already taken");
    const box = page.getByRole("textbox", { name: "Folder name" });
    await untilFocused(() => box, "A refused name: the keyboard stays in the box");
    expectFocusInSight();
    await pictureNow();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(args.onClose).toHaveBeenCalledOnce());
  },
};

export const DeleteToTheTrash: Story = {
  args: {
    asking: {
      does: { verb: "trashWorkspaceEntry", at: plan, folder: false, touchedBy: ["steward 3"] },
      busy: false,
    },
  },
  play: async ({ args }) => {
    const page = within(document.body);
    const cancel = await page.findByRole("button", { name: "Cancel" });
    await untilFocused(() => cancel, "Delete: the keyboard starts on Cancel");
    expectFocusInSight();
    await pictureNow();
    await userEvent.tab();
    await untilFocused(
      () => page.getByRole("button", { name: "Move to Trash" }),
      "Delete: Tab reaches Move to Trash",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.onTrash).toHaveBeenCalledOnce());
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(args.onClose).toHaveBeenCalledOnce());
  },
};

export const RenameWhileAChatWrites: Story = {
  args: {
    asking: {
      does: { verb: "renameWorkspaceEntry", at: plan, folder: false, touchedBy: ["steward 3"] },
      busy: false,
    },
  },
  play: async ({ args }) => {
    const page = within(document.body);
    const cancel = await page.findByRole("button", { name: "Cancel" });
    await untilFocused(() => cancel, "Rename while a chat writes: the keyboard starts on Cancel");
    expectFocusInSight();
    await pictureNow();
    await userEvent.tab();
    await untilFocused(
      () => page.getByRole("button", { name: "Rename anyway…" }),
      "Rename while a chat writes: Tab reaches Rename anyway…",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.onGoOn).toHaveBeenCalledOnce());
  },
};
