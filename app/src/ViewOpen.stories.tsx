// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Opening an extension's view, and its offer** (#4769; prototype `view-open-beside`): a
 * view's button with its beside icon after it, in a pane roomy enough to split and in one too
 * narrow, where the icon is greyed with its reason; and an extension's offer as one quiet note.
 * Drawn in every cell of the theme matrix.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { ChartColumn } from "lucide-react";
import { expect, fn } from "storybook/test";
import { PANE } from "./paneDrag";
import { expectFocusInSight, untilFocused } from "./stories.testkit";
import { OfferNotice, ViewOpenButton } from "./ViewOpen";

const VIEW = { from: "statistics", view: "stats", key: "steward" };

/** A pane of `width`, marked as the window marks one, holding the view's heading. */
function Heading({ width, onOpen }: { width: number; onOpen: (how: "tab" | "beside") => void }) {
  return (
    <div
      {...{ [PANE]: "7" }}
      style={{ inlineSize: width, blockSize: 320, padding: "var(--space-4)" }}
    >
      <div className="view-head">
        <ViewOpenButton
          view={VIEW}
          name="Statistics · steward"
          where="this"
          pane={7}
          tooltip="Asks statistics, in a tab of its own"
          icon={<ChartColumn className="node-icon" aria-hidden="true" />}
          onOpen={onOpen}
        >
          Statistics
        </ViewOpenButton>
      </div>
    </div>
  );
}

const meta = {
  component: Heading,
  args: { width: 900, onOpen: fn() },
} satisfies Meta<typeof Heading>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Roomy enough to split**: Tab reaches the button and then its beside icon, and Enter on
 *  each opens a tab and then beside. */
export const BesideThisPane: Story = {
  play: async ({ canvas, args, userEvent }) => {
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Statistics" }), "Tab to the view");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onOpen).toHaveBeenLastCalledWith("tab");
    await userEvent.tab();
    const beside = canvas.getByRole("button", {
      name: "Open Statistics · steward beside this pane (⌥-press)",
    });
    await untilFocused(() => beside, "Tab to its beside icon");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onOpen).toHaveBeenLastCalledWith("beside");
  },
};

/** **Too narrow to split**: the beside icon is greyed, says why, and keeps the focus. */
export const TooNarrowToOpenBeside: Story = {
  args: { width: 520 },
  play: async ({ canvas, args, userEvent }) => {
    const beside = await canvas.findByRole("button", {
      name: "Too narrow to open beside: each side needs 40 columns",
    });
    await expect(beside).toHaveAttribute("aria-disabled", "true");
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(() => beside, "Tab to the greyed beside icon");
    await userEvent.keyboard("{Enter}");
    await expect(args.onOpen).not.toHaveBeenCalled();
  },
};

/** **An extension's offer, in the Notices centre**: Open, Open beside, Stop offers and Dismiss,
 *  each reached by the keyboard. */
export const AnOffer: StoryObj<typeof OfferNotice> = {
  render: (args) => (
    <div style={{ inlineSize: 720, padding: "var(--space-4)" }}>
      <OfferNotice {...args} />
    </div>
  ),
  args: {
    offer: { extension: "statistics", view: "stats", key: "steward" },
    declared: [{ extension: "statistics", id: "stats", title: "Statistics", about: "personas" }],
    at: "inbox",
    onOpen: fn(),
    onDismiss: fn(),
    onStop: fn(),
  },
  play: async ({ canvas, args, userEvent }) => {
    await expect(canvas.getByText("statistics offers a view · Statistics · steward")).toBeVisible();
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Open" }), "Tab to Open");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onOpen).toHaveBeenCalledWith(VIEW, "Statistics · steward", "tab");
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Stop offers from statistics" }),
      "Tab to Stop offers",
    );
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Dismiss" }), "Tab to Dismiss");
    await userEvent.keyboard("{Enter}");
    await expect(args.onDismiss).toHaveBeenCalled();
  },
};
