// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The right side's bar as ADR 0121 has it** (#6020): each attention view's tooltip says its
 * question, each icon's count is in its name, and the ✋ bounces, the one movement in the window
 * that calls the person (#6343). Drawn in every cell of the theme matrix.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, waitFor, within } from "storybook/test";
import { ActivityBar, ActivityCount, ActivityDot, VIEW_MARKS, type Tabbed } from "./ActivityBar";
import type { ViewId } from "./regions";
import { expectFocusInSight, untilFocused } from "./stories.testkit";
import { drawPerson, personInForce } from "./theme/theme";

const VIEWS: Tabbed[] = [
  { view: "asks", name: "Asks", mark: VIEW_MARKS.asks },
  { view: "updates", name: "Updates", mark: VIEW_MARKS.updates },
  { view: "notices", name: "Notices", mark: VIEW_MARKS.notices },
];

function Attention({ bounce, onBounced }: { bounce?: ViewId; onBounced?: () => void }) {
  const [open, setOpen] = useState<ViewId | undefined>("updates");
  return (
    <div style={{ display: "flex", blockSize: 200 }}>
      <ActivityBar
        side="right"
        variant="bar"
        name="Attention"
        views={VIEWS}
        open={open}
        current={open}
        badges={{
          asks: <ActivityCount count={2} said="2 things wait on you" tone="needs-you" />,
          updates: <ActivityCount count={1} said="1 unread update from your chats" />,
          notices: <ActivityDot said="A Notice needs fixing" />,
        }}
        bounce={bounce}
        onBounced={onBounced}
        panelOf={(view) => `story-panel-${view}`}
        tabOf={(view) => `story-tab-${view}`}
        onPick={(view) => setOpen((was) => (was === view ? undefined : view))}
      />
      {VIEWS.map(({ view, name }) => (
        <div
          key={view}
          id={`story-panel-${view}`}
          role="tabpanel"
          aria-labelledby={`story-tab-${view}`}
          hidden={view !== open}
        >
          {name}
        </div>
      ))}
    </div>
  );
}

const meta = { component: Attention } satisfies Meta<typeof Attention>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Each icon names its count, and its tooltip asks its question**: walked by the arrows. */
export const Questions: Story = {
  parameters: { screenshot: "page" },
  play: async ({ canvas, userEvent }) => {
    const page = within(document.body);
    const asks = canvas.getByRole("tab", { name: "Asks 2 things wait on you" });
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("tab", { name: "Updates 1 unread update from your chats" }),
      "Tab to the open view's icon",
    );
    expectFocusInSight();
    await waitFor(() => expect(page.getByRole("tooltip")).toHaveTextContent("Did my work finish?"));
    await userEvent.keyboard("{ArrowUp}");
    await untilFocused(() => asks, "↑ to the ✋");
    expectFocusInSight();
    await waitFor(() =>
      expect(page.getByRole("tooltip")).toHaveTextContent("Do I need to answer something?"),
    );
    await userEvent.keyboard("{End}");
    await untilFocused(
      () => canvas.getByRole("tab", { name: "Notices A Notice needs fixing" }),
      "End to Notices",
    );
    expectFocusInSight();
    await waitFor(() =>
      expect(page.getByRole("tooltip")).toHaveTextContent(
        "Is something about my setup wrong or worth knowing?",
      ),
    );
  },
};

/** The style of the mark on the tab `hand`, whose movement is the bounce. */
const markStyle = (hand: HTMLElement) =>
  getComputedStyle(hand.querySelector('[data-part="icon"]') ?? hand);

/**
 * **The ✋'s bounce** (#6343): its mark hops 3 times, nothing beside it moves, and the window is
 * told as it ends. Not pictured: a bounce in flight has no frame that holds still.
 */
export const Bounce: Story = {
  args: { bounce: "asks", onBounced: fn() },
  parameters: { screenshot: false },
  play: async ({ canvas, args }) => {
    await expect(canvas.getByRole("tab", { name: /^Asks/ })).toHaveAttribute("data-bounce", "true");
    await expect(canvas.getByRole("tab", { name: /^Updates/ })).not.toHaveAttribute("data-bounce");
    const style = markStyle(canvas.getByRole("tab", { name: /^Asks/ }));
    await expect(style.animationName).toBe("purlis-hand-bounce");
    await expect(style.animationIterationCount).toBe("3");
    await waitFor(() => expect(args.onBounced).toHaveBeenCalledOnce(), { timeout: 5000 });
  },
};

/**
 * **Under reduced motion the ✋ does not move**: the number is all that changes. The window holds
 * the bounce itself (`handBounce.ts`), and should one be asked for, the theme's one switch ends
 * it before a frame is drawn, and the window still hears it end.
 *
 * Motion is turned off the way Settings turns it off, in the person's layer: the preview draws
 * the theme again as the story renders, and that drawing writes the root's `data-motion` from
 * the person's layer and the system, so an attribute set on the root by hand is gone by then.
 */
export const BounceUnderReducedMotion: Story = {
  args: { bounce: "asks", onBounced: fn() },
  parameters: { screenshot: false },
  beforeEach: () => {
    const was = personInForce();
    drawPerson({ ...was, motion: "reduced" });
    return () => drawPerson(was);
  },
  play: async ({ canvas, args }) => {
    const style = markStyle(canvas.getByRole("tab", { name: /^Asks/ }));
    await expect(Number.parseFloat(style.animationDuration)).toBe(0);
    await expect(style.animationIterationCount).toBe("1");
    await waitFor(() => expect(args.onBounced).toHaveBeenCalledOnce());
  },
};
