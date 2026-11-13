// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Waiting in its terminal** (R-292, T1-35): a chat whose permission hook gave up waits on its
 * harness's own prompt, which only its pane answers. Its row in Asks offers Go to chat, and no
 * Allow and no word for no.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";
import type { Shown } from "./bindings";
import { Inbox } from "./Inbox";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

/** Chat 4 as the core lists it once its hook gave up (`asking::terminal`). */
const TERMINAL: Shown = {
  session: 4,
  ask: "terminal:4",
  says: "Waiting in its terminal: a permission prompt",
  options: [],
  source: "terminal",
  chain: ["steward 4"],
  answer: { via: "in-its-pane" },
};

function InItsTerminal({ onGo }: { onGo: (session: number) => void }) {
  return (
    <div style={{ inlineSize: 420 }}>
      <Inbox part="asks" plane="/plane" asks={[TERMINAL]} onGo={onGo} onLeave={() => {}} />
    </div>
  );
}

const meta = {
  component: InItsTerminal,
  args: { onGo: fn() },
} satisfies Meta<typeof InItsTerminal>;

export default meta;
type Story = StoryObj<typeof meta>;

/** One action, Go to chat, reached by the keyboard; nothing on the row answers the prompt. */
export const GoToChatOnly: Story = {
  play: async ({ canvasElement, args }) => {
    const rows = () => [...canvasElement.querySelectorAll<HTMLElement>('[role="row"].inbox-ask')];
    await expect(rows()).toHaveLength(1);
    const row = within(rows()[0]);
    await expect(row.getByText("Waiting in its terminal: a permission prompt")).toBeInTheDocument();
    for (const word of [/^Allow/, /^Deny/, /^Don't allow/, /^Not now/]) {
      await expect(row.queryByRole("button", { name: word })).toBeNull();
    }
    await expect(row.queryByRole("textbox")).toBeNull();

    await userEvent.tab();
    await untilFocused(() => rows()[0], "Tab lands on the row");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => row.getByRole("button", { name: "Go to chat steward 4" }),
      "→ reaches Go to chat, the row's first link",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onGo).toHaveBeenCalledWith(4);
  },
};
