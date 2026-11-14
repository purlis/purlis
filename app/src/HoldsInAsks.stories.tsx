// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Holds in Asks** (T1-39; Q239, R-142, R-143): a merge a chat's hook held is an ask of its own
 * sort, of the Merge risk class. Two chats holding the same merge are two rows, never one
 * answered once for all (#6009), and a hold's row offers nothing to remember (#6010), even where
 * a rule offer reached it.
 */
import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import type { Shown } from "./bindings";
import { Inbox } from "./Inbox";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const HEAD = "0123456789abcdef0123456789abcdef01234567";

/** The merge chat `session`'s hook held: request 12 of `o/r` at one head, into main. */
const hold = (session: number, since: number, offered = false): Shown => ({
  session,
  ask: `merge:${session}:o/r#12@${HEAD}`,
  says: "Merge request 12 of o/r at 0123456789ab? There is no undo.",
  options: [
    { id: "merge", label: "Merge", allows: true },
    { id: "dont-merge", label: "Don't merge", allows: false },
  ],
  source: "merge-hold",
  chain: [`steward ${session}`],
  answer: {
    via: "merge-hold",
    repo: "o/r",
    number: 12,
    head: HEAD,
    facts: {
      title: "Add the parser",
      author: "octo",
      base: "main",
      checks: "PASSED",
      read_at: 1_763_000_000,
      ruleset: "none",
      look: "not-viewed",
      since_viewed: null,
      pushed_here: true,
      digest: "d-passed",
    },
  },
  risk: {
    class: "merge",
    why: "Merges request 12 of o/r at 0123456 into main, with no undo",
    confirm: "Merge request 12 of o/r at 0123456 into main?",
  },
  since,
  // A rule offer the window must never draw on a hold.
  ...(offered
    ? {
        rule: {
          request: '["merge-hold","o/r#12"]',
          offer: { answered: "merge", times: 2, options: ["merge"] },
        },
      }
    : {}),
});

function Holds({ kind }: { kind: "twice" | "offered" }) {
  const [now] = useState(() => Math.floor(Date.now() / 1000));
  const asks =
    kind === "twice" ? [hold(3, now - 600), hold(5, now - 120)] : [hold(3, now - 600, true)];
  return (
    <div style={{ inlineSize: 420 }}>
      <Inbox plane="/plane" asks={asks} onGo={() => {}} onLeave={() => {}} />
    </div>
  );
}

const meta = {
  component: Holds,
} satisfies Meta<typeof Holds>;

export default meta;
type Story = StoryObj<typeof meta>;

const holdRows = (canvasElement: HTMLElement) => [
  ...canvasElement.querySelectorAll<HTMLElement>('[role="row"][data-source="merge-hold"]'),
];

/** **Two identical holds render as two rows** (R-142): no group, each its own row, ↓ between. */
export const TwoIdenticalHoldsAreTwoRows: Story = {
  args: { kind: "twice" },
  play: async ({ canvasElement }) => {
    await expect(holdRows(canvasElement)).toHaveLength(2);
    await expect(canvasElement.querySelector(".inbox-same")).toBeNull();
    await expect(within(holdRows(canvasElement)[0]).getByText("steward 3")).toBeInTheDocument();
    await expect(within(holdRows(canvasElement)[1]).getByText("steward 5")).toBeInTheDocument();
    await userEvent.tab();
    await untilFocused(() => holdRows(canvasElement)[0], "Tab lands on the first hold's row");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => holdRows(canvasElement)[1], "↓ reaches the second hold's row");
    expectFocusInSight();
  },
};

/** **A hold row offers no remembering option** (R-143): no rule to make, and no answer with a
 *  scope, on its row or behind its Answer. */
export const AHoldOffersNothingToRemember: Story = {
  args: { kind: "offered" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const row = holdRows(canvasElement)[0];
    await expect(canvas.queryByRole("group", { name: /^Make a rule/ })).toBeNull();
    await userEvent.tab();
    await untilFocused(() => row, "Tab lands on the hold's row");
    expectFocusInSight();
    const answer = within(row).getByRole("button", { name: /^Answer/ });
    for (let step = 0; step < 6 && document.activeElement !== answer; step++)
      await userEvent.keyboard("{ArrowRight}");
    await untilFocused(() => answer, "→ reaches its Answer");
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(canvas.getByRole("button", { name: "Don't merge" })).toBeInTheDocument(),
    );
    await expect(canvas.queryByRole("group", { name: /^Make a rule/ })).toBeNull();
    const words = canvas.getAllByRole("button").map((one) => one.textContent ?? "");
    await expect(words.filter((one) => /always|rule|remember/i.test(one))).toEqual([]);
    await expect(canvas.getByRole("button", { name: "Merge" })).toBeInTheDocument();
    await expect(canvas.getByRole("button", { name: "Don't merge" })).toBeInTheDocument();
  },
};
