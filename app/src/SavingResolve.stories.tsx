// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A save's conflict, settled in a resolve folder off to the side** (#5824 F7): the Notice names
 * the folder and the files, and offers a chat, Continue and Abort. A settings key both machines
 * changed is chosen by its value, this machine's or the remote's, and never marked in the file
 * (#5852).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import type { PlaneSaving } from "./bindings";
import { SaveConflict } from "./SavingResolve";
import { expectFocusInSight } from "./stories.testkit";

const RESOLVING: PlaneSaving = {
  stage: "blocked",
  changed: [],
  ahead: 1,
  pr: null,
  request: "pull request",
  blocked: "a resolve is open",
  branch: "main",
  pushes: true,
  behind: 2,
  pushFailed: null,
  live: [],
  conflicts: ["notes.md"],
  conflictKeys: {},
  notice: null,
  mode: "pr",
  modeFrom: "purlis.toml",
  journal: [],
  stoppedBy: "resolve",
  resolving: { folder: "/home/dev/plane/.purlis/resolve", keys: [] },
};

const meta = {
  component: SaveConflict,
  args: { plane: "/home/dev/plane", saving: RESOLVING },
} satisfies Meta<typeof SaveConflict>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A file a person wrote conflicts: Tab reaches the chat, Continue and Abort in turn. */
export const Resolving: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      canvas.getByText(/resolve folder \/home\/dev\/plane\/\.purlis\/resolve/),
    ).toBeVisible();
    for (const name of ["Resolve in a chat", "Continue", "Abort"]) {
      await userEvent.tab();
      await expect(canvas.getByRole("button", { name })).toHaveFocus();
      await expectFocusInSight();
    }
  },
};

/** A settings key both machines changed: one Tab stop, the arrows pick a value. */
export const ResolvingAKey: Story = {
  args: {
    saving: {
      ...RESOLVING,
      conflicts: ["purlis.toml"],
      conflictKeys: { "purlis.toml": ["sandbox.mode"] },
      resolving: {
        folder: "/home/dev/plane/.purlis/resolve",
        keys: [{ file: "purlis.toml", key: "sandbox.mode", ours: '"workspace"', theirs: '"off"' }],
      },
    },
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const group = canvas.getByRole("radiogroup", { name: "sandbox.mode in purlis.toml" });
    const ours = within(group).getByRole("radio", { name: 'This machine\'s: "workspace"' });
    // The group is one Tab stop among the Notice's ways out, wherever it is drawn.
    for (let stop = 0; stop < 6 && document.activeElement !== ours; stop += 1) {
      await userEvent.tab();
    }
    await expect(ours).toHaveFocus();
    await expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}");
    const theirs = within(group).getByRole("radio", { name: 'The remote\'s: "off"' });
    await expect(theirs).toHaveFocus();
    await expect(theirs).toBeChecked();
    await expectFocusInSight();
  },
};
