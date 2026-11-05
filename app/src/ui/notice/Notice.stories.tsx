// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The notice family** (#2912): a pane's row of Notices with an Allow beside Keep blocked and
 * every other Notice under it (#4754: no cap), the Inbox's list with news and trouble, and a Notice standing alone on
 * a page, each walked by the keyboard. Each held to axe and compared with its baseline
 * (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState, type ReactNode } from "react";
import { expect, waitFor } from "storybook/test";
import { Notice, NoticeInCentre, NoticeList, NoticeOf, NoticePaneRow } from "../../Notice";
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

/** Tab reaches Allow, then Keep blocked, then each next Notice's Dismiss: every one stands, with
 *  no "+N more" (#4754), and Esc on a Notice puts it away, the keyboard going on to the next. */
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
    await expect(canvas.getAllByRole("button", { name: "Dismiss" })[1]).toHaveFocus();
    expectFocusInSight();
    await expect(canvas.getByRole("status", { name: "Guessed" })).toBeVisible();
    await expect(canvas.queryByRole("button", { name: /more/ })).toBeNull();
  },
};

/** The Inbox's list: trouble first, then news, each a card of its own (#5217). */
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
    // Trouble is listed first, whatever order the two were written in: its × in its head, then
    // its answer under its words.
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Dismiss" })).toHaveFocus();
    expectFocusInSight();
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
 * shows its mark, a question in a circle, so it is not told by colour alone. The picture is the three of them.
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
      // A pane's line says it to a reader with a colon, a Notices centre card in its head (#5217).
      await expect(box).toHaveTextContent(/Needs a decision/);
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
    // The card's × at its top right, then its fix.
    await userEvent.tab();
    await untilFocused(
      () => canvas.getAllByRole("button", { name: "Dismiss" })[0],
      "Needs a decision: Tab lands on the card's ×",
    );
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Review the change" }),
      "Needs a decision: Tab goes on to the card's fix",
    );
    expectFocusInSight();
  },
};

/** No chat's row to tell. */
const heldNowhere = () => undefined;

/** The Notices centre as the Inbox draws it: as wide as the right sidebar. */
const inTheCentre = (notice: ReactNode) => (
  <div style={{ inlineSize: "22rem" }}>
    <NoticeList>
      <NoticeInCentre.Provider value={heldNowhere}>{notice}</NoticeInCentre.Provider>
    </NoticeList>
  </div>
);

/** **A note of a chat in no pane, in the Notices centre** (#4759, #5217): whose it is, its state and age
 *  beneath, Go to chat a quiet link and its ×, and its one answer the primary button. */
export const CentreNote: Story = {
  render: () =>
    inTheCentre(
      <NoticeOf.Provider value={{ whose: "steward 4", onGo: () => undefined }}>
        <Notice
          cause="mcp-failed:4:github"
          at="pane"
          tone="trouble"
          fixes={[{ label: "Retry", onPress: () => undefined }]}
          link={{ label: "See its settings", onPress: () => undefined }}
          onDismiss={() => undefined}
        >
          MCP server github could not start, twice, so its tools are not in steward 4.
        </Notice>
      </NoticeOf.Provider>,
    ),
  play: async ({ canvas: card, userEvent }) => {
    const go = await card.findByRole("button", { name: "Go to chat steward 4" });
    await userEvent.tab();
    await expect(go).toHaveFocus();
    expectFocusInSight();
    await userEvent.tab();
    await expect(card.getByRole("button", { name: "Dismiss" })).toHaveFocus();
    expectFocusInSight();
    await userEvent.tab();
    await expect(card.getByRole("button", { name: "Retry" })).toHaveFocus();
    expectFocusInSight();
    await userEvent.tab();
    await expect(card.getByRole("button", { name: "See its settings" })).toHaveFocus();
    expectFocusInSight();
    // The picture is of the card at rest, never of a way the keyboard is on.
    (document.activeElement as HTMLElement).blur();
  },
};

/** **A note with no name of its own, and a plain one with no way but its ×** (#5217). */
export const CentreNoteAlone: Story = {
  play: async ({ canvas, userEvent }) => {
    await waitFor(() => expect(canvas.getAllByRole("status")).toHaveLength(2));
    await userEvent.tab();
    await expect(canvas.getAllByRole("button", { name: "Dismiss" })[0]).toHaveFocus();
    expectFocusInSight();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Undo" })).toHaveFocus();
    expectFocusInSight();
    (document.activeElement as HTMLElement).blur();
  },
  render: () =>
    inTheCentre(
      <>
        <Notice
          cause="pin-forgotten:ide"
          fixes={[{ label: "Undo", onPress: () => undefined }]}
          onDismiss={() => undefined}
        >
          Forgot the pin to ide.
        </Notice>
        <Notice cause="chat-fresh:7" label="Started fresh" onDismiss={() => undefined}>
          This chat started fresh: its harness kept no conversation.
        </Notice>
      </>,
    ),
};

/** **An arrival card in the Notices centre** (#4759, #5217): Go to chat a quiet link in its head,
 *  and no primary button, since it asks nothing. */
export const CentreArrival: Story = {
  play: async ({ canvas, userEvent }) => {
    const go = await canvas.findByRole("button", { name: "Go to chat" });
    await userEvent.tab();
    await expect(go).toHaveFocus();
    expectFocusInSight();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Dismiss" })).toHaveFocus();
    expectFocusInSight();
    (document.activeElement as HTMLElement).blur();
  },
  render: () =>
    inTheCentre(
      <Notice
        cause="result:chat.arrived:1"
        title="steward 6 arrived from steward 4"
        quiet
        fixes={[{ label: "Go to it", onPress: () => undefined, primary: true, go: true }]}
        onDismiss={() => undefined}
      >
        {undefined}
      </Notice>,
    ),
};

/** **A sandbox block in the Notices centre** (ADR 0097 §6, #5217): its full waiting edge, Allow
 *  beside Keep blocked at equal weight, no primary button, and what opens more a quiet link. */
export const CentreSandboxBlock: Story = {
  play: async ({ canvas, userEvent }) => {
    await canvas.findByRole("button", { name: "Go to chat steward 4" });
    await userEvent.tab();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Dismiss" })).toHaveFocus();
    await userEvent.tab();
    // Nothing moves the keyboard onto Allow but a Tab the person pressed (ADR 0097 §6).
    await expect(canvas.getByRole("button", { name: "Allow for this chat" })).toHaveFocus();
    expectFocusInSight();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Keep blocked" })).toHaveFocus();
    expectFocusInSight();
    (document.activeElement as HTMLElement).blur();
  },
  render: () =>
    inTheCentre(
      <NoticeOf.Provider value={{ whose: "steward 4", onGo: () => undefined }}>
        <Notice
          cause="sandbox-blocked:4:network:connect:chat"
          at="pane"
          tone="trouble"
          label="Sandbox block"
          consent
          fixes={[
            { label: "Allow for this chat", onPress: () => undefined, allows: true },
            { label: "Keep blocked", onPress: () => undefined },
            {
              label: "Show the request",
              onPress: () => undefined,
              opens: { id: "the-request", open: false },
            },
          ]}
          onDismiss={() => undefined}
        >
          The sandbox blocked a connection to api.github.com.
        </Notice>
      </NoticeOf.Provider>,
    ),
};

/** **A chat's own words on a card** (#5217): markdown under the safe subset, folded past six
 *  lines, with purlis's sentence above them in plain words. */
export const CentreChatWords: Story = {
  play: async ({ canvas, userEvent }) => {
    const all = await canvas.findByRole("button", { name: /^Show all/ });
    all.focus();
    await userEvent.keyboard("{Enter}");
    const less = await canvas.findByRole("button", { name: "Show less" });
    await expect(less).toHaveAttribute("aria-expanded", "true");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await canvas.findByRole("button", { name: /^Show all/ });
    (document.activeElement as HTMLElement).blur();
  },
  render: () =>
    inTheCentre(
      <NoticeOf.Provider value={{ whose: "steward 4", onGo: () => undefined }}>
        <Notice
          cause="passed-on:4"
          at="pane"
          label="Passed on"
          chatWords={[
            "**Read this first.** The login fails on the second try:",
            "",
            "- the token is `expired`",
            "- the retry reuses it",
            "- the session is lost",
            "- nothing is logged",
            "- the user sees a blank page",
            "- the next launch is fine",
          ].join("\n")}
          onDismiss={() => undefined}
        >
          steward 4 passed this on.
        </Notice>
      </NoticeOf.Provider>,
    ),
};
