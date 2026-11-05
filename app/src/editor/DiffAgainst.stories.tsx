// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** **What a file's diff is compared against** (#5405), and its keyboard path. */
import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { DiffAgainst, type Against } from "./DiffAgainst";

function Live({ from }: { from: Against }) {
  const [value, setValue] = useState(from);
  return <DiffAgainst value={value} onChange={setValue} />;
}

const meta = {
  component: Live,
  args: { from: "base" },
} satisfies Meta<typeof Live>;

export default meta;
type Story = StoryObj<typeof meta>;

export const AgainstBase: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const base = canvas.getByRole("radio", { name: "Against base" });
    const turn = canvas.getByRole("radio", { name: "Latest turn" });
    await expect(base).toBeChecked();

    await userEvent.tab();
    await untilFocused(() => base, "Tab reaches the chosen segment");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(() => turn, "the arrow moves to the latest turn");
    await expect(turn).toBeChecked();
    expectFocusInSight();
    await userEvent.keyboard("{ArrowLeft}");
    await untilFocused(() => base, "the arrow moves back to against base");
    await expect(base).toBeChecked();
  },
};

export const LatestTurn: Story = { args: { from: "turn" } };
