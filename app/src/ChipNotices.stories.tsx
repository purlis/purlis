// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A workspace's notices, hung from its chip** (#3648, spec #3647, the accepted prototype
 * `prototype/chip-notices`): the ready notice of a workspace just made, as a card centred under
 * its chip with its point, over the window, pushing nothing; hidden by a press elsewhere, it
 * leaves a ● dot on the chip's corner opposite the amber waiting count, which brings it back; ×
 * and Esc dismiss it. At the window's edge the card shifts inward and its point stays on the
 * chip. Each held to axe, walked by the keyboard and compared with its baseline
 * (`docs/component-stories.md`). The card draws outside its frame, so the pictures are the page's.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useMemo, useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import { OUTSIDE } from "./actions";
import { workspaceMade } from "./actionResult";
import type { ChipNotice, ChipNotices } from "./ChipNotices";
import { TitleBarSwitch, type SwitchProject } from "./TitleBarSwitch";

/** What `purlis workspace create` says of a workspace made with no vision. */
const LINES = [
  "✓ Workspace '3rd-try' ready (LOCAL) → workspaces/3rd-try/",
  '⬢ No vision yet — ask the developer what this workspace is for, then record it: purlis workspace vision "<the goal>"',
];

function TheWindow({
  pins,
  hasNotice,
  width,
  startOpen = true,
}: {
  pins: string[];
  hasNotice: string;
  /** The title bar's width: the window's. */
  width: string;
  startOpen?: boolean;
}) {
  const [said, setSaid] = useState("Nothing pressed");
  const [open, setOpen] = useState<string | undefined>(startOpen ? hasNotice : undefined);
  const [kept, setKept] = useState<readonly ChipNotice[]>(() => [
    {
      id: 1,
      kind: "info",
      said: workspaceMade(hasNotice, LINES, { onAddVision: () => setSaid("Asked for a vision") }),
    },
  ]);
  const projects = useMemo<SwitchProject[]>(
    () => [
      {
        plane: "/p/purlis-demo",
        name: "purlis-demo",
        waiting: 2,
        workspaces: {
          names: pins,
          pins,
          focused: hasNotice,
          waiting: { [OUTSIDE]: 0, [hasNotice]: 2 },
        },
      },
    ],
    [pins, hasNotice],
  );
  const notices: ChipNotices = {
    on: new Map(kept.length === 0 ? [] : [[hasNotice, kept]]),
    open: kept.length === 0 ? undefined : open,
    show: setOpen,
    hide: () => setOpen(undefined),
    dismiss: (id) => setKept((was) => was.filter((one) => one.id !== id)),
    onSettings: () => undefined,
  };
  return (
    <div style={{ display: "grid", gap: "var(--space-6)", inlineSize: width }}>
      <header className="title-bar" data-testid="title-bar">
        <span className="title-bar-leading">
          <TitleBarSwitch
            projects={projects}
            inFront="/p/purlis-demo"
            railShown
            does={{
              go: (_, workspace) => setSaid(`Went to ${workspace}`),
              choose: () => undefined,
              pin: () => undefined,
              create: () => undefined,
            }}
            notices={notices}
          />
        </span>
      </header>
      {/* What the window holds under the bar, which a card must never move. */}
      <div data-testid="page" style={{ blockSize: 240, background: "var(--surface-base)" }}>
        <p role="status" style={{ margin: 0, padding: "var(--space-4)" }}>
          {said}
        </p>
      </div>
    </div>
  );
}

const meta = {
  component: TheWindow,
  parameters: { screenshot: "page" },
  args: {
    pins: ["skills-studio", "another-workspace", "second-try", "3rd-try"],
    hasNotice: "3rd-try",
    width: "60rem",
  },
} satisfies Meta<typeof TheWindow>;

export default meta;
type Story = StoryObj<typeof meta>;

const chipOf = (name: string) =>
  [
    ...screen
      .getByTestId("title-bar")
      .querySelectorAll<HTMLElement>('[data-scope="titlebar"][data-part="workspace"]'),
  ].find((one) => one.dataset.chipOf === name) as HTMLElement;
const centre = (box: DOMRect) => box.left + box.width / 2;
const cardOf = () => screen.queryByRole("region", { name: "Notices for 3rd-try" });
const surfaceOf = (card: HTMLElement) =>
  card.closest<HTMLElement>('[data-scope="popover"][data-part="root"]') as HTMLElement;

/**
 * **The ready notice, hung from its chip**: centred under it with its point in the middle, over
 * the window, taking no keyboard. A press elsewhere hides it and moves nothing; the dot it
 * leaves brings it back; × dismisses it, and Esc while the keyboard is in it.
 */
export const HangsFromItsChip: Story = {
  play: async ({ canvas, userEvent }) => {
    const card = await waitFor(() => cardOf() as HTMLElement);
    await expect(card).toHaveTextContent("Workspace 3rd-try is ready");
    await expect(within(card).queryByRole("button", { name: /^Open/ })).toBeNull();
    // It takes no keyboard as it comes.
    await expect(card.contains(document.activeElement)).toBe(false);
    // Centred under its chip, its point on the chip's middle.
    const chip = chipOf("3rd-try").getBoundingClientRect();
    const surface = surfaceOf(card).getBoundingClientRect();
    await expect(Math.abs(centre(surface) - centre(chip))).toBeLessThanOrEqual(1);
    await expect(surface.top).toBeGreaterThan(chip.bottom);
    const point = surfaceOf(card).querySelector('[data-part="arrow"]') as HTMLElement;
    await expect(
      Math.abs(centre(point.getBoundingClientRect()) - centre(chip)),
    ).toBeLessThanOrEqual(1);
    // It pushes nothing: the page under the bar is where it is with the card hidden.
    const page = canvas.getByTestId("page").getBoundingClientRect();
    await userEvent.click(canvas.getByTestId("page"));
    await waitFor(() => expect(cardOf()).toBeNull());
    await expect(canvas.getByTestId("page").getBoundingClientRect().toJSON()).toEqual(
      page.toJSON(),
    );
    // The dot it leaves, beside the amber count, brings it back.
    const dot = canvas.getByRole("button", { name: "Show the notice on 3rd-try" });
    await expect(dot.querySelector("svg")).not.toBeNull();
    await userEvent.click(dot);
    const again = await waitFor(() => cardOf() as HTMLElement);
    // The keyboard reaches it by a press: its ways, then Esc dismisses it.
    await userEvent.click(again);
    await userEvent.tab();
    await expect(again.contains(document.activeElement)).toBe(true);
    await userEvent.click(within(again).getByRole("button", { name: "Details" }));
    await expect(again).toHaveTextContent("ready (LOCAL)");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(cardOf()).toBeNull());
    await expect(canvas.queryByRole("button", { name: /^Show the notice/ })).toBeNull();
  },
};

/** **The dot**, while the card is hidden: ● on the chip's leading corner, the amber waiting
 *  count at its end, both read at once. */
export const DotBesideTheCount: Story = {
  args: { startOpen: false },
  play: async ({ canvas }) => {
    const dot = canvas.getByRole("button", { name: "Show the notice on 3rd-try" });
    const chip = chipOf("3rd-try");
    const count = chip.querySelector('[data-part="count"]') as HTMLElement;
    const [d, c, k] = [dot, chip, count].map((one) => one.getBoundingClientRect());
    // The dot over the chip's leading corner, the count at its trailing end: apart.
    await expect(centre(d)).toBeLessThan(c.left + 8);
    await expect(d.top).toBeLessThan(c.top + 4);
    await expect(d.right).toBeLessThan(k.left);
    await expect(chip).toHaveAccessibleDescription("2 waiting, 1 notice");
  },
};

/** **At the window's edge**: the card shifts inward, never cut off, and its point stays on the
 *  chip it hangs from. */
export const AtTheWindowEdge: Story = {
  args: { pins: ["3rd-try", "skills-studio"], width: "40rem" },
  play: async () => {
    const card = await waitFor(() => cardOf() as HTMLElement);
    const surface = surfaceOf(card).getBoundingClientRect();
    const chip = chipOf("3rd-try").getBoundingClientRect();
    await expect(surface.left).toBeGreaterThanOrEqual(8);
    // Shifted: no longer centred on the chip, which is too near the edge for that.
    await expect(centre(surface)).toBeGreaterThan(centre(chip) + 8);
    const point = surfaceOf(card).querySelector('[data-part="arrow"]') as HTMLElement;
    await expect(
      Math.abs(centre(point.getBoundingClientRect()) - centre(chip)),
    ).toBeLessThanOrEqual(1);
  },
};
