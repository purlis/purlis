// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Known gaps in Settings' Sandbox group** (M75-37, #7046): per harness, each gap's sentence,
 * what purlis does instead and where it comes from, and the line that says a gap never stops a
 * chat from starting. Fixture data, never a live host. The list has no control of its own, so its
 * keyboard check is that Tab leaves the page without a trap.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent } from "storybook/test";
import { KnownGapsList, STILL_STARTS } from "./KnownGaps";
import { HARNESSES } from "./knownGapsFixture";

const meta = {
  component: KnownGapsList,
  args: { harnesses: HARNESSES, ids: { id: "known-gaps", labelledBy: undefined } },
  parameters: { screenshot: "page" },
} satisfies Meta<typeof KnownGapsList>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Two harnesses with gaps, one with none (left out), and one gap that holds for some chats. */
export const PerHarness: Story = {
  play: async ({ canvas }) => {
    await expect(canvas.getByText(STILL_STARTS)).toBeVisible();
    await expect(canvas.getByRole("list", { name: "Claude Code known gaps" })).toBeVisible();
    await expect(canvas.getByRole("list", { name: "Codex known gaps" })).toBeVisible();
    await expect(canvas.queryByRole("list", { name: "OpenCode known gaps" })).toBeNull();
    await userEvent.tab();
    await expect(document.body).toBeVisible();
  },
};

/** Nothing declared: every rule in force is enforced here. */
export const NoneDeclared: Story = {
  args: { harnesses: HARNESSES.map((one) => ({ ...one, gaps: [] })) },
  play: async ({ canvas }) => {
    await expect(canvas.getByText(/No known gaps are declared/)).toBeVisible();
  },
};
