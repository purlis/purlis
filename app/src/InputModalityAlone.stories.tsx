// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** TEMPORARY (#5040 proof, reverted before review): `FocusTakenAfterAPress` drawn first in its
 *  file, so on a page no story has touched, to compare its picture with the one drawn after
 *  `EndsOnAPress`. */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import { Button } from "./ui/button/Button";
import { untilFocused } from "./stories.testkit";

function Pressable({ autoFocus = false }: { autoFocus?: boolean }) {
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <p style={{ margin: 0 }}>Words to press on</p>
      <Button autoFocus={autoFocus}>Cancel</Button>
    </div>
  );
}

const meta = { component: Pressable } satisfies Meta<typeof Pressable>;
export default meta;
type Story = StoryObj<typeof meta>;

export const FocusTakenAlone: Story = {
  args: { autoFocus: true },
  play: async ({ canvas }) => {
    const cancel = canvas.getByRole("button", { name: "Cancel" });
    await untilFocused(() => cancel, "Cancel, focused as it is drawn");
    await expect(cancel).toHaveAttribute("data-focus-visible");
  },
};
