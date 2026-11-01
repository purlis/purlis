// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** **The steps family** (#4480): each state a step draws, and the keyboard's way through. */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";
import { expectFocusInSight } from "../../stories.testkit";
import { StepList } from "./Steps";

const meta = {
  title: "Families/Steps",
  component: StepList,
  args: {
    "aria-label": "Steps of a guide",
    onGo: fn(),
    showing: "pool",
    steps: [],
  },
} satisfies Meta<typeof StepList>;

export default meta;
type Story = StoryObj<typeof meta>;

export const EveryState: Story = {
  args: {
    steps: [
      { id: "where", name: "Where chats run", state: "done", line: "Docker on this Mac" },
      { id: "pool", name: "Pool", state: "attention", line: "Docker stopped" },
      { id: "signins", name: "Sign-ins", state: "skipped", line: "Skipped" },
      { id: "identity", name: "Agent identity", state: "waiting", line: "Waiting for your admin" },
      { id: "try", name: "Try it", state: "later" },
    ],
  },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: /^Where chats run/ })).toHaveFocus();
    await expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onGo).toHaveBeenCalledWith("where");
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: /^Pool/ })).toHaveFocus();
    await expectFocusInSight();
  },
};

export const AtTheCurrentStep: Story = {
  args: {
    steps: [
      { id: "where", name: "Where chats run", state: "done", line: "Docker on this Mac" },
      { id: "pool", name: "Pool", state: "current" },
      { id: "signins", name: "Sign-ins", state: "later" },
    ],
  },
};
