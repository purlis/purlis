// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What the Inbox is still reading** (#5325): what may yet list a Notice, one line and one mark
 * each, in the Notices section, with the Inbox usable around them.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";
import { Inbox } from "./Inbox";

function Reading() {
  return (
    <div style={{ inlineSize: 420 }}>
      <Inbox
        plane="/plane"
        asks={[]}
        notices={<></>}
        reading={["Checking repos for a stale branch or address…", "Reading the alerts…"]}
        onGo={() => {}}
        onLeave={() => {}}
        onIgnore={() => {}}
      />
    </div>
  );
}

const meta = { component: Reading } satisfies Meta<typeof Reading>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Each line says what it is reading in words, so motion is never the only thing that says it. */
export const TwoStillComing: Story = {
  play: async ({ canvasElement }) => {
    await expect(within(canvasElement).getByText("Reading the alerts…")).toHaveAttribute(
      "aria-busy",
      "true",
    );
  },
};
