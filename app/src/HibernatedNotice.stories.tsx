// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The band over a hibernated chat** (SC-4a, #7630): it says the chat is hibernated, and
 * offers Resume. Drawn in every cell of the theme matrix, at a
 * pane's width. Its baselines, in Chromium and WebKit, are drawn by a pull request's `stories`
 * run, in the pinned image.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn } from "storybook/test";
import { HibernatedBand } from "./HibernatedNotice";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

function Band(props: Parameters<typeof HibernatedBand>[0]) {
  return (
    <div style={{ inlineSize: 640 }}>
      <HibernatedBand {...props} />
    </div>
  );
}

const meta = {
  component: Band,
  args: { session: 4, onResume: fn() },
} satisfies Meta<typeof Band>;

export default meta;
type Story = StoryObj<typeof meta>;

// Its baselines are drawn by the `update-baselines` label's run, never by hand (#7659).
/** **A hibernated chat**: Resume, reached and pressed by the keyboard. */
export const Hibernated: Story = {
  play: async ({ canvas, args, userEvent }) => {
    await expect(canvas.getByText(/This chat is hibernated/)).toBeVisible();
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Resume" }), "Tab to Resume");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onResume).toHaveBeenCalled();
  },
};
