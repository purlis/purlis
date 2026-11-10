// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **All chats** (ADR 0121 decisions 18 and 21, #6013): the Chats sidebar's mode that lists every
 * chat in every open project. Each row has its mark and name, its time in its state, its state by
 * name with its project and workspace, its last line, and the chat's actions in the one order.
 * Rows are chosen with Space or their boxes, and the chosen ones are stopped or snoozed together.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { AllChats, type FleetDoes } from "./AllChats";
import type { FleetChat } from "./everyChat";
import type { ChatState } from "./shownState";
import { expectFocusInSight, untilFocused, untilFocusWithin } from "./stories.testkit";

/** A fixed now, so a row's time reads the same in every picture. */
const NOW = Date.now();

function chat(session: number, state: ChatState, more: Partial<FleetChat> = {}): FleetChat {
  return {
    plane: "/p/purlis",
    project: "purlis",
    session,
    name: `steward ${session}`,
    persona: null,
    workspace: "smart-ide",
    state,
    word: state,
    since: null,
    line: null,
    asks: [],
    task: false,
    stoppable: true,
    ...more,
  };
}

/** One chat in each Chat state, across two projects. */
const EVERY_STATE: FleetChat[] = [
  chat(3, "needs", {
    word: "needs you",
    line: "Shall I push the fix to main, or open a pull request first?",
    asks: ["q3"],
    since: NOW - 4 * 60_000,
  }),
  chat(7, "stuck", { plane: "/p/docs", project: "docs", workspace: "site", word: "working" }),
  chat(2, "failed", { word: "a task failed", line: "a task failed" }),
  chat(4, "working", { word: "working", line: "running cargo test", since: NOW - 90_000 }),
  chat(5, "waiting", { word: "waiting on 2 tasks", line: "waiting on 2 tasks" }),
  chat(6, "idle", { plane: "/p/docs", project: "docs", workspace: "site", word: "idle" }),
  chat(8, "ended", { word: "done", stoppable: false }),
  chat(9, "cant-tell", { name: "shell 9", word: "running (no detail)" }),
];

const DOES: FleetDoes = {
  go: () => {},
  answer: () => {},
  snooze: () => Promise.resolve(undefined),
  stop: () => Promise.resolve(undefined),
};

/** All chats as wide and tall as the sidebar. */
function Sidebar({ chats, filter }: { chats: FleetChat[]; filter?: ChatState }) {
  return (
    <div style={{ inlineSize: 320, blockSize: 640, display: "flex", flexDirection: "column" }}>
      <AllChats chats={chats} filter={filter} onFilter={() => {}} onLeave={() => {}} does={DOES} />
    </div>
  );
}

const meta = {
  component: Sidebar,
} satisfies Meta<typeof Sidebar>;

export default meta;
type Story = StoryObj<typeof meta>;

const rows = (canvas: HTMLElement) =>
  within(within(canvas).getByRole("grid", { name: "All chats" })).getAllByRole("row");

/** **Every Chat state**, Waiting on you first, then Stuck?, then the rest: Tab lands on the first
 *  row, ↓ walks the rows, → reaches a row's actions in their order. */
export const EveryState: Story = {
  args: { chats: EVERY_STATE },
  play: async ({ canvasElement }) => {
    const listed = rows(canvasElement);
    await expect(listed.map((one) => one.getAttribute("data-state"))).toEqual([
      "needs",
      "stuck",
      "failed",
      "working",
      "waiting",
      "idle",
      "ended",
      "cant-tell",
    ]);
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(() => rows(canvasElement)[0], "Tab lands on the first row");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => within(rows(canvasElement)[0]).getByRole("button", { name: "Answer steward 3" }),
      "→ reaches Answer, the first of its actions",
    );
    expectFocusInSight();
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => within(rows(canvasElement)[0]).getByRole("button", { name: "Go to chat steward 3" }),
      "→ goes on to Go to chat",
    );
    expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}");
    await untilFocusWithin(() => rows(canvasElement)[1], "↓ goes to the next row");
    expectFocusInSight();
  },
};

/** **Several chosen**: Space chooses a row, and the chosen ones are stopped or snoozed together;
 *  Stop asks once, in the view, and Esc clears what is chosen. */
export const SeveralChosen: Story = {
  args: { chats: EVERY_STATE },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(() => rows(canvasElement)[0], "Tab lands on the first row");
    await userEvent.keyboard(" ");
    await userEvent.keyboard("{ArrowDown}");
    await userEvent.keyboard(" ");
    const chosen = () => canvas.getByRole("group", { name: "Chosen chats" });
    await waitFor(() => expect(chosen()).toHaveTextContent("2 chats chosen"));
    await userEvent.click(within(chosen()).getByRole("button", { name: "Stop" }));
    const asked = () => canvas.getByRole("group", { name: "Stop chats" });
    await waitFor(() => expect(asked()).toHaveTextContent("Stop 2 chats?"));
    await userEvent.click(within(asked()).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(chosen()).toHaveTextContent("2 chats chosen"));
  },
};

/** **Filtered to one state**, as Asks' "N chats working" opens it (#6020): Show all takes it off. */
export const OneState: Story = {
  args: { chats: EVERY_STATE, filter: "working" },
  play: async ({ canvasElement }) => {
    await expect(rows(canvasElement).map((one) => one.getAttribute("data-state"))).toEqual([
      "working",
    ]);
    await expect(within(canvasElement).getByRole("button", { name: "Show all" })).toBeVisible();
  },
};

/** **No chat open anywhere**: the list says so. */
export const NoChats: Story = {
  args: { chats: [] },
  play: async ({ canvasElement }) => {
    await expect(within(canvasElement).getByText("No chat is open in any project.")).toBeVisible();
  },
};
