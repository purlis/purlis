// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A workspace's notices, hung from its chip** (#3648, spec #3647, the accepted prototype
 * `prototype/chip-notices`): the ready notice of a workspace just made, as a card centred under
 * its chip with its point, over the window, pushing nothing; hidden by a press elsewhere, it
 * leaves a ● dot on the chip's corner opposite the amber waiting count, which brings it back; ×
 * and Esc dismiss it. At the window's edge the card shifts inward and its point stays on the
 * chip. Several notices stack in the one card, newest first, and the dot takes the most urgent
 * kind, ▲ for a warning (#3649). Each held to axe, walked by the keyboard and compared with its baseline
 * (`docs/component-stories.md`). The card draws outside its frame, so the pictures are the page's.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useMemo, useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import { OUTSIDE } from "./actions";
import { reposCloning, reposNotCloned, workspaceMade } from "./actionResult";
import type { Said } from "./actions";
import {
  dormantCause,
  dormantSays,
  type ChipNotice,
  type ChipNotices,
  type StandingNotice,
} from "./ChipNotices";
import { untilFocused } from "./stories.testkit";
import { TitleBarSwitch, type SwitchProject } from "./TitleBarSwitch";

/** What `purlis workspace create` says of a workspace made with no vision. */
const LINES = [
  "✓ Workspace '3rd-try' ready (LOCAL) → workspaces/3rd-try/",
  '⬢ No vision yet — ask the developer what this workspace is for, then record it: purlis workspace vision "<the goal>"',
];

/** A stale clone's warning, as #3650 brings them to a workspace's chip. */
const OFF_MAIN = (workspace: string): Said => ({
  from: "story",
  refused: true,
  title: "web is on spike/search, not main",
  words: "New chats here start from spike/search.",
  workspace,
});

/** **More than one notice on the chip** (#3649), newest first: `stacked`, the clone counting
 *  over a warning over the ready notice; `failed`, a clone that did not land over the ready
 *  notice; `findings`, the ready notice over the project's findings about the workspace, a repo
 *  off its default branch and a layout behind (#3650). */
type Stack = "ready" | "stacked" | "failed" | "findings";

function TheWindow({
  pins,
  hasNotice,
  width,
  startOpen = true,
  dormant = false,
  stack = "ready",
}: {
  pins: string[];
  hasNotice: string;
  /** `hasNotice` is a pin whose workspace is gone, kept dormant (#3651): its notice is the
   *  standing "kept dormant" line, and the first pin is in front. */
  dormant?: boolean;
  /** The title bar's width: the window's. */
  width: string;
  startOpen?: boolean;
  stack?: Stack;
}) {
  const [said, setSaid] = useState("Nothing pressed");
  const [open, setOpen] = useState<string | undefined>(startOpen ? hasNotice : undefined);
  /** The standing line was dismissed: the per-project record's, not the kept list's. */
  const [putAway, setPutAway] = useState(false);
  const [kept, setKept] = useState<readonly ChipNotice[]>(() => [
    ...(dormant
      ? [
          {
            id: dormantCause(hasNotice),
            kind: "info" as const,
            standing: {
              workspace: hasNotice,
              cause: dormantCause(hasNotice),
              says: dormantSays(hasNotice),
              fixes: [
                { label: "Forget", onPress: () => setSaid(`Forgot the pin to ${hasNotice}`) },
              ],
              dismiss: () => setPutAway(true),
            },
          },
        ]
      : [
          ...(stack === "stacked"
            ? [
                {
                  id: 3,
                  kind: "info" as const,
                  said: reposCloning(hasNotice, ["api", "web"], 1, 1),
                },
                { id: 2, kind: "warning" as const, said: OFF_MAIN(hasNotice) },
              ]
            : stack === "failed"
              ? [
                  {
                    id: 2,
                    kind: "warning" as const,
                    said: reposNotCloned(
                      hasNotice,
                      [{ repo: "web", said: "web: clone failed — no access to acme/web." }],
                      () => setSaid("Asked to clone web again"),
                      1,
                    ),
                  },
                ]
              : []),
          {
            id: 1,
            kind: "info" as const,
            said:
              hasNotice === OUTSIDE
                ? {
                    from: "story",
                    refused: false,
                    title: "The root's layout is up to date",
                    words: "Its layout was read again.",
                    workspace: OUTSIDE,
                  }
                : workspaceMade(hasNotice, LINES, {
                    onAddVision: () => setSaid("Asked for a vision"),
                  }),
          },
          // A standing line follows the results, as the window appends them (#3650).
          ...(stack === "findings"
            ? [
                {
                  id: `stale-clone:${hasNotice}/web|branch:spike/search`,
                  kind: "warning" as const,
                  standing: {
                    workspace: hasNotice,
                    cause: `stale-clone:${hasNotice}/web|branch:spike/search`,
                    says: (
                      <p>
                        <code>web</code> in {hasNotice} is on <code>spike/search</code>, not{" "}
                        <code>main</code>. A chat that starts in this repo works on{" "}
                        <code>spike/search</code>.
                      </p>
                    ),
                    fixes: [
                      { label: "Switch to main", onPress: () => setSaid("Asked to switch web") },
                    ],
                    kind: "warning" as const,
                    label: `web in ${hasNotice}`,
                    dismiss: () => setPutAway(true),
                  } satisfies StandingNotice,
                },
                {
                  id: `alert:reinit:${hasNotice}`,
                  kind: "warning" as const,
                  standing: {
                    workspace: hasNotice,
                    cause: `alert:reinit:${hasNotice}`,
                    says: `${hasNotice} is behind the current layout. Update workspace layout brings it up to date.`,
                    fixes: [
                      {
                        label: "Update workspace layout",
                        onPress: () => setSaid("Asked to update the layout"),
                      },
                    ],
                    kind: "warning" as const,
                  } satisfies StandingNotice,
                },
              ]
            : []),
        ]),
  ]);
  const projects = useMemo<SwitchProject[]>(
    () => [
      {
        plane: "/p/purlis-demo",
        name: "purlis-demo",
        waiting: 2,
        workspaces: dormant
          ? {
              names: pins,
              pins,
              dormant: [hasNotice],
              focused: pins[0],
              waiting: { [OUTSIDE]: 0, [pins[0]]: 2 },
            }
          : {
              names: pins,
              pins,
              focused: hasNotice,
              waiting: { [OUTSIDE]: 0, [hasNotice]: 2 },
            },
      },
    ],
    [pins, hasNotice, dormant],
  );
  const standing = putAway ? kept.filter((one) => !("standing" in one)) : kept;
  const notices: ChipNotices = {
    on: new Map(standing.length === 0 ? [] : [[hasNotice, standing]]),
    open: standing.length === 0 ? undefined : open,
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
      <div
        data-testid="page"
        style={{
          display: "flex",
          flexDirection: "column",
          justifyContent: "space-between",
          blockSize: 360,
          background: "var(--surface-base)",
        }}
      >
        <p role="status" style={{ margin: 0, padding: "var(--space-4)" }}>
          {said}
        </p>
        {/* Somewhere to press that no card is over, whatever a theme's metrics: the page's
            bottom start corner. */}
        <span data-testid="elsewhere" style={{ inlineSize: 40, blockSize: 40 }} />
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
// Centred within a pixel and a half: layout rounds a subpixel box to the device's pixels.
const cardOf = () => screen.queryByRole("region", { name: "Notices for 3rd-try" });
const surfaceOf = (card: HTMLElement) =>
  card.closest<HTMLElement>('[data-scope="popover"][data-part="root"]') as HTMLElement;

/**
 * **The ready notice, hung from its chip**: centred under it with its point in the middle, over
 * the window, taking no keyboard. A press elsewhere hides it and moves nothing; the dot it
 * leaves brings it back; the keyboard reaches it, and Details opens in place.
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
    await expect(Math.abs(centre(surface) - centre(chip))).toBeLessThanOrEqual(1.5);
    await expect(surface.top).toBeGreaterThan(chip.bottom);
    const point = surfaceOf(card).querySelector('[data-part="point"]') as HTMLElement;
    await expect(
      Math.abs(centre(point.getBoundingClientRect()) - centre(chip)),
    ).toBeLessThanOrEqual(1.5);
    // It pushes nothing: the page under the bar is where it is with the card hidden.
    const page = canvas.getByTestId("page").getBoundingClientRect();
    // A press elsewhere, well clear of the card in every theme's metrics.
    await userEvent.click(canvas.getByTestId("elsewhere"));
    await waitFor(() => expect(cardOf()).toBeNull());
    await expect(canvas.getByTestId("page").getBoundingClientRect().toJSON()).toEqual(
      page.toJSON(),
    );
    // The dot it leaves, beside the amber count, brings it back.
    const dot = canvas.getByRole("button", { name: "Show the notice on 3rd-try" });
    await expect(dot.querySelector("svg")).not.toBeNull();
    await userEvent.click(dot);
    const again = await waitFor(() => cardOf() as HTMLElement);
    // The keyboard reaches it as F6 lands it there, on the card: Tab walks its ways.
    again.focus();
    await userEvent.tab();
    await expect(again.contains(document.activeElement)).toBe(true);
    await expect(document.activeElement).not.toBe(again);
    // Details opens in place. The picture is the card, its Details open.
    await userEvent.click(within(again).getByRole("button", { name: "Details" }));
    await expect(again).toHaveTextContent("ready (LOCAL)");
  },
};

/** **Esc dismisses it** while the keyboard is in it, and no dot is left. */
export const DismissedByEsc: Story = {
  play: async ({ canvas, userEvent }) => {
    const card = await waitFor(() => cardOf() as HTMLElement);
    card.focus();
    // Past its × (whose tooltip an Esc would close first) to its first way on.
    await userEvent.tab();
    await userEvent.tab();
    await expect(within(card).getByRole("button", { name: "Add a vision" })).toHaveFocus();
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

/** **At the window's edge**: a notice about the plane root, whose house stands just after the
 *  project's name, too near the edge for a card centred under it. The card shifts inward and
 *  is never cut off, and its point stays on the house. */
export const AtTheWindowEdge: Story = {
  args: { hasNotice: OUTSIDE, width: "40rem" },
  play: async ({ canvas }) => {
    const card = await waitFor(
      () => screen.queryByRole("region", { name: "Notices for Plane root" }) as HTMLElement,
    );
    const surface = surfaceOf(card).getBoundingClientRect();
    const house = canvas.getByRole("button", { name: "Plane root" }).getBoundingClientRect();
    await expect(surface.left).toBeGreaterThanOrEqual(8);
    // Shifted: centred, it would start left of the window.
    await expect(centre(house) - surface.width / 2).toBeLessThan(8);
    await expect(centre(surface)).toBeGreaterThan(centre(house));
    const point = surfaceOf(card).querySelector('[data-part="point"]') as HTMLElement;
    await expect(
      Math.abs(centre(point.getBoundingClientRect()) - centre(house)),
    ).toBeLessThanOrEqual(1.5);
  },
};

/** Workspaces enough that the strip is too long to show whole (R103). */
const MANY = [
  "skills-studio",
  "another-workspace",
  "3rd-try",
  "release-notes",
  "billing-migration",
  "search-reindex",
  "onboarding-flow",
  "docs-site",
  "mobile-shell",
  "infra-terraform",
  "data-pipeline",
  "design-tokens",
];

/**
 * **A strip too long to show whole** (#3648's review): the card follows its chip as the strip
 * scrolls, centred on it, and goes down, dismissing nothing, once the chip has scrolled out of
 * the strip's sight. Its dot stays on the chip for when it comes back.
 */
export const InAStripTooLongToShow: Story = {
  args: { pins: MANY, hasNotice: "3rd-try" },
  play: async ({ canvas }) => {
    const strip = canvas.getByRole("toolbar", { name: "Pinned workspaces" });
    await expect(strip.scrollWidth).toBeGreaterThan(strip.clientWidth);
    const card = await waitFor(() => cardOf() as HTMLElement);
    // A little way along: still in sight, and followed.
    strip.scrollLeft = 40;
    await waitFor(() => {
      const chip = chipOf("3rd-try").getBoundingClientRect();
      const surface = surfaceOf(card).getBoundingClientRect();
      expect(Math.abs(centre(surface) - centre(chip))).toBeLessThanOrEqual(1.5);
    });
    // Its chip out of the strip's sight: down, and its notice is not dismissed.
    strip.scrollLeft = strip.scrollWidth;
    await waitFor(() => expect(cardOf()).toBeNull());
    await expect(
      canvas.getByRole("button", { name: "Show the notice on 3rd-try" }),
    ).toBeInTheDocument();
  },
};

/**
 * **A card that comes up on a chip out of sight** (#3648's round 3): a workspace made at the end
 * of a strip too long to show whole. The card brings its chip into the strip's sight, smoothly
 * with the window's own motion, and stays up while it scrolls in, then hangs centred from it.
 */
export const ComesUpOnAChipOutOfSight: Story = {
  args: { pins: MANY, hasNotice: "design-tokens" },
  play: async ({ canvas }) => {
    const strip = canvas.getByRole("toolbar", { name: "Pinned workspaces" });
    const card = await waitFor(
      () => screen.queryByRole("region", { name: "Notices for design-tokens" }) as HTMLElement,
    );
    await waitFor(() => {
      const chip = chipOf("design-tokens").getBoundingClientRect();
      const seen = strip.getBoundingClientRect();
      expect(chip.right).toBeLessThanOrEqual(seen.right + 1);
      const surface = surfaceOf(card).getBoundingClientRect();
      expect(Math.abs(centre(surface) - centre(chip))).toBeLessThanOrEqual(1.5);
    });
    // Up all the way in: the same card, never taken down by its own scroll.
    await expect(card.isConnected).toBe(true);
    // The picture once everything has come to rest: the strip still over two frames, with the
    // chip in its sight, and the card arrived.
    let was = -1;
    await waitFor(async () => {
      const now = strip.scrollLeft;
      await new Promise((frame) => requestAnimationFrame(frame));
      await new Promise((frame) => requestAnimationFrame(frame));
      const still = now === was && now === strip.scrollLeft;
      was = strip.scrollLeft;
      expect(still).toBe(true);
    });
    await waitFor(() => expect(surfaceOf(card).hasAttribute("data-entering")).toBe(false));
  },
};

/**
 * **A dormant pin's notice, on its dormant chip** (#3651): the pin whose workspace is gone is
 * drawn after the live pins, its words muted and slanted and its edge dotted, so its shape says
 * it. "Kept dormant" hangs from it with Forget; the chip says what it is to a screen reader.
 */
export const OnADormantChip: Story = {
  args: {
    pins: ["skills-studio", "another-workspace", "3rd-try"],
    hasNotice: "second-try",
    dormant: true,
  },
  play: async ({ canvas, userEvent }) => {
    const chip = chipOf("second-try");
    await expect(chip).toHaveAttribute("data-variant", "dormant");
    await expect(chip).toHaveAccessibleDescription("gone, kept dormant, 1 notice");
    // After the live pins, not among them.
    const live = chipOf("3rd-try").getBoundingClientRect();
    await expect(chip.getBoundingClientRect().left).toBeGreaterThan(live.right);
    const card = await waitFor(
      () => screen.queryByRole("region", { name: "Notices for second-try" }) as HTMLElement,
    );
    await expect(card).toHaveTextContent("second-try is gone, kept dormant");
    const surface = surfaceOf(card).getBoundingClientRect();
    await expect(
      Math.abs(centre(surface) - centre(chip.getBoundingClientRect())),
    ).toBeLessThanOrEqual(1.5);
    // Its Forget, reached by the keyboard as F6 lands it in the card.
    card.focus();
    const forget = within(card).getByRole("button", { name: "Forget" });
    forget.focus();
    await userEvent.keyboard("{Enter}");
    await expect(canvas.getByRole("status")).toHaveTextContent("Forgot the pin to second-try");
    // The picture once the card has arrived, never mid-entrance.
    await waitFor(() => expect(surfaceOf(card).hasAttribute("data-entering")).toBe(false));
  },
};

/** **A dormant chip whose card is hidden**: the dot on its corner, and a press on the chip
 *  itself brings the card back, since the chip opens nothing else. */
export const ADormantChipShowsItsNotice: Story = {
  args: {
    pins: ["skills-studio", "another-workspace", "3rd-try"],
    hasNotice: "second-try",
    dormant: true,
    startOpen: false,
  },
  play: async ({ canvas, userEvent }) => {
    await expect(
      canvas.getByRole("button", { name: "Show the notice on second-try" }),
    ).toBeInTheDocument();
    await expect(screen.queryByRole("region", { name: "Notices for second-try" })).toBeNull();
    // Pressed from the keyboard, as the strip's arrows reach it: no pointer is left over the
    // chip to light it, so the picture is the same on every run.
    chipOf("second-try").focus();
    await userEvent.keyboard("{Enter}");
    const card = await waitFor(
      () => screen.queryByRole("region", { name: "Notices for second-try" }) as HTMLElement,
    );
    // Asked for, so the keyboard goes into it.
    await untilFocused(() => card, "the dormant chip's card, brought by its press");
    // The picture once the card has arrived, never mid-entrance.
    await waitFor(() => expect(surfaceOf(card).hasAttribute("data-entering")).toBe(false));
  },
};

/**
 * **A notice about the current, unpinned workspace** (#3651): it hangs from the dashed
 * `name +` chip, centred under it with its point, as from a pin.
 */
export const OnTheDashedChip: Story = {
  args: { pins: ["skills-studio", "another-workspace"], hasNotice: "3rd-try" },
  play: async () => {
    const chip = chipOf("3rd-try");
    await expect(chip).toHaveAttribute("data-variant", "unpinned");
    const card = await waitFor(() => cardOf() as HTMLElement);
    await expect(card).toHaveTextContent("Workspace 3rd-try is ready");
    const surface = surfaceOf(card).getBoundingClientRect();
    await expect(
      Math.abs(centre(surface) - centre(chip.getBoundingClientRect())),
    ).toBeLessThanOrEqual(1.5);
    // The picture once the card has arrived, never mid-entrance.
    await waitFor(() => expect(surfaceOf(card).hasAttribute("data-entering")).toBe(false));
  },
};

/** **The ▲ dot**, while a card holding a warning is hidden: the most urgent of its three
 *  notices, though the newest is news, beside the amber waiting count. */
export const WarningDotBesideTheCount: Story = {
  args: { startOpen: false, stack: "stacked" },
  play: async ({ canvas }) => {
    const dot = canvas.getByRole("button", {
      name: "Show the 3 notices on 3rd-try, most urgent: warning",
    });
    await expect(dot).toHaveAttribute("data-kind", "warning");
    await expect(dot.querySelector("svg.lucide-triangle")).not.toBeNull();
    const chip = chipOf("3rd-try");
    const count = chip.querySelector('[data-part="count"]') as HTMLElement;
    const [d, c, k] = [dot, chip, count].map((one) => one.getBoundingClientRect());
    await expect(centre(d)).toBeLessThan(c.left + 8);
    await expect(d.top).toBeLessThan(c.top + 4);
    await expect(d.right).toBeLessThan(k.left);
    await expect(chip).toHaveAccessibleDescription("2 waiting, 3 notices, most urgent: warning");
  },
};

/** **Stacked, newest first** (#3649): one card for the chip, a line that says how many, the
 *  clone counting at the top. */
export const StackedNewestFirst: Story = {
  args: { stack: "stacked" },
  play: async () => {
    const card = await waitFor(() => cardOf() as HTMLElement);
    await expect(card).toHaveTextContent("3 notices, newest first");
    const words = card.textContent ?? "";
    const order = [
      "Cloning 2 repos…",
      "web is on spike/search, not main",
      "Workspace 3rd-try is ready",
    ];
    await expect(order.map((one) => words.indexOf(one))).toEqual(
      [...order.map((one) => words.indexOf(one))].sort((a, b) => a - b),
    );
    await expect(card).toHaveTextContent("1 of 2 done.");
    await expect(screen.getAllByRole("region", { name: /^Notices for / })).toHaveLength(1);
  },
};

/** **A clone that did not land** (#3649): its notice offers Retry and the way to the
 *  workspace's repo settings, and Retry asks for the clone again. */
export const CloneCouldNotLand: Story = {
  args: { stack: "failed" },
  play: async ({ canvas, userEvent }) => {
    const card = await waitFor(() => cardOf() as HTMLElement);
    await expect(card).toHaveTextContent("Could not clone web into 3rd-try");
    await expect(card).toHaveTextContent("2 notices, newest first");
    const retry = within(card).getByRole("button", { name: "Retry" });
    await expect(
      within(card).getByRole("button", { name: "Open 3rd-try's repo settings" }),
    ).toBeInTheDocument();
    await userEvent.click(retry);
    await expect(canvas.getByText("Asked to clone web again")).toBeInTheDocument();
  },
};

/**
 * **The project's findings about a workspace, on its chip** (#3650): a repo off its default branch
 * with its Switch, and the workspace's layout behind with its Update, under the ready notice. The
 * dot is ▲, the most urgent; the layout line has no × of its own, since it goes once it is mended.
 */
export const FindingsOnTheChip: Story = {
  args: { stack: "findings", startOpen: false },
  play: async ({ canvas, userEvent }) => {
    const dot = canvas.getByRole("button", {
      name: "Show the 3 notices on 3rd-try, most urgent: warning",
    });
    await expect(dot).toHaveAttribute("data-kind", "warning");
    dot.focus();
    await userEvent.keyboard("{Enter}");
    const card = await waitFor(() => cardOf() as HTMLElement);
    await untilFocused(() => card, "the card the ▲ dot brought");
    await expect(card).toHaveTextContent("web in 3rd-try is on spike/search, not main.");
    await expect(card).toHaveTextContent("3rd-try is behind the current layout.");
    const layout = card.querySelector<HTMLElement>('[data-cause="alert:reinit:3rd-try"]');
    await expect(layout).not.toBeNull();
    await expect(
      within(layout as HTMLElement).queryByRole("button", { name: "Dismiss" }),
    ).toBeNull();
    await userEvent.click(within(card).getByRole("button", { name: "Switch to main" }));
    await expect(canvas.getByText("Asked to switch web")).toBeInTheDocument();
    await waitFor(() => expect(surfaceOf(card).hasAttribute("data-entering")).toBe(false));
  },
};
