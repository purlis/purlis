// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A story starts from the same input modality whatever the story before it did** (#5040,
 * `.storybook/at-rest.setup.ts`). The stories of a file run one after another on one page, and
 * React Aria remembers on that page whether the last input was a key or a press: a control that
 * takes the focus draws its focus ring only when it was not a press. So without the setup, a
 * story's picture would show the ring when the story ran alone and lose it when the story before
 * it ended on a press, or stopped part way through after one (#5022's `PoolUnavailable`).
 *
 * `EndsOnAPress` leaves the page as such a story would: its last input a press on words, with
 * nothing taking the focus after it. The two stories after it each take the focus as a story
 * does, as a dialog opens (`autoFocus`) and by a call (`focus()`, as a story's `play` moves it),
 * and each must draw its ring, as it does when it runs alone; each ends on a press too.
 *
 * The order of the exports is the test: each story after the first fails without the setup
 * only because of the press that ends the story before it, and passes when it runs alone.
 *
 * No pictures: the ring is what the picture would differ by, and it is asserted here.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import { Button } from "./ui/button/Button";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

function Pressable({ autoFocus = false }: { autoFocus?: boolean }) {
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <p style={{ margin: 0 }}>Words to press on</p>
      <Button autoFocus={autoFocus}>Cancel</Button>
    </div>
  );
}

const meta = {
  component: Pressable,
  parameters: { screenshot: false },
} satisfies Meta<typeof Pressable>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Presses the story's words, leaving the page as a story that ends on a press leaves it: its
 *  last input a press, and nothing taking the focus after it. */
async function pressTheWords({ canvas, userEvent }: Parameters<NonNullable<Story["play"]>>[0]) {
  await userEvent.click(canvas.getByText("Words to press on"));
}

/** **A story that ends on a press**, on its words. */
export const EndsOnAPress: Story = {
  play: pressTheWords,
};

/** **The focus taken as it is drawn, after a press**: the ring is drawn, as it is when this runs
 *  alone. It ends on a press too, for the story after it. */
export const FocusTakenAfterAPress: Story = {
  args: { autoFocus: true },
  play: async (context) => {
    const cancel = context.canvas.getByRole("button", { name: "Cancel" });
    await untilFocused(() => cancel, "Cancel, focused as it is drawn");
    expectFocusInSight();
    await expect(cancel).toHaveAttribute("data-focus-visible");
    await pressTheWords(context);
  },
};

/** **The focus moved by a call, after a press**: the ring is drawn, as it is when this runs
 *  alone. */
export const FocusMovedAfterAPress: Story = {
  play: async (context) => {
    const cancel = context.canvas.getByRole("button", { name: "Cancel" });
    cancel.focus();
    await untilFocused(() => cancel, "Cancel, focused by a call");
    expectFocusInSight();
    await expect(cancel).toHaveAttribute("data-focus-visible");
    await pressTheWords(context);
  },
};
