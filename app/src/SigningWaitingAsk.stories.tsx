// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **One notice for lanes refused for unsigned commits** (#5633, `prototype/commit-signing`
 * moment "Notice (3 lanes)"): the registry's one Ask, in the Inbox, before setup, after setup
 * and once every lane went. Held to axe, walked by the keyboard, and pictured in every pictured
 * theme cell (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import type { Shown } from "./bindings";
import { Inbox } from "./Inbox";
import { expectFocusInSight } from "./stories.testkit";

const LANES =
  "- purlis · tab-names (tab-names)\n- purlis · empty-tab (empty-tab)\n- purlis · pane-foot (pane-foot)";

function notice(says: string, id: string, label: string): Shown {
  return {
    session: 0,
    ask: "signing-waiting",
    says,
    options: [{ id, label, allows: false }],
    source: "signing",
    chain: ["purlis"],
    answer: { via: "signing" },
  };
}

const BEFORE = notice(
  `3 lanes can't merge: purlis/purlis only accepts signed commits\n${LANES}`,
  "set-up",
  "Set up signing",
);
const AFTER = notice(
  `3 lanes can't merge: purlis/purlis only accepts signed commits\n${LANES}`,
  "retry-all",
  "Sign and retry all 3",
);
const DONE = notice("3 lanes signed and sent ✓", "dismiss", "Dismiss");

function TheInbox({ ask }: { ask: Shown }) {
  return (
    <div style={{ inlineSize: 420 }}>
      <Inbox plane="/plane" asks={[ask]} onGo={() => {}} onLeave={() => {}} onIgnore={() => {}} />
    </div>
  );
}

const meta = { component: TheInbox } satisfies Meta<typeof TheInbox>;

export default meta;
type Story = StoryObj<typeof meta>;

async function reached(canvasElement: HTMLElement, name: string) {
  const button = await waitFor(() => within(canvasElement).getByRole("button", { name }));
  button.focus();
  await expect(button).toHaveFocus();
  expectFocusInSight();
}

/** **Before setup**: one notice for the three lanes, and Set up signing. */
export const BeforeSetup: Story = {
  args: { ask: BEFORE },
  play: async ({ canvasElement }) => {
    await expect(within(canvasElement).getAllByText(/3 lanes can't merge/)).toHaveLength(1);
    await reached(canvasElement, "Set up signing");
  },
};

/** **After setup**: Sign and retry all 3. */
export const AfterSetup: Story = {
  args: { ask: AFTER },
  play: async ({ canvasElement }) => {
    await reached(canvasElement, "Sign and retry all 3");
    await userEvent.keyboard("{Tab}");
  },
};

/** **Done**: 3 lanes signed and sent ✓, and Dismiss. */
export const Done: Story = {
  args: { ask: DONE },
  play: async ({ canvasElement }) => {
    await expect(within(canvasElement).getByText(/3 lanes signed and sent ✓/)).toBeVisible();
    await reached(canvasElement, "Dismiss");
  },
};
