// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The title bar's save indicator, waiting on a request** (ADR 0051), and the same while the
 * project could not be fetched (#5824 F8): the pill says it is stale, and its name and tooltip
 * say since when and why.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";
import type { PlaneSaving } from "./bindings";
import { SaveIndicator, type SaveShown } from "./SavingView";
import { expectFocusInSight } from "./stories.testkit";

const WAITING: PlaneSaving = {
  stage: "pr-open",
  changed: [],
  ahead: 1,
  pr: "https://github.com/acme/plane/pull/7",
  request: "pull request",
  blocked: null,
  branch: "main",
  pushes: true,
  behind: 0,
  pushFailed: null,
  live: [],
  conflicts: [],
  conflictKeys: {},
  notice: null,
  mode: "pr",
  modeFrom: "purlis.toml",
  journal: [],
};

/** The indicator once the standing is read. */
function Shown(props: SaveShown) {
  return <SaveIndicator {...props} />;
}

const meta = {
  component: Shown,
  args: { saving: WAITING, busy: false, onOpen: fn(), onSave: fn() },
} satisfies Meta<typeof Shown>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Waiting on its pull request: Tab lands on the pill, and Enter opens the Saving tab. */
export const Waiting: Story = {
  play: async ({ canvasElement, args }) => {
    const pill = within(canvasElement).getByTestId("save-indicator-where");
    await userEvent.tab();
    await expect(pill).toHaveFocus();
    await expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onOpen).toHaveBeenCalledTimes(1);
  },
};

/** A fetch failed while the request is open: the pill says stale, its name says since when. */
export const Stale: Story = {
  args: {
    saving: {
      ...WAITING,
      stale: {
        since: new Date(2026, 10, 8, 14, 2).getTime() / 1000,
        why: "main could not be fetched: Could not resolve host: github.com",
      },
    },
  },
  play: async ({ canvasElement, args }) => {
    const pill = within(canvasElement).getByTestId("save-indicator-where");
    await expect(pill).toHaveTextContent("· stale");
    await expect(pill).toHaveAccessibleName(/Stale since .*Could not resolve host/);
    await userEvent.tab();
    await expect(pill).toHaveFocus();
    await expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onOpen).toHaveBeenCalledTimes(1);
  },
};
