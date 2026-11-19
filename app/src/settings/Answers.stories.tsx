// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The sealed answers and the device id in Settings' Sandbox group** (M75-07a, #7015): where the
 * answers stand and this machine's id as a label, with Confirm while it waits. Fixture data,
 * never a live host. Confirm is reached by Tab.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent } from "storybook/test";
import { AnswersView } from "./Answers";
import { DENIED, HOLDS, WAITING_FOR_CONFIRM } from "./answersFixture";
import { expectFocusInSight, untilFocused } from "../stories.testkit";

const meta = {
  component: AnswersView,
  args: {
    shown: HOLDS,
    busy: false,
    onConfirm: fn(),
    ids: { id: "sealed-answers", labelledBy: undefined },
  },
  parameters: { screenshot: "page" },
} satisfies Meta<typeof AnswersView>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Sealed and confirmed: the id is a label, nothing to press. */
export const Confirmed: Story = {
  play: async ({ canvas }) => {
    await expect(canvas.getByText(/Rules can name it/)).toBeVisible();
    await expect(canvas.queryByRole("button")).toBeNull();
  },
};

/** Sealed and waiting: Confirm names the id shown, and the keyboard reaches it. */
export const WaitingForConfirm: Story = {
  args: { shown: WAITING_FOR_CONFIRM },
  play: async ({ canvas, args }) => {
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Confirm device id" }),
      "the keyboard reaches Confirm",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onConfirm).toHaveBeenCalledWith(WAITING_FOR_CONFIRM.device.id);
  },
};

/** The deny state, with no id to confirm. */
export const Denied: Story = {
  args: { shown: DENIED },
  play: async ({ canvas }) => {
    await expect(canvas.getByText(/deny state/)).toBeVisible();
    await expect(canvas.queryByRole("button")).toBeNull();
  },
};
