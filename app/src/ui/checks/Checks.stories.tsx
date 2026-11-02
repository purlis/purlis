// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** **The checks family** (#4481): each verdict a line draws. There's nothing to press in it. */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { CheckLine, CheckList } from "./Checks";

const meta = {
  component: CheckList,
  args: { "aria-label": "What was checked", children: null },
} satisfies Meta<typeof CheckList>;

export default meta;
type Story = StoryObj<typeof meta>;

export const EveryVerdict: Story = {
  args: {
    children: (
      <>
        <CheckLine verdict="ok" title="A chat started on dev-box" trail="In 11 seconds" />
        <CheckLine verdict="missing" title="A user who isn't root">
          The image runs as root, and a runner needs another user.
        </CheckLine>
        <CheckLine verdict="running" title="The profile is signed in" />
        <CheckLine verdict="todo" title="A chat starts on dev-box" />
        <CheckLine verdict="warn" title="Its harness can't say whether it's signed in" />
        <CheckLine verdict="skip" title="gh wasn't checked">
          Agent identity was skipped.
        </CheckLine>
      </>
    ),
  },
};
