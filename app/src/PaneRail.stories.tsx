// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A folded side of a split** (#4766): a rail beside the pane that kept the room, and a bar
 * above or below it, each pane with its icon, its notes count and ✋ while it asks. Drawn in
 * every cell of the theme matrix.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Puzzle, SquareTerminal } from "lucide-react";
import { expect, fn } from "storybook/test";
import { PaneRail, type RailPane } from "./PaneRail";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const FOLDED: RailPane[] = [
  {
    pane: 2,
    name: "devops 2",
    mark: <SquareTerminal aria-hidden="true" />,
    holds: { notes: 3, trouble: 1, asks: 0 },
    asks: false,
  },
  {
    pane: 3,
    name: "triage",
    mark: <SquareTerminal aria-hidden="true" />,
    holds: { notes: 0, trouble: 0, asks: 1 },
    asks: true,
  },
  {
    pane: 4,
    name: "Changes",
    mark: <Puzzle aria-hidden="true" />,
    holds: { notes: 0, trouble: 0, asks: 0 },
    asks: false,
  },
];

function Folded(props: Parameters<typeof PaneRail>[0]) {
  return (
    <div
      style={{
        inlineSize: props.as === "rail" ? 40 : 640,
        blockSize: props.as === "rail" ? 360 : 32,
        display: "flex",
      }}
    >
      <PaneRail {...props} />
    </div>
  );
}

const meta = {
  component: Folded,
  args: { as: "rail", panes: FOLDED, onOpen: fn() },
} satisfies Meta<typeof Folded>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **A rail, side by side**: each pane is reached by Tab and opened by Enter. */
export const Rail: Story = {
  play: async ({ canvas, args, userEvent }) => {
    const asks = canvas.getByRole("button", { name: "Open triage, folded, asks you" });
    await expect(asks).toBeVisible();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Open devops 2, folded, 3 notices" }),
      "Tab to the first folded pane",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => asks, "Tab to the pane that asks");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onOpen).toHaveBeenCalledWith(3);
  },
};

/** **A bar, stacked**: it has room for each pane's name. */
export const Bar: Story = {
  args: { as: "bar" },
  play: async ({ canvas, userEvent }) => {
    await expect(canvas.getByText("devops 2")).toBeVisible();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Open devops 2, folded, 3 notices" }),
      "Tab to the first folded pane",
    );
    expectFocusInSight();
  },
};
