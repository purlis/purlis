// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The band over an ended chat's last screen** (#4756): it says the chat ended, with Resume
 * where the chat has a conversation to start again on, and Clear pane. Drawn in every cell of
 * the theme matrix, at a pane's width.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn } from "storybook/test";
import { EndedBand } from "./EndedBand";
import { StateShown } from "./StateShown";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

function Band(props: Parameters<typeof EndedBand>[0]) {
  return (
    <div style={{ inlineSize: 640 }}>
      <EndedBand {...props} />
    </div>
  );
}

const meta = {
  component: Band,
  args: { session: 4, name: "steward 4", resume: fn(), onClear: fn() },
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
    await expect(args.resume).toHaveBeenCalled();
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

/** **A task a pane was left on, ended** (#4757): how it ended, as its finished row says it, the
 *  core's word beside it, and its report, as text. */
export const TaskWithReport: Story = {
  args: {
    name: "sweep",
    how: (
      <StateShown
        shown={{ kind: "failed", word: "failed", shape: "cross", token: "state.failed" }}
      />
    ),
    more: "blocked",
    report: "The queue is locked.\nNothing was changed.",
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByText(/sweep ended:/)).toBeVisible();
    await expect(canvas.getByRole("region", { name: "Report from sweep" })).toBeVisible();
  },
};
