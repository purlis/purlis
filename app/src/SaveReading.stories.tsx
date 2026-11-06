// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The title bar's save indicator while the project's standing is still coming** (#5325): its
 * own mark, in words as well as motion, and a way to the Saving tab, which says the same.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";
import { SaveReading } from "./SavingView";
import { expectFocusInSight } from "./stories.testkit";

const meta = {
  component: SaveReading,
  args: { onOpen: fn() },
} satisfies Meta<typeof SaveReading>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Tab lands on it and Enter opens the Saving tab. */
export const Reading: Story = {
  play: async ({ canvasElement, args }) => {
    const mark = within(canvasElement).getByRole("button", {
      name: "Saving: reading the project",
    });
    await userEvent.tab();
    await expect(mark).toHaveFocus();
    await expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onOpen).toHaveBeenCalledTimes(1);
  },
};
