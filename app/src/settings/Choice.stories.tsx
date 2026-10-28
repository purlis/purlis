// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The settings set's radio choice** (`Choice` with `kind: "radio"`), the first story the
 * component harness runs (#2901): drawn, held to axe, walked by the keyboard, and compared with
 * its baseline in Chromium and WebKit.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor } from "storybook/test";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { Choice } from "./components";

const DENSITIES = [
  { value: "comfortable", label: "Comfortable", says: "Rows and controls at their full height." },
  { value: "compact", label: "Compact", says: "Rows and controls a step shorter." },
  { value: "spacious", label: "Spacious", says: "Rows and controls a step taller." },
] as const;

function Density() {
  const [value, setValue] = useState<string>("comfortable");
  return (
    <Choice
      kind="radio"
      ids={{ id: "story-density", label: "Density" }}
      options={DENSITIES}
      value={value}
      onValueChange={setValue}
    />
  );
}

const meta = {
  component: Density,
} satisfies Meta<typeof Density>;

export default meta;

/**
 * One Tab stop for the group, landing on the option picked; each arrow moves the pick with the
 * focus, round from the last option to the first; Shift+Tab leaves the group.
 */
export const Radios: StoryObj<typeof meta> = {
  play: async ({ canvas, userEvent }) => {
    const group = canvas.getByRole("radiogroup", { name: "Density" });
    const comfortable = canvas.getByRole("radio", { name: "Comfortable" });
    const compact = canvas.getByRole("radio", { name: "Compact" });
    const spacious = canvas.getByRole("radio", { name: "Spacious" });

    await userEvent.tab();
    await expect(comfortable).toHaveFocus();
    await expect(comfortable).toBeChecked();
    expectFocusInSight();

    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => compact, "Down moves the keyboard to Compact");
    await waitFor(() => expect(compact).toBeChecked());
    expectFocusInSight();

    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(spacious).toBeChecked());

    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(comfortable).toBeChecked());
    await expect(spacious).not.toBeChecked();

    await userEvent.tab({ shift: true });
    await expect(group).not.toContainElement(document.activeElement as HTMLElement | null);

    // Back in, on the option picked, which is what the baseline shows: the focus ring on it.
    await userEvent.tab();
    await expect(comfortable).toHaveFocus();
    expectFocusInSight();
  },
};
