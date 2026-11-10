// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Asks, ordered by facts** (ADR 0121 decision 5, #6006): an Expiring ask first with its
 * countdown, then one that blocks other chats, then the longest wait. Each row's reason line
 * says why it stands there. ↓ walks the rows in that order.
 */
import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import type { Shown } from "./bindings";
import { Inbox } from "./Inbox";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const permission = (session: number, says: string, more: Partial<Shown>): Shown => ({
  session,
  ask: `permission-${session}`,
  says,
  options: [
    { id: "allow", label: "Allow", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: [`steward ${session}`],
  answer: { via: "hook" },
  ...more,
});

/** Three asks, listed by the registry newest first; Asks orders them by their facts. The
 *  Expiring one ends 2½ minutes out, so its countdown reads whole minutes in every picture. */
function ThreeAsks() {
  const [now] = useState(() => Math.floor(Date.now() / 1000));
  return (
    <div style={{ inlineSize: 420 }}>
      <Inbox
        plane="/plane"
        asks={[
          permission(7, "Run make release", { since: now - 20, expires: now + 150 }),
          permission(5, "Run npm test", { since: now - 120, blocks: 2 }),
          permission(3, "Run cargo test", { since: now - 720 }),
        ]}
        onGo={() => {}}
        onLeave={() => {}}
      />
    </div>
  );
}

const meta = {
  component: ThreeAsks,
} satisfies Meta<typeof ThreeAsks>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Expiring, then blocks others, then the longest wait; ↓ walks them in that order. */
export const OrderedByFacts: Story = {
  play: async ({ canvasElement }) => {
    const list = within(canvasElement);
    await expect(list.getByText("Expires in 3 min")).toBeInTheDocument();
    await expect(list.getByText("Blocks 2 chats")).toBeInTheDocument();
    await expect(list.getByText("Waiting 12 min")).toBeInTheDocument();
    const rows = () => [...canvasElement.querySelectorAll<HTMLElement>('[role="row"].inbox-ask')];
    await expect(rows()).toHaveLength(3);
    await userEvent.tab();
    await untilFocused(() => rows()[0], "Tab lands on the Expiring ask");
    expectFocusInSight();
    await expect(within(rows()[0]).getByText("Run make release")).toBeInTheDocument();
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => rows()[1], "↓ reaches the ask that blocks other chats");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => rows()[2], "↓ reaches the longest wait");
    expectFocusInSight();
    await expect(within(rows()[2]).getByText("Run cargo test")).toBeInTheDocument();
  },
};
