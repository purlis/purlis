// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The loosening press** (M75-15; spec §9.2): the project's policy notice with its diff against
 * the policy in force: each rule as loosens, tightens or changes, each mode and limit before and
 * after, the commit with its author as the commit claims it, and a tightening of your own file that goes.
 * Accept and Not now are reached by the keyboard. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PolicyState } from "./bindings";
import { ProjectPolicyNotice } from "./ProjectPolicyNotice";
import { forgetPolicies } from "./projectPolicy";
import { WATCH } from "./projectPolicy.fixture";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const PLANE = "/home/op/project";

const PRESSED: PolicyState = {
  ...WATCH,
  press: {
    reason: "loosening",
    rules: [
      { id: "guards-merge-ban", kind: "loosens", what: "removed" },
      { id: "guards-merge-new", kind: "tightens", what: "added" },
      { id: "guards-merge-edit", kind: "changes", what: "edited" },
    ],
    modes: [{ key: "guards.merge", before: "enforce", after: "log", kind: "loosens" }],
    limits: [{ key: "dispatch.running-per-chat", before: "3", after: "9", kind: "loosens" }],
    local: [{ key: "template", was: "strict", now: "open" }],
    other: ["policy.sets.protected-branches"],
    commit: {
      sha: "0123abcd0123abcd0123abcd0123abcd0123abcd",
      subject: "Drop the merge ban",
      author: "Ada <ada@x>",
    },
    lost: null,
  },
};

const meta = {
  component: ProjectPolicyNotice,
  args: { plane: PLANE, canAccept: true },
  beforeEach: () => {
    mockIPC((cmd) => (cmd === "project_policy_state" ? PRESSED : null));
    return () => {
      forgetPolicies();
      clearMocks();
    };
  },
} satisfies Meta<typeof ProjectPolicyNotice>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **A commit loosens**: the diff, the commit, an author purlis could not verify, Accept and Not now. */
export const LooseningPress: Story = {
  play: async ({ canvas, userEvent }) => {
    await canvas.findByText("loosens: removed");
    await canvas.findByText(/purlis can't check who wrote it/);
    await canvas.findByText("your own file loosens: strict to open");
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Accept" }), "Tab reaches Accept");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Not now" }),
      "Tab reaches Not now",
    );
    expectFocusInSight();
    await expect(canvas.getAllByRole("listitem").length).toBeGreaterThan(4);
  },
};
