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
import { CHAT_STATES } from "./shownState";
import { ChatMark } from "./ui/chat-mark/ChatMark";
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
    const asks = canvas.getByRole("button", { name: "triage, folded, asks you" });
    await expect(asks).toBeVisible();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "devops 2, 3 notices, folded" }),
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
      () => canvas.getByRole("button", { name: "devops 2, 3 notices, folded" }),
      "Tab to the first folded pane",
    );
    expectFocusInSight();
  },
};

/** One folded chat in each state its mark tells, the one that needs you asking. */
const EACH_STATE: RailPane[] = CHAT_STATES.map((state, at) => ({
  pane: at + 10,
  name: `chat ${state}`,
  mark: <ChatMark state={state} who={<SquareTerminal aria-hidden="true" />} />,
  holds: { notes: 0, trouble: 0, asks: state === "needs" ? 1 : 0 },
  asks: state === "needs",
  stated: true,
}));

/**
 * **A rail in each state** (#5447): each folded chat wears its one mark, and the one that needs
 * you shows ✋ once, on its mark's badge, with no second ✋ of the rail's beside it.
 */
export const RailInEachState: Story = {
  args: { panes: EACH_STATE },
  play: async ({ canvas, canvasElement, userEvent }) => {
    const asks = canvas.getByRole("button", { name: "chat needs, folded, asks you" });
    const item = asks.closest(".pane-rail-item");
    await expect(item?.querySelectorAll('[data-mark="needs-you"]').length).toBe(1);
    await expect(canvasElement.querySelector(".pane-rail-asks")).toBeNull();
    for (const state of CHAT_STATES)
      await expect(
        canvasElement.querySelector(`[data-scope="chat-mark"][data-variant="${state}"]`),
        state,
      ).not.toBeNull();
    await userEvent.tab();
    await untilFocused(() => asks, "Tab to the first folded chat");
    expectFocusInSight();
  },
};
