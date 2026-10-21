// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Temporary (#2944): one deliberate failure for each gate, reverted before merge. */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";

function Probe({ broken }: { broken?: "render" | "axe" }) {
  if (broken === "render") throw new Error("the probe would not draw");
  if (broken === "axe") return <button type="button" />;
  return <button type="button">Probe</button>;
}

const meta = { component: Probe } satisfies Meta<typeof Probe>;
export default meta;

export const Throws: StoryObj<typeof meta> = { args: { broken: "render" } };
export const Unnamed: StoryObj<typeof meta> = { args: { broken: "axe" } };
export const Steps: StoryObj<typeof meta> = {
  play: async ({ canvas, userEvent }) => {
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Probe" })).not.toHaveFocus();
  },
};
export const Unpictured: StoryObj<typeof meta> = {};
