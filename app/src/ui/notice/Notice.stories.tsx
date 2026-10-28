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
import { expectFocusInSight, untilFocused } from "../../stories.testkit";

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

/**
 * **What needs a decision** (#3678): nothing went wrong, but the person must choose. A card alone
 * on a page (a skill a chat changed), a line in the Inbox's list between trouble and news, and a
 * line in a pane. Each is a polite status, never an alert, says "Needs a decision" to a reader and
 * shows its triangle, so it is not told by colour alone. The picture is the three of them.
 */
export const NeedsADecision: Story = {
  render: () => (
    <div
      style={{
        inlineSize: "28rem",
        display: "grid",
        gap: "var(--space-5)",
        padding: "var(--space-5)",
        background: "var(--surface-base)",
      }}
    >
      <Notice
        cause="skill-changed:grilling"
        tone="attention"
        title="A chat changed grilling"
        fixes={[{ label: "Review the change", onPress: () => undefined, primary: true }]}
        onDismiss={() => undefined}
      >
        Chats keep the version you accepted until you review this one.
      </Notice>
      <NoticeList>
        <Notice cause="pin-dormant:ide" link={{ label: "Open the pins", onPress: () => undefined }}>
          The pin on ide has not been used in a month.
        </Notice>
        <Notice
          cause="draft-stale:review-pr"
          tone="attention"
          fixes={[{ label: "Ask the chat to redo it", onPress: () => undefined }]}
        >
          The draft of review-pr is out of date: the project&apos;s copy changed since.
        </Notice>
        <Notice
          cause="window-trouble"
          tone="trouble"
          fixes={[{ label: "Try again", onPress: () => undefined }]}
        >
          The sample project could not be saved: its remote refused the push.
        </Notice>
      </NoticeList>
      <div style={{ display: "flex", flexDirection: "column", blockSize: "8rem" }}>
        <NoticePaneRow>
          <Notice
            cause="grade-plan-changed:7"
            at="pane"
            tone="attention"
            label="Grade plan changed"
            fixes={[{ label: "Look at the plan", onPress: () => undefined }]}
          >
            The grade plan changed while this chat ran.
          </Notice>
        </NoticePaneRow>
      </div>
    </div>
  ),
  play: async ({ canvas, canvasElement, userEvent }) => {
    // Every one is a polite status, and none an alert.
    await waitFor(() => expect(canvas.getAllByRole("status").length).toBeGreaterThanOrEqual(5));
    await expect(canvas.queryByRole("alert")).toBeNull();
    const boxes = [
      ...canvasElement.querySelectorAll<HTMLElement>(
        '[data-scope="notice"][data-part="root"][data-variant="attention"]',
      ),
    ];
    await expect(boxes).toHaveLength(3);
    for (const box of boxes) {
      await expect(box).toHaveTextContent(/Needs a decision:/);
    }
    // Its mark on the card and on the Inbox's line; the pane's band draws its own.
    await expect(boxes[0].querySelector('[data-part="mark"] svg')).not.toBeNull();
    await expect(boxes[1].querySelector('[data-part="mark"] svg')).not.toBeNull();
    // In the Inbox's list it stands after trouble and before news.
    const order = [
      ...canvasElement.querySelectorAll<HTMLElement>(
        '[data-scope="notice-list"] [data-scope="notice"][data-part="root"]',
      ),
    ].map((one) => one.getAttribute("data-variant"));
    await expect(order).toEqual(["trouble", "attention", "news"]);
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Review the change" }),
      "Needs a decision: Tab lands on the card's fix",
    );
    expectFocusInSight();
  },
};
