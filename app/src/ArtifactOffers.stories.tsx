// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The actions an artifact's page offers, on purlis's bar** (#5972, ADR 0120 §8): each in the
 * catalogue's words, a delete marked and asking first, and a refused offer's sentence.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";
import type { Offer } from "./actions";
import { OffersDrawn } from "./ArtifactOffers";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const DONE: Offer = {
  id: "todo.done:fix-login",
  title: "Mark done: Fix login",
  available: true,
  reason: "",
  does: { verb: "closeTodo", workspace: "alpha", slug: "fix-login" },
  note: "Marks it done in alpha; the workspace's journal records it, and it stays, struck through, to reopen.",
};

const DELETE: Offer = {
  id: "todo.delete:old-one",
  title: "Delete done todo Old one",
  available: true,
  reason: "",
  does: { verb: "forgetDoneTodo", workspace: "alpha", slug: "old-one" },
  note: "Deletes it from alpha's done todos with nothing journalled. There is no undo.",
};

const meta: Meta<typeof OffersDrawn> = {
  title: "Views/Pinned artifact/Offers",
  component: OffersDrawn,
  args: { name: "Todo board", onPress: fn() },
  decorators: [
    (Story) => (
      <header className="artifact-bar">
        <Story />
      </header>
    ),
  ],
};
export default meta;

/** Two offers and one refused: the buttons are purlis's, and Tab walks them in order. */
export const Offered: StoryObj<typeof OffersDrawn> = {
  args: {
    drawn: [
      { key: DONE.id, offer: DONE },
      { key: DELETE.id, offer: DELETE, asks: "Delete" },
      {
        key: "workspace.remove:alpha",
        refused: "A page cannot offer workspace.remove, so purlis did not draw it",
      },
    ],
  },
  play: async ({ canvasElement, args }) => {
    const bar = within(canvasElement);
    expect(bar.getByRole("group", { name: "Offered by Todo board" })).toBeVisible();
    await userEvent.tab();
    await untilFocused(
      () => bar.getByRole("button", { name: "Mark done: Fix login" }),
      "Tab reaches the first offer",
    );
    await expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    expect(args.onPress).toHaveBeenCalledWith(DONE);
    await userEvent.tab();
    await untilFocused(
      () => bar.getByRole("button", { name: "Delete done todo Old one…" }),
      "Tab reaches the delete",
    );
    await expectFocusInSight();
    expect(bar.getByTestId("artifact-offer-refused").textContent).toContain("workspace.remove");
  },
};

/** A delete the page offered asks first, starting on Cancel; Esc leaves it undone. */
export const DeleteAsks: StoryObj<typeof OffersDrawn> = {
  // The dialog draws over the page, outside the story's frame.
  parameters: { screenshot: "page" },
  args: { drawn: [{ key: DELETE.id, offer: DELETE, asks: "Delete" }] },
  play: async ({ canvasElement, args }) => {
    const page = within(canvasElement.ownerDocument.body);
    await userEvent.click(within(canvasElement).getByRole("button", { name: /Delete done todo/ }));
    const cancel = await page.findByRole("button", { name: "Cancel" });
    await untilFocused(() => cancel, "It starts on Cancel");
    await userEvent.tab();
    await untilFocused(() => page.getByRole("button", { name: "Delete" }), "Tab reaches Delete");
    await expectFocusInSight();
    await userEvent.keyboard("{Escape}");
    expect(args.onPress).not.toHaveBeenCalled();
  },
};
