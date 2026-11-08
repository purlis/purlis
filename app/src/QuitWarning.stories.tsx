// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The quit warning, and Restart to update's ask** (#5748): each lists a chat whose turn ended on
 * work of its own still in flight, with its hourglass, and says what of that work ends, beside a
 * chat mid-turn. Pictured in every cell while the question is up; both still go ahead when the
 * person answers so.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";
import { MidTurn, QuitWarning, type Ending } from "./QuitWarning";
import { expectFocusInSight, pictureNow, untilFocused } from "./stories.testkit";

const CHATS: Ending[] = [
  {
    key: "1",
    name: "steward 1",
    harness: "claude",
    cwd: "~/work/smart-ide",
    state: "waiting",
    ownWork: { helpers: 0, workflows: 0, monitors: 1, shells: 2 },
  },
  {
    key: "2",
    name: "steward 2",
    harness: "claude",
    cwd: "~/work/smart-ide",
    state: "running",
  },
  {
    key: "3",
    name: "steward 3",
    harness: "claude",
    cwd: "~/work/smart-ide",
    state: "waiting",
  },
];

const meta = {
  title: "QuitWarning",
  component: QuitWarning,
  // The dialog draws over the page, outside the story's frame.
  parameters: { screenshot: "page" },
  args: { chats: CHATS, onQuit: fn(), onCancel: fn() },
} satisfies Meta<typeof QuitWarning>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Quitting over own work**: said, and Quit purlis still quits. Esc is Cancel. */
export const OwnWorkEnds: Story = {
  play: async ({ args }) => {
    const page = within(document.body);
    const dialog = await page.findByRole("alertdialog", { name: "Quitting ends 3 chats" });
    await expect(dialog).toHaveTextContent(
      "steward 1 is waiting on its own work: 1 monitor and 2 background shells will end.",
    );
    const cancel = within(dialog).getByRole("button", { name: "Cancel" });
    await untilFocused(() => cancel, "Quit warning: the keyboard starts on Cancel");
    expectFocusInSight();
    await pictureNow();
    await userEvent.tab();
    await untilFocused(
      () => within(dialog).getByRole("button", { name: "Quit purlis" }),
      "Quit warning: Tab reaches Quit purlis",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.onQuit).toHaveBeenCalledOnce());
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(args.onCancel).toHaveBeenCalledOnce());
  },
};

const onWait = fn();
const onRestart = fn();

/** **Restart to update over own work**: the same lines; Restart now still restarts, Esc waits. */
export const RestartToUpdateOwnWorkEnds: Story = {
  render: () => <MidTurn chats={CHATS.slice(0, 2)} onWait={onWait} onRestart={onRestart} />,
  play: async () => {
    onWait.mockClear();
    onRestart.mockClear();
    const page = within(document.body);
    const dialog = await page.findByRole("alertdialog", { name: "Restart to update" });
    await expect(dialog).toHaveTextContent(
      "steward 1 is waiting on its own work: 1 monitor and 2 background shells will end.",
    );
    const wait = within(dialog).getByRole("button", { name: "Wait" });
    await untilFocused(() => wait, "Restart to update: the keyboard starts on Wait");
    expectFocusInSight();
    await pictureNow();
    await userEvent.tab();
    await untilFocused(
      () => within(dialog).getByRole("button", { name: "Restart now" }),
      "Restart to update: Tab reaches Restart now",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(onRestart).toHaveBeenCalledOnce());
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(onWait).toHaveBeenCalledOnce());
  },
};
