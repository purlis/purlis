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
import { pictureNow, untilFocused } from "./stories.testkit";
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
    // The picture is the card come back with the keyboard in it, taken before Tab: the ×'s
    // tooltip that Tab brings up is placed a pixel apart from one run to the next (#3653's
    // review, round 2).
    await pictureNow();
    // Tab walks its ways.
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

/** The dot's drawn disc: its box inside the clear border its 24px target keeps around it. */
function discOf(dot: HTMLElement): DOMRect {
  const box = dot.getBoundingClientRect();
  const drawn = getComputedStyle(dot);
  const left = box.left + parseFloat(drawn.borderLeftWidth);
  const top = box.top + parseFloat(drawn.borderTopWidth);
  const right = box.right - parseFloat(drawn.borderRightWidth);
  const bottom = box.bottom - parseFloat(drawn.borderBottomWidth);
  return new DOMRect(left, top, right - left, bottom - top);
}

/** **The dot**, while the card is hidden: ● at the bell's top leading corner, the bell still
 *  read beside it (#5889), its count read beside that, whole inside the bar at its height
 *  (#1922), and a 24px target (WCAG 2.5.8). */
export const DotOnNotices: Story = {
  args: { startOpen: false },
  play: async ({ canvas }) => {
    const dot = canvas.getByRole("button", { name: /^Show the notice on the status bar/ });
    await expect(dot.querySelector("svg")).not.toBeNull();
    const item = canvas.getByTestId("status-alerts");
    const bar = canvas.getByTestId("status-line");
    const icon = item.querySelector('[data-part="icon"] svg');
    await expect(icon).not.toBeNull();
    const slot = dot.parentElement as HTMLElement;
    const [d, s, bell] = [dot, slot, icon as Element].map((one) => one.getBoundingClientRect());
    const disc = discOf(dot);
    await expect(Math.round(d.width)).toBeGreaterThanOrEqual(24);
    await expect(Math.round(d.height)).toBeGreaterThanOrEqual(24);
    // A 16px disc, as round as it is wide.
    await expect(Math.round(disc.width)).toBe(16);
    await expect(Math.round(disc.height)).toBe(16);
    await expect(getComputedStyle(dot).backgroundClip).toBe("padding-box");
    // At the bell's top leading corner: its middle before the bell and above the bell's own,
    // reaching into the bell, but no more than a quarter of the way, so the bell is read.
    await expect(centre(disc)).toBeLessThan(bell.left);
    await expect(disc.top + disc.height / 2).toBeLessThan(bell.top + bell.height / 2);
    await expect(disc.right).toBeGreaterThan(bell.left);
    await expect(disc.right).toBeLessThanOrEqual(bell.left + bell.width / 4 + 0.5);
    // Its clear border over nothing before its slot.
    await expect(d.left).toBeGreaterThanOrEqual(s.left);
    // Whole inside the bar's clip (inside its top border), the disc and its target.
    const b = bar.getBoundingClientRect();
    await expect(d.top).toBeGreaterThanOrEqual(b.top + bar.clientTop);
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
