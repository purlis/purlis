// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A project's or the app's result, popped from the status bar** (#3653, spec #3647, the
 * accepted prototype `prototype/chip-notices`): a refused pin, as the card a workspace's notice
 * is on its chip, hung above the status bar's Notices item and pointing down at it, over the
 * window, pushing nothing. Hidden by a press elsewhere, it leaves a ● dot on the Notices item's
 * leading corner, which brings it back; × and Esc dismiss it. Each held to axe, walked by the
 * keyboard and compared with its baseline (`docs/component-stories.md`). The card draws outside
 * its frame, so the pictures are the page's.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import { AT_STATUS, type ChipNotice, type ChipNotices } from "./ChipNotices";
import { StatusLine } from "./StatusLine";
import { untilFocused } from "./stories.testkit";

const REFUSED = "purlis pins at most 12 workspaces in one project. Unpin one first.";

function TheWindow({ startOpen = true }: { startOpen?: boolean }) {
  const [open, setOpen] = useState<string | undefined>(startOpen ? AT_STATUS : undefined);
  const [kept, setKept] = useState<readonly ChipNotice[]>(() => [
    { id: 1, kind: "info", said: { from: "workspace.pin", refused: true, words: REFUSED } },
  ]);
  const notices: ChipNotices = {
    on: new Map(kept.length === 0 ? [] : [[AT_STATUS, kept]]),
    open: kept.length === 0 ? undefined : open,
    show: setOpen,
    hide: () => setOpen(undefined),
    dismiss: (id) => setKept((was) => was.filter((one) => one.id !== id)),
    onSettings: () => undefined,
  };
  return (
    <div style={{ display: "grid", inlineSize: "48rem" }}>
      {/* What the window holds over the bar, which a card must never move. */}
      <div
        data-testid="page"
        style={{
          display: "flex",
          flexDirection: "column",
          justifyContent: "space-between",
          blockSize: 320,
          background: "var(--surface-base)",
        }}
      >
        <p style={{ margin: 0, padding: "var(--space-4)" }}>The work</p>
        {/* Somewhere to press that no card is over, whatever a theme's metrics: the page's
            top end corner. */}
        <span
          data-testid="elsewhere"
          style={{ alignSelf: "flex-end", inlineSize: 40, blockSize: 40, order: -1 }}
        />
      </div>
      <StatusLine
        plane="/p/purlis-demo"
        read
        where="3rd-try"
        workspaces={4}
        state={{ pieces: {}, piecesRefused: {}, reading: false }}
        alerts={{ count: 2, open: () => undefined }}
        notices={notices}
      />
    </div>
  );
}

const meta = {
  component: TheWindow,
  parameters: { screenshot: "page" },
} satisfies Meta<typeof TheWindow>;

export default meta;
type Story = StoryObj<typeof meta>;

const centre = (box: DOMRect) => box.left + box.width / 2;
const cardOf = () => screen.queryByRole("region", { name: "Notices for purlis-demo" });
const surfaceOf = (card: HTMLElement) =>
  card.closest<HTMLElement>('[data-scope="popover"][data-part="root"]') as HTMLElement;

/**
 * **The refused pin, popped from Notices**: above the item with its point on it, over the
 * window, taking no keyboard. A press elsewhere hides it and moves nothing; the dot it leaves
 * brings it back with the keyboard in it.
 */
export const PopsFromNotices: Story = {
  play: async ({ canvas, userEvent }) => {
    const card = await waitFor(() => cardOf() as HTMLElement);
    await expect(card).toHaveTextContent(REFUSED);
    await expect(card.contains(document.activeElement)).toBe(false);
    // Above the Notices item, its point on the item's middle, pointing down.
    const item = canvas.getByTestId("status-alerts").getBoundingClientRect();
    const surface = surfaceOf(card);
    await waitFor(() => expect(surface).toHaveAttribute("data-placement", "top"));
    await expect(surface.getBoundingClientRect().bottom).toBeLessThan(item.top);
    const point = surface.querySelector('[data-part="point"]') as HTMLElement;
    await expect(
      Math.abs(centre(point.getBoundingClientRect()) - centre(item)),
    ).toBeLessThanOrEqual(1.5);
    // It pushes nothing: the page and the bar are where they are with the card hidden.
    const page = canvas.getByTestId("page").getBoundingClientRect();
    const bar = canvas.getByTestId("status-line").getBoundingClientRect();
    await userEvent.click(canvas.getByTestId("elsewhere"));
    await waitFor(() => expect(cardOf()).toBeNull());
    await expect(canvas.getByTestId("page").getBoundingClientRect().toJSON()).toEqual(
      page.toJSON(),
    );
    await expect(canvas.getByTestId("status-line").getBoundingClientRect().toJSON()).toEqual(
      bar.toJSON(),
    );
    await expect(canvas.getByTestId("status-alerts").getBoundingClientRect().toJSON()).toEqual(
      item.toJSON(),
    );
    // The dot, a Tab stop of its own, brings it back with the keyboard in it.
    const dot = canvas.getByRole("button", { name: "Show the notice on the status bar" });
    dot.focus();
    await userEvent.keyboard("{Enter}");
    const again = await waitFor(() => cardOf() as HTMLElement);
    await untilFocused(() => again, "the card the dot brought back");
    // Tab walks its ways: the picture is the card with the keyboard on its ×.
    await userEvent.tab();
    await expect(within(again).getByRole("button", { name: "Dismiss" })).toHaveFocus();
  },
};

/** **Esc dismisses it** while the keyboard is in it, the keyboard goes back to Notices, and no
 *  dot is left. */
export const DismissedByEsc: Story = {
  play: async ({ canvas, userEvent }) => {
    const card = await waitFor(() => cardOf() as HTMLElement);
    card.focus();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(cardOf()).toBeNull());
    await expect(canvas.queryByRole("button", { name: /^Show the notice/ })).toBeNull();
    await untilFocused(() => canvas.getByTestId("status-alerts"), "Notices, once Esc dismissed");
  },
};

/** **The dot**, while the card is hidden: ● over the Notices item's leading corner, its count
 *  still read beside it, inside the bar. */
export const DotOnNotices: Story = {
  args: { startOpen: false },
  play: async ({ canvas }) => {
    const dot = canvas.getByRole("button", { name: "Show the notice on the status bar" });
    await expect(dot.querySelector("svg")).not.toBeNull();
    const [d, i, b] = [
      dot,
      canvas.getByTestId("status-alerts"),
      canvas.getByTestId("status-line"),
    ].map((one) => one.getBoundingClientRect());
    await expect(centre(d)).toBeLessThan(i.left + 8);
    await expect(d.top).toBeLessThan(i.top + 4);
    // Whole inside the bar, which clips what leaves it.
    await expect(d.top).toBeGreaterThanOrEqual(b.top);
    await expect(d.bottom).toBeLessThanOrEqual(b.bottom);
  },
};
