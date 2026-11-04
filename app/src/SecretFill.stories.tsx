// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import { SecretFill } from "./SecretFill";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const meta = {
  title: "Notices/Missing secret",
  component: SecretFill,
} satisfies Meta<typeof SecretFill>;

export default meta;
type Story = StoryObj<typeof meta>;

const secret = {
  persona: "devops",
  vault: "team",
  key: "KUBECONFIG",
  tool: "kubectl",
  declared: true,
  waiting: 1,
};

/** #5068: a key a declared tool's run missed, with the box its value is typed in. */
export const ForATool: Story = {
  args: { secret, box: null },
  play: async ({ canvas, userEvent }) => {
    const box = canvas.getByLabelText("Value of KUBECONFIG");
    await userEvent.tab();
    await untilFocused(() => box, "the value's box");
    expectFocusInSight();
    await userEvent.keyboard("s3cret");
    // Masked: what was typed is never drawn.
    await expect(box).toHaveAttribute("type", "password");
  },
};

/** A key no declared tool names. */
export const NoTool: Story = {
  args: { secret: { ...secret, key: "API_TOKEN", tool: null }, box: null },
};
