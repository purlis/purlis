// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Inbox's card for a chat waiting on the person's reply** (#4550): what the chat said under
 * the status line, over the lines it wrote and never cut, and the one line that says so where
 * the app heard no message. The person answers from here, so the reply box is walked to. What
 * the chat said is its markdown under the safe subset, folded past three lines (#5649). It is a
 * row of Asks (#5649): its chat and age on its first line, Answer and Go to chat among its links,
 * the reply a strip in the row, opened by Answer (#5651), whose send is its one button, and the ×
 * that ignores it over its age.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import type { Shown } from "./bindings";
import { Inbox, NO_MESSAGE, REPLY_HINT } from "./Inbox";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const QUESTION: Shown = {
  session: 6,
  ask: "question:6",
  says: "Waiting on your reply",
  options: [],
  source: "question",
  chain: ["steward 6"],
  answer: { via: "in-its-pane" },
};

/** The Inbox as wide as the window's side of it, holding one card. */
function OneCard({ last_message }: { last_message?: string }) {
  return (
    <div style={{ inlineSize: 420 }}>
      <Inbox
        plane="/plane"
        asks={[{ ...QUESTION, last_message }]}
        onGo={() => {}}
        onLeave={() => {}}
        onIgnore={() => {}}
      />
    </div>
  );
}

const meta = {
  component: OneCard,
} satisfies Meta<typeof OneCard>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The list is one Tab stop: Tab lands on the row, ↓ goes on (past its Show all, where it has
 *  one) to its Answer and its Go to chat, Enter on Answer opens the reply strip with the caret in
 *  it (#5651), and Shift+Tab leaves the list. */
async function theReplyBoxIsReached(canvasElement: HTMLElement) {
  const card = () => canvasElement.querySelector("li.inbox-ask");
  await userEvent.tab();
  await untilFocused(card, "Tab lands on the card");
  expectFocusInSight();
  await userEvent.keyboard("{ArrowDown}");
  // Words that wrap past three lines fold first: their Show all is the row's next stop.
  if ((document.activeElement?.textContent ?? "").startsWith("Show all"))
    await userEvent.keyboard("{ArrowDown}");
  await untilFocused(
    () => within(canvasElement).getByRole("button", { name: "Answer steward 6" }),
    "↓ reaches Answer, the first of the row's links",
  );
  expectFocusInSight();
  await userEvent.keyboard("{ArrowDown}");
  await untilFocused(
    () => within(canvasElement).getByRole("button", { name: "Go to chat steward 6" }),
    "↓ reaches Go to chat among the row's links",
  );
  expectFocusInSight();
  await userEvent.keyboard("{ArrowUp}{Enter}");
  await untilFocused(
    () => within(canvasElement).getByRole("textbox", { name: "Reply to steward 6" }),
    "Enter on Answer opens the reply strip with the caret in it",
  );
  expectFocusInSight();
  await userEvent.keyboard("{Escape}");
  await untilFocused(card, "Esc leaves the strip for the card");
  expectFocusInSight();
  await userEvent.tab({ shift: true });
  await waitFor(() => expect(card()?.contains(document.activeElement)).toBe(false));
}

/** What the chat said, over its lines, the status line above it once. */
export const WhatTheChatSaid: Story = {
  args: {
    last_message:
      "I checked both branches. main has the fix, and dev has the new tests.\n\nWhich should I build on, main or dev?",
  },
  play: async ({ canvasElement }) => {
    const card = within(canvasElement);
    await expect(card.getByText(/Which should I build on/)).toBeInTheDocument();
    await expect(card.getAllByText("Waiting on your reply")).toHaveLength(1);
    await theReplyBoxIsReached(canvasElement);
  },
};

/** The app heard no message: the card says so, once. */
export const NoMessageToShow: Story = {
  play: async ({ canvasElement }) => {
    const card = within(canvasElement);
    await expect(card.getAllByText(NO_MESSAGE)).toHaveLength(1);
    await expect(card.getAllByText("Waiting on your reply")).toHaveLength(1);
    await theReplyBoxIsReached(canvasElement);
  },
};

const LONG = `I'm waiting for #5141 to merge before the last part of #5160. Until then there's nothing left to do on PR #5163.

- **CI on the current head (\`07e0ff5\`):** the Rust tests pass now that the wrong expectation is fixed.
- **The one remaining failure:** stories WebKit 5/7, on \`SkillPage.stories.tsx\`. These are #4672's pictures, so I'm not pulling any.
- **Still to do:** remove the Notices-centre \`VaultRefusedNotice\` for a chat with no pane, and add the vault rows to \`noticePlace.test.ts\`.

I've sent you a note saying this. When you tell me #5141 has merged, I'll rebase onto main, do that last part, run tsc and eslint, and push.`;

/** **A long message** (#5649): folded at three lines behind Show all · N lines, which ↓ reaches
 *  first; opened, it shows whole, and ↓ goes on to Answer, which opens the reply strip (#5651). */
export const ALongMessage: Story = {
  args: { last_message: LONG },
  play: async ({ canvasElement }) => {
    const card = within(canvasElement);
    await expect(card.getAllByText("Still to do:")[0]?.tagName).toBe("STRONG");
    const show = () => card.getByRole("button", { name: /^Show all · \d+ lines$/ });
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(show, "↓ reaches Show all, the row's first stop");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await untilFocused(
      () => card.getByRole("button", { name: "Show less" }),
      "Enter opens it whole",
    );
    expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(
      () => card.getByRole("button", { name: "Answer steward 6" }),
      "↓ goes on to Answer",
    );
    await userEvent.keyboard("{Enter}");
    await untilFocused(
      () => card.getByRole("textbox", { name: "Reply to steward 6" }),
      "Enter on Answer opens the reply box in the row",
    );
    expectFocusInSight();
  },
};

/** **The reply strip held** (#5214): two lines and its hint once it holds the caret, its send the
 *  accent's once there is something to send, Shift+Enter a new line. */
export const TheReplyStripHeld: Story = {
  args: { last_message: "Which should I build on, main or dev?" },
  play: async ({ canvasElement }) => {
    const card = within(canvasElement);
    // Its strip stands behind its Answer link (#5651).
    await userEvent.click(card.getByRole("button", { name: "Answer steward 6" }));
    const box = await card.findByRole("textbox", { name: "Reply to steward 6" });
    const strip = () => canvasElement.querySelector("[data-foot-strip]");
    // Answer puts the caret in it, so it opens at two lines.
    await userEvent.click(box);
    await waitFor(() => expect(strip()).toHaveAttribute("data-rows", "2"));
    await expect(card.getByText(REPLY_HINT)).toHaveAttribute("data-shown");
    await userEvent.keyboard("main,{Shift>}{Enter}{/Shift}then rebase");
    await expect(box).toHaveValue("main,\nthen rebase");
    await expect(card.getByRole("button", { name: "Send reply to steward 6" })).toHaveAttribute(
      "data-variant",
      "primary",
    );
    expectFocusInSight();
  },
};
