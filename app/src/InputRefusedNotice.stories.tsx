// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a chat was not sent, on its pane** (#4734): the Notice a pane shows when what it sent
 * its chat was refused, here for a chat whose runner is reconnecting, with Dismiss. Drawn in every
 * cell of the theme matrix, light and dark among them. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { InputRefusedNotice } from "./InputRefusedNotice";
import { forgetInputRefused, heardInputRefused } from "./inputRefused";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const PLANE = "/home/op/project";

function Pane() {
  return (
    <div style={{ inlineSize: 720 }}>
      <InputRefusedNotice plane={PLANE} session={3} />
    </div>
  );
}

const meta = {
  component: Pane,
  beforeEach: () => {
    mockIPC(() => null);
    heardInputRefused({ plane: PLANE, session: 3, why: "this chat's runner is reconnecting" });
    return () => {
      forgetInputRefused(PLANE, 3);
      clearMocks();
    };
  },
} satisfies Meta<typeof Pane>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Its runner is reconnecting**: the line with why, and Dismiss reached by the keyboard. */
export const RunnerReconnecting: Story = {
  play: async ({ canvas, userEvent }) => {
    await canvas.findByText(/could not send this chat what you typed/);
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Dismiss" }),
      "Tab reaches Dismiss",
    );
    expectFocusInSight();
    await expect(canvas.getByRole("button", { name: "Dismiss" })).toBeVisible();
  },
};
