// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** **The code family** (#4481): a command, long text that scrolls, and the keyboard's way in. */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { expectFocusInSight } from "../../stories.testkit";
import { CodeBlock } from "./CodeBlock";

const meta = {
  component: CodeBlock,
  args: { label: "Commands", text: "docker version" },
} satisfies Meta<typeof CodeBlock>;

export default meta;
type Story = StoryObj<typeof meta>;

export const OneCommand: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: /^Copy/ })).toHaveFocus();
    await expectFocusInSight();
    await userEvent.tab();
    await expect(canvas.getByText("docker version").closest("pre")).toHaveFocus();
    await expectFocusInSight();
  },
};

export const LongerThanTheBox: Story = {
  args: {
    label: "What the button runs",
    text: `# Add the pool. purlis makes its runner and pairs it with this machine.
purlis pool add dev-box --image ghcr.io/purlis/purlis-runner@sha256:7be1c4f0a2d3e4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d`,
  },
};
