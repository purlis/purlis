// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The known gaps a chat started with, on its pane head** (M75-37, #7046; proposed, review round
 * 1, S5): one standing line with Show known gaps and Dismiss, both reached by Tab. The core is
 * faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { KnownGapsNotice } from "./KnownGapsNotice";
import { CODEX_HOOKS } from "./settings/knownGapsFixture";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const PLANE = "/home/op/project";

function Pane() {
  return (
    <div style={{ inlineSize: 720 }}>
      <KnownGapsNotice plane={PLANE} session={4} />
    </div>
  );
}

const meta = {
  component: Pane,
  beforeEach: () => {
    mockIPC((cmd) => {
      if (cmd === "chat_known_gaps") return [CODEX_HOOKS];
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<typeof Pane>;

export default meta;
type Story = StoryObj<typeof meta>;

export const StartedWithAGap: Story = {
  play: async ({ canvas }) => {
    await canvas.findByText(/This chat started with 1 known gap: hook-fails-open/);
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Show known gaps" }),
      "the keyboard reaches the way out",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: /Dismiss/ }), "and then Dismiss");
    await expect(canvas.getByRole("button", { name: /Dismiss/ })).toBeVisible();
  },
};
