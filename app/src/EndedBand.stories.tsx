// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The band over an ended chat's last screen** (#4756): it says the chat ended, with Resume
 * where the chat has a conversation to start again on, and Clear pane. Drawn in every cell of
 * the theme matrix, at a pane's width.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn } from "storybook/test";
import type { Offer } from "./actions";
import { EndedBand } from "./EndedBand";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const RESTART: Offer = {
  id: "chat.restart:4",
  title: "Restart chat steward 4",
  available: true,
  reason: "",
  does: { verb: "restartChat", session: 4 },
};

function Band(props: Parameters<typeof EndedBand>[0]) {
  return (
    <div style={{ inlineSize: 640 }}>
      <EndedBand {...props} />
    </div>
  );
}

const meta = {
  component: Band,
  args: { session: 4, name: "steward 4", resume: RESTART, onResume: fn(), onClear: fn() },
} satisfies Meta<typeof Band>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **A chat with a conversation**: Resume, then Clear pane, each reached and pressed by the
 *  keyboard. */
export const WithResume: Story = {
  play: async ({ canvas, args, userEvent }) => {
    await expect(canvas.getByText(/steward 4 ended\./)).toBeVisible();
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Resume" }), "Tab to Resume");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onResume).toHaveBeenCalledWith(RESTART);
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Clear pane" }),
      "Tab to Clear pane",
    );
    await userEvent.keyboard("{Enter}");
    await expect(args.onClear).toHaveBeenCalled();
  },
};

/** **A chat with nothing to resume**, such as a shell: Clear pane alone. */
export const ClearPaneOnly: Story = {
  args: { resume: undefined },
  play: async ({ canvas, userEvent }) => {
    await expect(canvas.queryByRole("button", { name: "Resume" })).toBeNull();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Clear pane" }),
      "Tab to Clear pane",
    );
    expectFocusInSight();
  },
};
