// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** THROWAWAY (#4073): a story that fails its keyboard path as well as its missing picture. */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";

const meta = {
  title: "UI/Proof4073",
  render: () => (
    <button type="button" style={{ margin: 8 }}>
      Proof
    </button>
  ),
} satisfies Meta;

export default meta;

export const FailsItsKeyboard: StoryObj<typeof meta> = {
  play: async ({ canvasElement }) => {
    await expect(within(canvasElement).getByRole("button", { name: "Proof" })).toHaveFocus();
  },
};
