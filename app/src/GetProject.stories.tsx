// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Get project's clone progress** (#3731): the meter family's bar, git's stage its name and the
 * percent its value, with Cancel beside it. Held to axe, walked by the keyboard, and pictured
 * part-way through in every pictured theme cell.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn } from "storybook/test";
import { CloneProgress } from "./GetProject";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const meta = {
  component: CloneProgress,
  args: { stage: "Receiving objects", percent: 45, onCancel: fn() },
  decorators: [
    (Story) => (
      <div style={{ inlineSize: "36rem", maxInlineSize: "100%" }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof CloneProgress>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Part-way**: the stage names the bar, its value is the percent, and Tab reaches Cancel. */
export const PartWay: Story = {
  play: async ({ canvas, userEvent, args }) => {
    const bar = canvas.getByRole("progressbar", { name: "Receiving objects" });
    await expect(bar).toHaveAttribute("aria-valuenow", "45");
    const cancel = canvas.getByRole("button", { name: "Cancel" });
    await userEvent.tab();
    await untilFocused(() => cancel, "PartWay: Tab lands on Cancel");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onCancel).toHaveBeenCalledOnce();
  },
};

/** **Before git gives a percent**: the bar goes with no end known. No picture: its stripes move. */
export const Connecting: Story = {
  args: { stage: "Connecting", percent: null },
  parameters: { screenshot: false },
  play: async ({ canvas }) => {
    const bar = canvas.getByRole("progressbar", { name: "Connecting" });
    await expect(bar).not.toHaveAttribute("aria-valuenow");
  },
};
