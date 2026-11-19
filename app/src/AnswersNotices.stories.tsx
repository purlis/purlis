// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What the sealed answers say at a project's window** (M75-07a, #7015): the nevers imported
 * from the old files, and the deny state with its way out. One standing line each, with Show
 * sealed answers and Dismiss reached by Tab. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { AnswersNotices } from "./AnswersNotices";
import { DENIED, IMPORTED } from "./settings/answersFixture";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const PLANE = "/home/op/project";

function Window() {
  return (
    <div style={{ inlineSize: 720 }}>
      <AnswersNotices plane={PLANE} />
    </div>
  );
}

const meta = {
  component: Window,
  beforeEach: () => () => clearMocks(),
} satisfies Meta<typeof Window>;

export default meta;
type Story = StoryObj<typeof meta>;

export const NeversImported: Story = {
  beforeEach: () => {
    mockIPC((cmd) => (cmd === "answers_shown" ? IMPORTED : null));
  },
  play: async ({ canvas }) => {
    await canvas.findByText(/purlis sealed the 2 nevers you said before/);
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: /Dismiss/ }), "Dismiss");
    expectFocusInSight();
  },
};

export const DenyState: Story = {
  beforeEach: () => {
    mockIPC((cmd) => (cmd === "answers_shown" ? DENIED : null));
  },
  play: async ({ canvas }) => {
    await canvas.findByText(/nothing they allow holds/);
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Show sealed answers" }),
      "the keyboard reaches the way out",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: /Dismiss/ }), "and then Dismiss");
    await expect(canvas.getByRole("button", { name: /Dismiss/ })).toBeVisible();
  },
};
