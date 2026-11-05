// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A story starts with nothing under the pointer, wherever the story before it left it** (#5063,
 * `.storybook/pointer.setup.ts`). The stories of a file run one after another on one page, and the
 * engine's own pointer stays where it last was: over a point the next story may draw on, whose
 * control would then be hovered (`:hover`, and React Aria's `data-hovered`) and drawn as it is
 * not when that story runs alone.
 *
 * `EndsOnAPress` presses a button with the engine's pointer, the only one that leaves `:hover`
 * behind: a story's own `userEvent` only dispatches events. `NothingUnderThePointer` draws the same
 * button in the same place, and must find it not hovered.
 *
 * The order of the exports is the test: the second story fails without the setup only because of
 * where the first left the pointer, and passes when it runs alone.
 *
 * No pictures: the hover is what the picture would differ by, and it is asserted here.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor } from "storybook/test";
import { Button } from "./ui/button/Button";
import { pressWithTheEnginesPointer } from "./stories.testkit";

/** Long enough for the engine to tell the page what is under its pointer, as it does when a page
 *  lays out under a pointer that has not moved. Nothing on the page says it has: a pointer that
 *  hovers nothing makes no event to wait for. */
const THE_ENGINE_REPORTS_MS = 300;

function Pressable() {
  return <Button>Cancel</Button>;
}

const meta = {
  component: Pressable,
  parameters: { screenshot: false },
} satisfies Meta<typeof Pressable>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **A story that ends on a press with the engine's pointer**, which it leaves on the button. */
export const EndsOnAPress: Story = {
  play: async ({ canvas }) => {
    const cancel = canvas.getByRole("button", { name: "Cancel" });
    if (!(await pressWithTheEnginesPointer(cancel))) return;
    await waitFor(() => expect(cancel.matches(":hover")).toBe(true));
  },
};

/** **The same button in the same place, after that press**: not hovered, as it is not when this
 *  runs alone. */
export const NothingUnderThePointer: Story = {
  play: async ({ canvas }) => {
    const cancel = canvas.getByRole("button", { name: "Cancel" });
    await new Promise((done) => setTimeout(done, THE_ENGINE_REPORTS_MS));
    await expect(cancel.matches(":hover")).toBe(false);
    await expect(cancel).not.toHaveAttribute("data-hovered");
  },
};
