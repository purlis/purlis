// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The project's policy on a chat's pane head** (Q308a, instruction 45): the template's word,
 * and the count of the families the project sets apart from it, with the whole sentence on its
 * title and to a screen reader. A fact, never a control. Drawn in every cell of the theme matrix.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import type { PolicyState } from "./bindings";
import { PolicyMarkOf } from "./PolicyMark";
import { WATCH } from "./projectPolicy.fixture";

function Head({ state }: { state: PolicyState }) {
  return (
    <div className="pane-chips" style={{ inlineSize: 320 }}>
      <PolicyMarkOf state={state} />
    </div>
  );
}

const meta = { component: Head, args: { state: WATCH } } satisfies Meta<typeof Head>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **The shipped default**: a project that names no policy runs watch. */
export const TheDefault: Story = {
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("img", { name: "Policy: watch, the default." })).toBeVisible();
  },
};

/** **A template and two families set apart**, one of them enforced for now. */
export const SetApart: Story = {
  args: {
    state: {
      ...WATCH,
      template: "team",
      template_default: false,
      overrides: [
        { family: "guards.release", said: "log", runs: "log" },
        { family: "network", said: "off", runs: "enforce" },
      ],
    },
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByText("team +2")).toBeVisible();
  },
};
