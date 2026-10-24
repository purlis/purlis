// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The separator family** (#2912): the line between two groups inside a surface, across a
 * card's rows and down a strip, its own part and never a control's border. Each held to axe and
 * compared with its baseline (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import { Card } from "../card/Card";
import { Separator } from "./Separator";

/** Two groups of a card, a line between them. */
function Across() {
  return (
    <div
      style={{
        inlineSize: "20rem",
        padding: "var(--space-5)",
        background: "var(--surface-raised)",
      }}
    >
      <Card aria-label="Hosts">
        <p style={{ margin: 0 }}>Waiting for your Confirm · 2</p>
        <Separator />
        <p style={{ margin: 0 }}>Confirmed · 17</p>
      </Card>
    </div>
  );
}

const meta = {
  component: Across,
} satisfies Meta<typeof Across>;

export default meta;
type Story = StoryObj<typeof meta>;

export const BetweenRows: Story = {
  play: async ({ canvas }) => {
    // A screen reader hears the line between the two groups.
    await expect(canvas.getByRole("separator")).toHaveAttribute("data-orientation", "horizontal");
  },
};

/** Down a strip, between two marks: hidden from a screen reader, which reads no groups there. */
export const DownAStrip: Story = {
  render: () => (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: "var(--space-3)",
        blockSize: "var(--size-chrome)",
        padding: "0 var(--space-4)",
        background: "var(--surface-deep)",
      }}
    >
      <span>purlis</span>
      <Separator orientation="vertical" decorative />
      <span>smart-ide</span>
    </div>
  ),
  play: async ({ canvas }) => {
    await expect(canvas.queryByRole("separator")).toBeNull();
    await expect(canvas.getByText("smart-ide")).toBeInTheDocument();
  },
};
