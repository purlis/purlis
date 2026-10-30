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
import { useFocusMode } from "./keys/focusMode";
import { Button } from "./ui/button/Button";

const REFUSED = "purlis pins at most 12 workspaces in one project. Unpin one first.";

function TheWindow({
  startOpen = true,
  results = 1,
  tall = 320,
}: {
  startOpen?: boolean;
  /** How many results are kept, the newest the refused pin. */
  results?: number;
  /** How tall the page over the bar is: the room the card has above it. */
  tall?: number;
}) {
  const [open, setOpen] = useState<string | undefined>(startOpen ? AT_STATUS : undefined);
  const [kept, setKept] = useState<readonly ChipNotice[]>(() => [
    {
      id: results,
      kind: "warning",
      said: { from: "workspace.pin", refused: true, words: REFUSED },
    },
    ...Array.from({ length: results - 1 }, (_, at): ChipNotice => ({
      id: results - 1 - at,
      kind: "info",
      said: { from: "workspace.layout", refused: false, words: `Layout ${at + 1} was read again.` },
    })),
  ]);
  // Focus mode, as the window's key turns it on: the bar goes, and its card with it.
  const focus = useFocusMode();
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
          blockSize: tall,
          background: "var(--surface-base)",
        }}
      >
        <div className="region-centre" style={{ padding: "var(--space-4)" }}>
          <Button>The work</Button>{" "}
          <Button data-testid="focus-mode" onPress={focus.toggle}>
            Focus mode
          </Button>
        </div>
        {/* Somewhere to press that no card is over, whatever a theme's metrics: the page's
            top end corner. */}
        <span
          data-testid="elsewhere"
          style={{ alignSelf: "flex-end", inlineSize: 40, blockSize: 40, order: -1 }}
        />
      </div>
      {!focus.on && (
        <StatusLine
          plane="/p/purlis-demo"
          read
          where="3rd-try"
          workspaces={4}
          state={{ pieces: {}, piecesRefused: {}, reading: false }}
          alerts={{ count: 2, open: () => undefined }}
          notices={notices}
        />
      )}
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
    const dot = canvas.getByRole("button", { name: /^Show the notice on the status bar/ });
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
  // It ends with the card gone and the bar as `DotOnNotices` draws it without its dot: nothing
  // to picture that another story does not.
  parameters: { screenshot: false },
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
    const dot = canvas.getByRole("button", { name: /^Show the notice on the status bar/ });
    await expect(dot.querySelector("svg")).not.toBeNull();
    const [d, i, b] = [
      dot,
      canvas.getByTestId("status-alerts"),
      canvas.getByTestId("status-line"),
    ].map((one) => one.getBoundingClientRect());
    await expect(centre(d)).toBeLessThan(i.left + 12);
    await expect(d.top).toBeLessThan(i.top + 4);
    // Whole inside the bar, which clips what leaves it.
    await expect(d.top).toBeGreaterThanOrEqual(b.top);
    await expect(d.bottom).toBeLessThanOrEqual(b.bottom);
  },
};

/**
 * **Many results in a short window** (#3653's review, D-3653-9): the card keeps its side above
 * Notices, never flipped below the window, held to the room there, and its notices scroll inside
 * it while its point stays on the item.
 */
export const ManyInAShortWindow: Story = {
  args: { results: 8, tall: 160 },
  play: async ({ canvas }) => {
    const card = await waitFor(() => cardOf() as HTMLElement);
    const surface = surfaceOf(card);
    await waitFor(() => expect(surface).toHaveAttribute("data-placement", "top"));
    const item = canvas.getByTestId("status-alerts").getBoundingClientRect();
    const box = surface.getBoundingClientRect();
    await expect(box.bottom).toBeLessThan(item.top);
    await expect(box.top).toBeGreaterThanOrEqual(0);
    // Held to the room, its notices scroll inside it.
    await expect(card.scrollHeight).toBeGreaterThan(card.clientHeight);
    await expect(document.documentElement.scrollHeight).toBeLessThanOrEqual(window.innerHeight);
  },
};

/**
 * **Focus mode with the keyboard in the card** (#3653's review, WCAG 2.4.3): the bar goes, and
 * the card with it; the keyboard goes to the work, never left on nothing. In a real browser the
 * hidden Notices item takes no keyboard, which jsdom would let it.
 */
export const FocusModeKeepsTheKeyboard: Story = {
  // It ends with the bar and the card gone: what it holds is where the keyboard is, no picture.
  parameters: { screenshot: false },
  play: async ({ canvas }) => {
    const card = await waitFor(() => cardOf() as HTMLElement);
    card.focus();
    await untilFocused(() => card, "the card, as F6 lands there");
    // The key's press, which moves no keyboard of its own.
    canvas.getByTestId("focus-mode").click();
    await waitFor(() => expect(cardOf()).toBeNull());
    await untilFocused(
      () => canvas.getByRole("button", { name: "The work" }),
      "the work, once focus mode took the bar",
    );
  },
};
