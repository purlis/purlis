// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The right side's bar as ADR 0121 has it** (#6020): each attention view's tooltip says its
 * question, each icon's count is in its name, and the ✋ bounces, the one movement in the window
 * that calls the person (#6343). Drawn in every cell of the theme matrix.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useId, useState } from "react";
import { expect, fn, waitFor, within } from "storybook/test";
import { ActivityBar, ActivityCount, ActivityDot, VIEW_MARKS, type Tabbed } from "./ActivityBar";
import type { ViewId } from "./regions";
import { expectFocusInSight, untilFocused } from "./stories.testkit";
import { Button } from "./ui/button/Button";
import { drawPerson, personInForce } from "./theme/theme";

/** `value`, which the story has drawn: it fails here, by name, where it is not. */
function drawn<T>(value: T | null | undefined, what: string): T {
  if (value === null || value === undefined) throw new Error(`${what} is not drawn`);
  return value;
}

const VIEWS: Tabbed[] = [
  { view: "asks", name: "Asks", mark: VIEW_MARKS.asks },
  { view: "updates", name: "Updates", mark: VIEW_MARKS.updates },
  { view: "notices", name: "Notices", mark: VIEW_MARKS.notices },
];

function Attention({
  bounce,
  onBounced,
  hand = [2],
  name = "Attention",
}: {
  bounce?: ViewId;
  onBounced?: () => void;
  /** The ✋'s counts, one per press of "Next count" (#6342). */
  hand?: number[];
  name?: string;
}) {
  const [open, setOpen] = useState<ViewId | undefined>("updates");
  const [step, setStep] = useState(0);
  const asked = hand[step % hand.length];
  // Its own ids: a story that draws several bars (Hand Counts) names each tab from its own badge,
  // where one id shared would read the first bar's count in every tab's name.
  const id = useId();
  return (
    <div style={{ display: "flex", blockSize: 200 }}>
      <ActivityBar
        side="right"
        variant="bar"
        name={name}
        views={VIEWS}
        open={open}
        current={open}
        badges={{
          asks: (
            <ActivityCount count={asked} said={`${asked} things wait on you`} tone="needs-you" />
          ),
          updates: <ActivityCount count={1} said="1 unread update from your chats" />,
          notices: <ActivityDot said="A Notice needs fixing" />,
        }}
        bounce={bounce}
        onBounced={onBounced}
        panelOf={(view) => `${id}-panel-${view}`}
        tabOf={(view) => `${id}-tab-${view}`}
        onPick={(view) => setOpen((was) => (was === view ? undefined : view))}
      />
      {VIEWS.map(({ view, name }) => (
        <div
          key={view}
          id={`${id}-panel-${view}`}
          role="tabpanel"
          aria-labelledby={`${id}-tab-${view}`}
          hidden={view !== open}
        >
          {name}
        </div>
      ))}
      {hand.length > 1 && (
        <Button variant="secondary" onPress={() => setStep((was) => was + 1)}>
          Next count
        </Button>
      )}
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

/** The ✋'s round count (#6342), or `null` where none is drawn. */
const handCount = (hand: HTMLElement) =>
  hand.querySelector<HTMLElement>('[data-scope="count"][data-part="round"]');

/**
 * **The ✋'s count is one small circle in its colour** (#6342): 0 draws none, 1 and 9 as they are,
 * 10 and 120 as 9+, every one 16px with its centre on the icon's top right corner, and the ✋
 * says the real number.
 */
export const HandCounts: Story = {
  render: () => (
    <div style={{ display: "flex", gap: "var(--space-6)" }}>
      {[0, 1, 9, 10, 120].map((count) => (
        <Attention key={count} hand={[count]} name={`Attention ${count}`} />
      ))}
    </div>
  ),
  play: async ({ canvas, userEvent }) => {
    const hands = canvas.getAllByRole("tab", { name: /^Asks/ });
    await expect(handCount(hands[0])).toBeNull();
    await expect(hands.slice(1).map((hand) => handCount(hand)?.dataset.drawn)).toEqual([
      "1",
      "9",
      "9+",
      "9+",
    ]);
    for (const hand of hands.slice(1)) {
      const circle = drawn(handCount(hand), "the ✋'s count").getBoundingClientRect();
      const icon = drawn(hand.querySelector("svg"), "the ✋'s icon").getBoundingClientRect();
      await expect([Math.round(circle.width), Math.round(circle.height)]).toEqual([16, 16]);
      await expect(circle.left + circle.width / 2).toBeCloseTo(icon.right, 0);
      await expect(circle.top + circle.height / 2).toBeCloseTo(icon.top, 0);
    }
    await expect(canvas.getByRole("tab", { name: "Asks 120 things wait on you" })).toBe(hands[4]);
    await userEvent.tab();
    await untilFocused(() => canvas.getAllByRole("tab", { name: /^Updates/ })[0], "Tab in");
    await userEvent.keyboard("{ArrowUp}");
    await untilFocused(() => hands[0], "↑ to the ✋");
    expectFocusInSight();
    await userEvent.keyboard("{Escape}");
  },
};

/**
 * **The ✋'s changed count slides once** (#6342): 2 to 3 slides up, 3 to 12 slides again (its
 * other keyframes), 12 to 40 is 9+ both times and does not; the count is not drawn again.
 */
export const HandCountSlides: Story = {
  args: { hand: [2, 3, 12, 40, 1] },
  parameters: { screenshot: false },
  play: async ({ canvas, userEvent }) => {
    const hand = canvas.getByRole("tab", { name: /^Asks/ });
    const circle = drawn(handCount(hand), "the ✋'s count");
    const slide = () => getComputedStyle(circle, "::before").animationName;
    await expect(slide()).toBe("none");
    canvas.getByRole("button", { name: "Next count" }).focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(circle).toHaveAttribute("data-drawn", "3"));
    await expect(slide()).toBe("count-round-up-a");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(circle).toHaveAttribute("data-drawn", "9+"));
    await expect(slide()).toBe("count-round-up-b");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(hand).toHaveAccessibleName("Asks 40 things wait on you"));
    await expect(slide()).toBe("count-round-up-b");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(circle).toHaveAttribute("data-drawn", "1"));
    await expect(slide()).toBe("count-round-down-a");
    await expect(handCount(hand)).toBe(circle);
  },
};

/** **Under reduced motion the ✋'s count just changes** (#6342): the slide ends before a frame. */
export const HandCountUnderReducedMotion: Story = {
  args: { hand: [2, 3] },
  parameters: { screenshot: false },
  beforeEach: () => {
    const was = personInForce();
    drawPerson({ ...was, motion: "reduced" });
    return () => drawPerson(was);
  },
  play: async ({ canvas, userEvent }) => {
    const hand = canvas.getByRole("tab", { name: /^Asks/ });
    canvas.getByRole("button", { name: "Next count" }).focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(handCount(hand)).toHaveAttribute("data-drawn", "3"));
    const style = getComputedStyle(drawn(handCount(hand), "the ✋'s count"), "::before");
    await expect(Number.parseFloat(style.animationDuration)).toBe(0);
  },
};
