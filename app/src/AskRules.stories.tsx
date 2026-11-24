// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Ask less** (ADR 0121 decision 7, #6010): the line at the head of Asks while a cool-off holds
 * a chat's ask, and the list Rules I made opens, each rule with its Undo. No rule is offered on a
 * row any more (M75 slice 19, Q314.15).
 */
import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import type { RuleMadeShown, Shown } from "./bindings";
import { RulesList } from "./AskRules";
import { Inbox } from "./Inbox";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const host = (session: number, since: number): Shown => ({
  session,
  ask: `block:${session}:connect:host:api.example.com`,
  says: "The sandbox refused api.example.com",
  options: [
    { id: "you", label: "Allow for me on this machine", allows: true, scope: "project" },
    { id: "chat", label: "Allow only for this chat", allows: true, scope: "chat" },
    { id: "project", label: "Allow for everyone in this project", allows: true, scope: "project" },
    { id: "keep", label: "Keep blocked", allows: false },
  ],
  source: "sandbox-host",
  chain: [`steward ${session}`],
  answer: {
    via: "sandbox-block",
    shown: { operation: "connect", kind: "host", what: "host", target: "api.example.com" },
  },
  since,
  rule: { request: '["sandbox-host","api.example.com"]' },
});

const RULES: RuleMadeShown[] = [
  {
    id: "rule-2",
    plane: "/plane",
    project: "plane",
    at: 1_800_000_000,
    source: "sandbox-host",
    says: "The sandbox refused api.example.com",
    label: "Allow for me on this machine",
    scope: "project",
    saved: 3,
    undoes: true,
  },
  {
    id: "rule-1",
    plane: "/plane",
    project: "plane",
    at: 1_799_990_000,
    source: "run-grant",
    says: "steward 4 wants to run gh with vault work",
    label: "Allow for this chat",
    scope: "chat",
    saved: 0,
    undoes: true,
  },
];

/** What a story draws: the asks, or the rules list alone. */
function AskLess({ kind }: { kind: "cooling" | "rules" }) {
  const [now] = useState(() => Math.floor(Date.now() / 1000));
  if (kind === "rules")
    return (
      <div style={{ inlineSize: 420 }}>
        <RulesList rules={RULES} onUndo={() => {}} />
      </div>
    );
  return (
    <div style={{ inlineSize: 420 }}>
      <Inbox
        plane="/plane"
        asks={[]}
        cooling={{ asks: [host(5, now - 60)], until: null }}
        onGo={() => {}}
        onLeave={() => {}}
      />
    </div>
  );
}

const meta = {
  component: AskLess,
} satisfies Meta<typeof AskLess>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **A cool-off at the head of Asks**: whose ask it holds, and End cool-off. */
export const CoolingOff: Story = {
  args: { kind: "cooling" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText(/steward 5 heard Don't allow twice/)).toBeInTheDocument();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "End cool-off" }),
      "Tab reaches End cool-off",
    );
    expectFocusInSight();
  },
};

/** **Rules I made**: each rule, what it allows and for whom, and its Undo. */
export const RulesIMadeListed: Story = {
  args: { kind: "rules" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.tab();
    await untilFocused(
      () => canvas.getAllByRole("button", { name: /^Undo/ })[0],
      "Tab reaches the first Undo",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getAllByRole("button", { name: /^Undo/ })[1],
      "Tab reaches the second Undo",
    );
    expectFocusInSight();
  },
};
