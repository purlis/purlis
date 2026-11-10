// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The number on an artifact's icon** (#5965, spec #5948 story 22): what the data declares,
 * drawn where an activity item draws its count. Each state in one bar: a fresh number, a stale
 * one dimmed with its icon, one past 99, a zero and none, which draw nothing. Held to axe and to
 * the keyboard.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import type { ArtifactFresh, ArtifactPinShown } from "./bindings";
import { ArtifactGroup } from "./ArtifactGroup";
import { expectFocusInSight } from "./stories.testkit";
import { ActivityBarFrame } from "./ui/activity/ActivityItem";

const HOUR = 60 * 60 * 1000;

/** Updated `hours` ago, and stale a day after. */
const updated = (hours: number): ArtifactFresh => {
  const at = Date.now() - hours * HOUR;
  return { at, stale_at: at + 24 * HOUR, source: "declared" };
};

const pin = (
  folder: string,
  name: string,
  icon: string,
  badge: number | null,
  fresh: ArtifactFresh,
): ArtifactPinShown => ({
  id: `id-${folder}`,
  folder,
  name,
  icon,
  missing: null,
  live: true,
  too_large: null,
  fresh,
  badge,
});

const PINS: ArtifactPinShown[] = [
  pin("backlog-state", "Backlog state", "chart", 12, updated(1)),
  pin("release", "Release", "rocket", 3, updated(48)),
  pin("inbox", "Inbox", "mail", 1204, updated(2)),
  pin("done", "Done", "star", 0, updated(1)),
  pin("notes", "Notes", "book", null, updated(1)),
];

const meta: Meta<typeof ArtifactGroup> = {
  title: "Views/Pinned artifact/Badge",
  component: ArtifactGroup,
};
export default meta;

/** Every badge state on the left bar. Release's data is two days old: its badge dims with it. */
export const States: StoryObj<typeof ArtifactGroup> = {
  render: () => (
    <ActivityBarFrame
      variant="bar"
      className="activity-column"
      data-side="left"
      top={<ArtifactGroup plane="p" workspace="alpha" pins={PINS} onOpen={() => {}} />}
      bottom={null}
    />
  ),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    const drawn = (name: string) =>
      page
        .getByRole("button", { name })
        .querySelector('[data-part="count"]')
        ?.getAttribute("data-count");
    await expect(drawn("Backlog state")).toBe("12");
    await expect(drawn("Release")).toBe("3");
    await expect(drawn("Inbox")).toBe("99+");
    await expect(drawn("Done")).toBeUndefined();
    await expect(drawn("Notes")).toBeUndefined();

    const stale = page.getByRole("button", { name: "Release" });
    await expect(stale).toHaveAttribute("data-dimmed");
    await expect(stale).toHaveAccessibleDescription(/Its data shows 3, and is out of date$/);
    await expect(page.getByRole("button", { name: "Inbox" })).toHaveAccessibleDescription(
      /Its data shows 1204$/,
    );

    // The badge takes no stop of its own: the arrows walk the icons as before.
    await userEvent.tab();
    await expect(page.getByRole("button", { name: "Backlog state" })).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    await expect(stale).toHaveFocus();
    expectFocusInSight();
  },
};
