// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Blocks are off by default, said once** (Q308a, instruction 45): the upgrade to the watch
 * policy, on the project's band, with Open policy settings and Dismiss. Drawn in every cell of
 * the theme matrix. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { forgetDismissals } from "./dismissals";
import { PolicyWatchAnnounced } from "./PolicyWatchAnnounced";
import { forgetPolicies, holdPolicy } from "./projectPolicy";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const PLANE = "/home/op/project";

function Band() {
  return (
    <div style={{ inlineSize: 720 }}>
      <PolicyWatchAnnounced plane={PLANE} />
    </div>
  );
}

const meta = {
  component: Band,
  beforeEach: () => {
    mockIPC((cmd) => (cmd === "seen_on_this_machine" ? false : null));
    holdPolicy(PLANE, {
      waiting: [],
      refused: [],
      not_in_force: [],
      unsealed: false,
      key_foreign: false,
      new_key: false,
      nevers_new_key: false,
      nevers_to_keep: null,
      nevers_unshowable: false,
      digest: "d1",
      template: "watch",
      template_default: true,
      overrides: [],
      locked: [],
    });
    return () => {
      forgetPolicies();
      forgetDismissals();
      clearMocks();
    };
  },
} satisfies Meta<typeof Band>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **The announcement**: what changed, how to turn a family back on, and its way to Settings. */
export const Announced: Story = {
  play: async ({ canvas, userEvent }) => {
    await canvas.findByText(/Blocks are now off by default/);
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Open policy settings" }),
      "Tab reaches Open policy settings",
    );
    expectFocusInSight();
    await expect(canvas.getByRole("button", { name: "Open policy settings" })).toBeVisible();
  },
};
