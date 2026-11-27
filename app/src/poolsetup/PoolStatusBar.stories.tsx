// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The pool's status bar** (#7191): each of the five states a pool is in, what is said of it on a
 * machine and on a cluster, and the keyboard's way through its answers. A forgotten pool draws
 * nothing.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, within } from "storybook/test";
import { expectFocusInSight } from "../stories.testkit";
import { PoolStatusBar } from "./PoolStatusBar";

const meta = {
  component: PoolStatusBar,
  args: {
    pool: "build-box",
    where: { kind: "machine", host: "build-box.local" },
    status: { state: "unreachable" },
    onTryNow: fn(),
    onCheckPool: fn(),
    onCheckKey: fn(),
    onRemake: fn(),
    onForget: fn(),
  },
  decorators: [
    (Story) => (
      <div style={{ maxWidth: 720 }} className="pool-setup-page">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof PoolStatusBar>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Unreachable: purlis keeps trying, and Try now asks at once. Tab walks Try now, then Forget. */
export const Unreachable: Story = {
  play: async ({ canvasElement, args, userEvent }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("Unreachable: pool build-box")).toBeVisible();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Try now" })).toHaveFocus();
    await expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onTryNow).toHaveBeenCalled();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Forget pool…" })).toHaveFocus();
    await expectFocusInSight();
  },
};

/** An add the machine would not take: the pool is kept, with its reason, and purlis stops. */
export const NotAddedOnAMachine: Story = {
  args: { status: { state: "refused", reason: "the disk on build-box.local is full" } },
  play: async ({ canvasElement, args, userEvent }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("Not added: pool build-box")).toBeVisible();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Check pool" })).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    await expect(args.onCheckPool).toHaveBeenCalled();
  },
};

/** The same on a cluster: it names the context, and no machine that answered. */
export const NotAddedOnACluster: Story = {
  args: {
    where: { kind: "cluster", context: "gke-team" },
    status: { state: "refused", reason: "the cluster operator is not installed" },
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      canvas.getByText(/The cluster \(context gke-team\) did not take the pool/),
    ).toBeVisible();
  },
};

/** A key that changed: purlis stops, and the one way on is to check the new key. */
export const HostKeyChanged: Story = {
  args: { status: { state: "host-key-changed" } },
  play: async ({ canvasElement, args, userEvent }) => {
    const canvas = within(canvasElement);
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Check the new key" })).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    await expect(args.onCheckKey).toHaveBeenCalled();
  },
};

/** The runner is gone, and what holds the sign-ins is not. */
export const RunnerGone: Story = {
  args: { status: { state: "runner-gone" } },
  play: async ({ canvasElement, args, userEvent }) => {
    const canvas = within(canvasElement);
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Make the runner again" })).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    await expect(args.onRemake).toHaveBeenCalled();
  },
};

/** A forgotten pool has no state to show, and the bar draws nothing. */
export const PoolForgotten: Story = {
  args: { status: { state: "pool-forgotten" } },
  // Nothing is drawn, so there is no picture to take.
  parameters: { screenshot: false },
  play: async ({ canvasElement }) => {
    await expect(canvasElement.querySelector("section")).toBeNull();
  },
};
