// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Keyboard triage in Asks** (ADR 0121 decision 9, #6016): begun as ⌘⇧I begins it, from a
 * button standing in for the chat the keyboard was in. It lands on the first ask's lead (a
 * sandbox host's Keep blocked, never its Allow), the later key goes on to the next ask's Answer
 * and round again, and Esc goes back where the keyboard was.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import type { Shown } from "./bindings";
import { Inbox } from "./Inbox";
import { beginTriage, endTriage, landOnFirst } from "./asksTriage";
import { onAMac } from "./mac";
import { Button } from "./ui/button/Button";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const HOST = {
  session: 4,
  ask: "block:4:connect:host:api.example.com",
  says: "Reach api.example.com",
  options: [
    { id: "chat", label: "Allow only for this chat", allows: true },
    { id: "keep", label: "Keep blocked", allows: false },
  ],
  source: "sandbox-host",
  chain: ["steward 4"],
  answer: {
    via: "sandbox-block",
    shown: { operation: "connect", kind: "host", what: "host", target: "api.example.com" },
  },
} as Shown;

/** A chat waiting on the person's reply: answered behind its Answer. */
const QUESTION: Shown = {
  session: 5,
  ask: "reply:5",
  says: "Which branch should I rebase onto?",
  options: [],
  source: "question",
  chain: ["steward 5"],
  answer: { via: "in-its-pane" },
};

/** Where the keyboard was before ⌘⇧I, and Asks beside it, as wide as the window's side. */
function Triage() {
  return (
    <div style={{ inlineSize: 420 }}>
      <Button variant="secondary" size="sm">
        Chat steward 1
      </Button>
      <Inbox
        part="asks"
        plane="/plane"
        asks={[HOST, QUESTION]}
        onGo={() => {}}
        onLeave={() => {}}
      />
    </div>
  );
}

const meta = {
  component: Triage,
} satisfies Meta<typeof Triage>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The later key, as this keyboard types it: ⌘⌥L on a Mac, Alt+Shift+L elsewhere. */
const later = () =>
  userEvent.keyboard(
    onAMac() ? "{Meta>}{Alt>}[KeyL]{/Alt}{/Meta}" : "{Alt>}{Shift>}[KeyL]{/Shift}{/Alt}",
  );

/** **⌘⇧I, later, and back**: the first ask's Keep blocked, the next ask's Answer, the first
 *  again, and Esc to where the keyboard was. */
export const LaterAndBack: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const chat = canvas.getByRole("button", { name: "Chat steward 1" });
    chat.focus();
    beginTriage(document.activeElement);
    landOnFirst(canvasElement.querySelector(".inbox-list"));
    const keep = () => canvas.getByRole("button", { name: "Keep blocked" });
    await untilFocused(keep, "⌘⇧I lands on Keep blocked, never the Allow");
    expectFocusInSight();
    await later();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Answer steward 5" }),
      "later goes on to the next ask's Answer",
    );
    expectFocusInSight();
    await later();
    await untilFocused(keep, "later past the last comes round to the first");
    await userEvent.keyboard("{Escape}");
    await untilFocused(() => chat, "Esc goes back where the keyboard was");
    await expect(chat).toHaveFocus();
    endTriage();
  },
};
