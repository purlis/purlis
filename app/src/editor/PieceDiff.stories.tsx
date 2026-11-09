// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A hunk's Discard, and the two warnings it can raise** (#5406): the control on each change of
 * a file's diff, walked by keyboard, and the questions a chat that is not idle and a file changed
 * on disk each ask. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PlaneId, WhatChanged } from "../bindings";
import { expectFocusInSight, pictureNow, untilFocused } from "../stories.testkit";
import { PieceDiffTab } from "./PieceDiff";

const DRAWN: WhatChanged = {
  mark: "changed",
  from: null,
  uncommitted: true,
  base: "main",
  stamp: "drawn",
  checks: {
    sets: [],
    onLines: [],
    outside: [],
    elsewhere: [],
    moreOnLines: 0,
    moreOutside: 0,
    moreElsewhere: 0,
    trouble: null,
    faults: [],
  },
  sinceTurnEnded: false,
  diff: {
    kind: "text",
    base: "one\ntwo\nthree\nfour\n",
    head: "ONE\ntwo\nthree\nFOUR\n",
    hunks: [
      { oldStart: 1, oldLines: 1, newStart: 1, newLines: 1 },
      { oldStart: 4, oldLines: 1, newStart: 4, newLines: 1 },
    ],
  },
};

/** The core: the file as drawn, and what a discard of it answers. */
function core(answer: unknown) {
  mockIPC((cmd) => {
    if (cmd === "what_changed") return DRAWN;
    if (cmd === "discard_hunk") return answer;
    throw new Error(`unexpected ${cmd}`);
  });
  return clearMocks;
}

const meta = {
  component: PieceDiffTab,
  args: {
    plane: "/home/op/project" as unknown as PlaneId,
    cut: { workspace: "alpha", repo: "svc", piece: "fix-it" },
    path: "src/lib.rs",
  },
  decorators: [
    (Story) => (
      <div style={{ width: 860, height: 320 }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof PieceDiffTab>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Each change has its own Discard, named for its line and reached by Tab. */
export const EachChangeHasItsDiscard: Story = {
  beforeEach: () => core({ kind: "stale" }),
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    const first = await canvas.findByRole("button", { name: "Discard the change at line 1" });
    const second = canvas.getByRole("button", { name: "Discard the change at line 4" });
    first.focus();
    await untilFocused(() => first, "the first Discard takes the keyboard");
    expectFocusInSight();
    // Its line's other controls come first, and how many depends on your editor's being known.
    for (let press = 0; press < 8 && document.activeElement !== second; press += 1)
      await userEvent.tab();
    await untilFocused(() => second, "Tab reaches the next Discard");
    expect(second).toBeEnabled();
    expectFocusInSight();
  },
};

/** A chat that is not idle: named with its state, the keyboard on the safe answer, Esc leaves it. */
export const AChatIsWorkingThere: Story = {
  // The warning draws over the page, outside the story's frame, and Esc takes it away.
  parameters: { screenshot: "page" },
  beforeEach: () =>
    core({ kind: "busy", chats: [{ name: "Fix the parser", state: "is running" }] }),
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    const page = within(document.body);
    const control = await canvas.findByRole("button", { name: "Discard the change at line 4" });
    control.focus();
    await userEvent.keyboard("{Enter}");
    await page.findByRole("alertdialog", { name: "Fix the parser is running" });
    await untilFocused(
      () => page.getByRole("button", { name: "Cancel" }),
      "the warning starts on Cancel",
    );
    expectFocusInSight();
    await pictureNow();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(page.queryByRole("alertdialog")).toBeNull());
  },
};

/** A file changed on disk: Reload or Overwrite, the keyboard on Reload, which is safe. */
export const TheFileChangedOnDisk: Story = {
  // The question draws over the page, outside the story's frame, and Esc takes it away.
  parameters: { screenshot: "page" },
  beforeEach: () => core({ kind: "stale" }),
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    const page = within(document.body);
    const control = await canvas.findByRole("button", { name: "Discard the change at line 1" });
    control.focus();
    await userEvent.keyboard("{Enter}");
    const stale = await page.findByRole("alertdialog", {
      name: "src/lib.rs changed on disk since this diff was drawn",
    });
    // The dialog fades in as it opens, and nothing in it is visible until it has.
    await waitFor(() =>
      expect(within(stale).getByRole("button", { name: "Overwrite" })).toBeVisible(),
    );
    await untilFocused(
      () => page.getByRole("button", { name: "Reload" }),
      "the question starts on Reload",
    );
    expectFocusInSight();
    await pictureNow();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(page.queryByRole("alertdialog")).toBeNull());
  },
};

/** A chat in the folder its agent can't be told in (#5477): named after the discard, and the
 *  line's Dismiss is reached and pressed from the keyboard. */
export const AChatIsNotToldOfIt: Story = {
  beforeEach: () =>
    core({
      kind: "discarded",
      now: DRAWN,
      unrecorded: null,
      untold: ["Fix the parser"],
    }),
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    const control = await canvas.findByRole("button", { name: "Discard the change at line 1" });
    control.focus();
    await userEvent.keyboard("{Enter}");
    await canvas.findByText(/Fix the parser is not told of this change at its next turn/);
    const dismiss = canvas.getByRole("button", { name: "Dismiss" });
    dismiss.focus();
    await untilFocused(() => dismiss, "the line's Dismiss takes the keyboard");
    expectFocusInSight();
    await pictureNow();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.queryByText(/is not told of this change/)).toBeNull());
  },
};
