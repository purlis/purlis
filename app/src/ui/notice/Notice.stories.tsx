// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The notice family** (#2912): a pane's row of Notices with an Allow beside Keep blocked and
 * the rest behind +N more, the Inbox's list with news and trouble, and a Notice standing alone on
 * a page, each walked by the keyboard. Each held to axe and compared with its baseline
 * (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor } from "storybook/test";
import { Notice, NoticeList, NoticePaneRow } from "../../Notice";
import { expectFocusInSight } from "../../stories.testkit";

/** A pane: its row of Notices above the terminal's place. */
function APane() {
  const [said, setSaid] = useState("Nothing answered yet");
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        inlineSize: "36rem",
        blockSize: "16rem",
        background: "var(--surface-base)",
      }}
    >
      <NoticePaneRow>
        <Notice
          cause="sandbox-blocked:7:network:connect:chat"
          at="pane"
          label="Sandbox block"
          fixes={[
            { label: "Allow for this chat", onPress: () => setSaid("Allowed"), allows: true },
            { label: "Keep blocked", onPress: () => setSaid("Kept blocked") },
          ]}
        >
          steward 4 tried to reach api.github.com, and the sandbox refused it.
        </Notice>
        <Notice cause="chat-fresh:7" at="pane" label="Fresh" onDismiss={() => setSaid("Put away")}>
          This chat started fresh: its harness kept no conversation.
        </Notice>
        <Notice cause="chat-guessed:7" at="pane" label="Guessed" onDismiss={() => undefined}>
          purlis guessed which conversation this chat had.
        </Notice>
      </NoticePaneRow>
      <p style={{ margin: "var(--space-4)" }}>{said}</p>
    </div>
  );
}

const meta = {
  component: APane,
} satisfies Meta<typeof APane>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Tab reaches Allow, then Keep blocked, then the next Notice's Dismiss, then +1 more; Enter on
 *  it shows the third, and Escape puts it back behind +1 more with the focus on it. */
export const InAPane: Story = {
  play: async ({ canvas, userEvent }) => {
    const allow = await canvas.findByRole("button", { name: "Allow for this chat" });
    await userEvent.tab();
    await expect(allow).toHaveFocus();
    expectFocusInSight();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Keep blocked" })).toHaveFocus();
    await userEvent.tab();
    await expect(canvas.getAllByRole("button", { name: "Dismiss" })[0]).toHaveFocus();
    await userEvent.tab();
    const more = canvas.getByRole("button", { name: "+1 more" });
    await expect(more).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status", { name: "Guessed" })).toBeVisible());
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(canvas.queryByRole("status", { name: "Guessed" })).toBeNull());
    await expect(more).toHaveFocus();
  },
};

/** The Inbox's list: trouble first, then news, a separator between two rows. */
export const InTheInbox: Story = {
  render: () => (
    <div
      style={{
        inlineSize: "22rem",
        padding: "var(--space-5)",
        background: "var(--surface-raised)",
      }}
    >
      <NoticeList>
        <Notice cause="pin-dormant:ide" link={{ label: "Open the pins", onPress: () => undefined }}>
          The pin on ide has not been used in a month.
        </Notice>
        <Notice
          cause="window-trouble"
          tone="trouble"
          fixes={[{ label: "Try again", onPress: () => undefined }]}
          onDismiss={() => undefined}
        >
          The sample project could not be saved: its remote refused the push.
        </Notice>
      </NoticeList>
    </div>
  ),
  play: async ({ canvas, userEvent }) => {
    await waitFor(() => expect(canvas.getAllByRole("status")).toHaveLength(2));
    // Trouble is listed first, whatever order the two were written in.
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Try again" })).toHaveFocus();
    expectFocusInSight();
  },
};

/** A Notice on a page, standing alone: the inline surface. */
export const Alone: Story = {
  render: () => (
    <div
      style={{ inlineSize: "28rem", padding: "var(--space-5)", background: "var(--surface-base)" }}
    >
      <Notice
        cause="sandbox-hosts"
        fixes={[{ label: "Confirm", onPress: () => undefined }]}
        onDismiss={() => undefined}
      >
        2 hosts a chat added wait for your Confirm.
      </Notice>
    </div>
  ),
  play: async ({ canvas, userEvent }) => {
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Confirm" })).toHaveFocus();
    expectFocusInSight();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Dismiss" })).toHaveFocus();
  },
};
