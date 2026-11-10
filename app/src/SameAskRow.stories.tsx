// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Answer once for all identical requests** (ADR 0121 decision 6, #6009): three chats asking to
 * reach the same host are one row in Asks, "3 chats want to reach api.example.com", drawn as a
 * decision: Allow for all, which allows each chat once with its narrowest grant, then Answer
 * each, which opens each chat's own row under it.
 */
import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import type { Shown } from "./bindings";
import { Inbox } from "./Inbox";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const host = (session: number, since: number): Shown => ({
  session,
  ask: `block:${session}:connect:host:api.example.com`,
  says: "The sandbox refused api.example.com",
  options: [
    { id: "you", label: "Allow for me on this machine", allows: true },
    { id: "chat", label: "Allow only for this chat", allows: true },
    { id: "keep", label: "Keep blocked", allows: false },
  ],
  source: "sandbox-host",
  chain: [`steward ${session}`],
  answer: {
    via: "sandbox-block",
    shown: { operation: "connect", kind: "host", what: "host", target: "api.example.com" },
  },
  since,
});

/** Three chats asking for the same host, waiting 12 minutes and less. */
function ThreeChats() {
  const [now] = useState(() => Math.floor(Date.now() / 1000));
  return (
    <div style={{ inlineSize: 420 }}>
      <Inbox
        plane="/plane"
        asks={[host(4, now - 720), host(5, now - 300), host(6, now - 60)]}
        onGo={() => {}}
        onLeave={() => {}}
      />
    </div>
  );
}

const meta = {
  component: ThreeChats,
} satisfies Meta<typeof ThreeChats>;

export default meta;
type Story = StoryObj<typeof meta>;

const groupRow = (canvasElement: HTMLElement) =>
  canvasElement.querySelector<HTMLElement>('[role="row"].inbox-same');

/** **One row for three chats**: Tab lands on it, → reaches Allow for all, then Answer each. */
export const OneRowForThreeChats: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("3 chats want to reach api.example.com")).toBeInTheDocument();
    await expect(
      canvasElement.querySelectorAll('[role="row"].inbox-ask:not(.inbox-same)'),
    ).toHaveLength(0);
    await userEvent.tab();
    await untilFocused(() => groupRow(canvasElement), "Tab lands on the group's row");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => canvas.getByRole("button", { name: "Allow for all" }),
      "→ reaches Allow for all",
    );
    expectFocusInSight();
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => canvas.getByRole("button", { name: /^Answer each/ }),
      "→ reaches Answer each",
    );
    expectFocusInSight();
  },
};

/** **Opened to answer each**: Answer each draws each chat's own row under the group's, and ↓
 *  walks them. */
export const OpenedToAnswerEach: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const each = canvas.getByRole("button", { name: /^Answer each/ });
    each.focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(each).toHaveAttribute("aria-expanded", "true"));
    await waitFor(() =>
      expect(
        canvasElement.querySelectorAll('[role="row"].inbox-ask:not(.inbox-same)'),
      ).toHaveLength(3),
    );
    const rows = () => [...canvasElement.querySelectorAll<HTMLElement>('[role="row"].inbox-ask')];
    rows()[0].focus();
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => rows()[1], "↓ reaches the first chat's own row");
    expectFocusInSight();
    await expect(within(rows()[1]).getByText("steward 4")).toBeInTheDocument();
  },
};
