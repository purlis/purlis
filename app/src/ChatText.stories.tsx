// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a chat wrote, as markdown under the safe subset** (#5214): a short message drawn whole,
 * and a long one folded at six lines behind "Show all · N lines", opened and folded again from
 * the keyboard. Its link shows its host and its picture only its alt text. Drawn in every cell
 * of the theme matrix.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { ChatText } from "./ChatText";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const SHORT =
  "I checked both branches. **main** has the fix, and `dev` has the new tests.\n\nWhich should I build on, main or dev?";

const LONG = `I'm waiting for #5141 to merge before the last part of #5160. Until then there's nothing left to do on PR #5163.

- **CI on the current head (\`07e0ff5\`):** the Rust tests pass now that the wrong expectation is fixed.
- **The one remaining failure:** stories WebKit 5/7, on \`SkillPage.stories.tsx\`. These are #4672's pictures, so I'm not pulling any.
- **Still to do:** remove the Notices-centre \`VaultRefusedNotice\` for a chat with no pane.

| leg | p95 |
| --- | --- |
| claude | 302 ms |
| codex | 334 ms |

The run is [on GitHub](https://github.com/purlis/purlis/actions) and ![the graph](https://example.com/graph.png) is attached.

When you tell me #5141 has merged, I'll rebase onto main, do that last part, run tsc and eslint, and push.`;

/** A chat's text as wide as an Inbox card's words. */
function Said({ text }: { text: string }) {
  return (
    <div style={{ inlineSize: 380 }}>
      <ChatText text={text} />
    </div>
  );
}

const meta = {
  component: Said,
} satisfies Meta<typeof Said>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A short message, drawn whole: its bold and its code as themselves, nothing to press. */
export const Short: Story = {
  args: { text: SHORT },
  play: async ({ canvasElement }) => {
    const said = within(canvasElement);
    await expect(said.getByText("main").tagName).toBe("STRONG");
    await expect(said.queryByRole("button")).toBeNull();
  },
};

/** A long message, folded at six lines with a fade: Show all opens it whole, from the keyboard,
 *  and Show less folds it again. */
export const Long: Story = {
  args: { text: LONG },
  play: async ({ canvasElement }) => {
    const said = within(canvasElement);
    await expect(canvasElement.querySelector("img, [src]")).toBeNull();
    const show = () => said.getByRole("button", { name: /^Show all · \d+ lines$/ });
    await expect(show()).toHaveAttribute("aria-expanded", "false");
    await userEvent.tab();
    await untilFocused(show, "Tab reaches Show all");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    const less = () => said.getByRole("button", { name: "Show less" });
    await untilFocused(less, "Enter opens it, the keyboard staying on its button");
    expectFocusInSight();
    await expect(less()).toHaveAttribute("aria-expanded", "true");
    await userEvent.keyboard("{Enter}");
    await untilFocused(show, "Enter folds it again");
  },
};

/** A long message, opened: every line, its table and its link with the host beside it. */
export const LongOpened: Story = {
  args: { text: LONG },
  play: async ({ canvasElement }) => {
    const said = within(canvasElement);
    await userEvent.click(said.getByRole("button", { name: /^Show all/ }));
    await expect(said.getByRole("table")).toBeVisible();
    await expect(said.getByRole("link", { name: "on GitHub" })).toBeVisible();
    await expect(said.getByText("(github.com)")).toBeVisible();
    await expect(said.getByText("the graph")).toBeVisible();
  },
};
