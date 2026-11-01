// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Inbox's card for a chat waiting on the person's reply** (#4550): what the chat said under
 * the status line, over the lines it wrote and never cut, and the one line that says so where
 * the app heard no message. The person answers from here, so the reply box is walked to.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import type { Shown } from "./bindings";
import { Inbox, NO_MESSAGE } from "./Inbox";
import { untilFocused } from "./stories.testkit";

const QUESTION: Shown = {
  session: 6,
  ask: "question:6",
  says: "Waiting on your reply",
  options: [],
  source: "question",
  chain: ["steward 6"],
  answer: { via: "in-its-pane" },
};

/** The Inbox as wide as the window's side of it, holding one card. */
function OneCard({ last_message }: { last_message?: string }) {
  return (
    <div style={{ inlineSize: 420 }}>
      <Inbox
        plane="/plane"
        asks={[{ ...QUESTION, last_message }]}
        onGo={() => {}}
        onLeave={() => {}}
      />
    </div>
  );
}

const meta = {
  component: OneCard,
} satisfies Meta<typeof OneCard>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The Tab key reaches the reply box, whatever the card says above it. */
async function theReplyBoxIsReached(canvasElement: HTMLElement) {
  const box = () => within(canvasElement).getByRole("textbox", { name: "Reply to steward 6" });
  await userEvent.tab();
  while (document.activeElement !== box() && canvasElement.contains(document.activeElement)) {
    await userEvent.tab();
  }
  await untilFocused(box, "Tab reaches the reply box");
}

/** What the chat said, over its lines, the status line above it once. */
export const WhatTheChatSaid: Story = {
  args: {
    last_message:
      "I checked both branches. main has the fix, and dev has the new tests.\n\nWhich should I build on, main or dev?",
  },
  play: async ({ canvasElement }) => {
    const card = within(canvasElement);
    await expect(card.getByText(/Which should I build on/)).toBeInTheDocument();
    await expect(card.getAllByText("Waiting on your reply")).toHaveLength(1);
    await theReplyBoxIsReached(canvasElement);
  },
};

/** The app heard no message: the card says so, once. */
export const NoMessageToShow: Story = {
  play: async ({ canvasElement }) => {
    const card = within(canvasElement);
    await expect(card.getAllByText(NO_MESSAGE)).toHaveLength(1);
    await expect(card.getAllByText("Waiting on your reply")).toHaveLength(1);
    await theReplyBoxIsReached(canvasElement);
  },
};
