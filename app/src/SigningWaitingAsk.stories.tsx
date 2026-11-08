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

function notice(says: string, id: string, label: string, more: [string, string][] = []): Shown {
  return {
    session: 0,
    ask: "signing-waiting",
    says,
    options: [[id, label] as [string, string], ...more].map(([one, named]) => ({
      id: one,
      label: named,
      allows: false,
    })),
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
const TWO_FORGES = notice(
  "2 lanes can't merge: purlis/purlis, team/purlis only accepts signed commits\n- purlis · tab-names (tab-names)\n- purlis · empty-tab (empty-tab)",
  "set-up:github:github.com",
  "Set up signing on GitHub",
  [["set-up:gitlab:gitlab.example.org", "Set up signing on GitLab"]],
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
  // The Ask's answers open from its card's Answer.
  await userEvent.click(
    await waitFor(() => within(canvasElement).getByRole("button", { name: "Answer purlis" })),
  );
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
  },
};

/** **Two forges** (#5790): lanes on GitHub and GitLab, each forge set up on its own. */
export const TwoForges: Story = {
  args: { ask: TWO_FORGES },
  play: async ({ canvasElement }) => {
    await reached(canvasElement, "Set up signing on GitLab");
    await expect(
      within(canvasElement).getByRole("button", { name: "Set up signing on GitHub" }),
    ).toBeEnabled();
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
