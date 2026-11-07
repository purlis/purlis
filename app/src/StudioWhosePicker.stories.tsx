// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Skill studio's scope picker, with Where** (#4227; variant A of
 * `app/src/SkillStudioPlace.prototype.html`, commit 71b51f075): Whose chats, and where the chat
 * the studio previews starts, at the workspace's root or in each clone that holds repo skills.
 * Walked by the keyboard, held to axe, and pictured in every pictured theme cell.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import type { StudioClone } from "./bindings";
import { AT_THE_ROOT, asPersona, inWorkspace, type StudioScope } from "./skillStudio";
import { StudioWhosePicker } from "./StudioWhosePicker";
import { expectFocusInSight, untilFocusWithin, untilFocused } from "./stories.testkit";

const PLACES: StudioClone[] = [
  { clone: "purlis", repo_skills: 12, preview: "" },
  { clone: "charter-app", repo_skills: 3, preview: "" },
];

const WHOSE: StudioScope[] = [AT_THE_ROOT, inWorkspace("smart-ide"), asPersona("steward")];

/** Twelve workspaces: a list long enough to be filtered. */
const MANY: StudioScope[] = [
  AT_THE_ROOT,
  ...[
    "smart-ide",
    "billing",
    "checkout",
    "docs-site",
    "infra",
    "mobile",
    "payments",
    "search",
    "support",
    "web",
    "warehouse",
  ].map(inWorkspace),
];

/** The picker as the studio's header holds it, after a button the keyboard leaves to: what is
 *  picked is drawn again. */
function Picker({
  places,
  start,
  whose = WHOSE,
}: {
  places: StudioClone[];
  start: StudioScope;
  whose?: StudioScope[];
}) {
  const [scope, setScope] = useState(start);
  return (
    <div style={{ display: "flex", justifyContent: "flex-end", inlineSize: 560 }}>
      <StudioWhosePicker scope={scope} whose={whose} places={places} onScope={setScope} />
    </div>
  );
}

const meta = {
  component: Picker,
  args: { places: PLACES, start: inWorkspace("smart-ide") },
} satisfies Meta<typeof Picker>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Takes a moment longer than a frame: the menu's surface goes as it closes. */
const CLOSES = { timeout: 3_000 };

const MENU = "Whose chats and where they start";

/**
 * **At the workspace's root**: Tab reaches the picker, Enter opens its menu, ↓ walks past Whose
 * chats to Where it starts and In purlis, Enter picks it and the picker says it; Enter opens it
 * again with purlis ticked, Escape closes it with nothing changed, and Shift+Tab leaves. The
 * picture is the picker in purlis.
 */
export const AtTheRoot: Story = {
  play: async ({ canvas, userEvent }) => {
    const picker = canvas.getByRole("button", { name: "A chat in smart-ide · at its root" });
    await userEvent.tab();
    await untilFocused(() => picker, "the picker");
    expectFocusInSight();

    await userEvent.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: MENU });
    await untilFocusWithin(() => menu, "the menu");
    expectFocusInSight();
    // Past the three of Whose chats and the workspace's root, to purlis.
    for (let step = 0; step < 4; step++) {
      await userEvent.keyboard("{ArrowDown}");
      expectFocusInSight();
    }
    const purlis = within(menu).getByRole("menuitemradio", { name: "In purlis" });
    await untilFocused(() => purlis, "In purlis");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), CLOSES);
    const there = canvas.getByRole("button", { name: "A chat in smart-ide · in purlis" });
    await untilFocused(() => there, "the picker, in purlis");
    expectFocusInSight();

    await userEvent.keyboard("{Enter}");
    const again = await screen.findByRole("menu", { name: MENU });
    await expect(within(again).getByRole("menuitemradio", { name: "In purlis" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), CLOSES);
    await untilFocused(() => there, "the picker, after Escape");
    expectFocusInSight();
    await userEvent.tab({ shift: true });
    await waitFor(() => expect(there).not.toHaveFocus());
  },
};

/** **Open, in purlis**: the menu as the person sees it, opened from the keyboard, its place and
 *  its workspace ticked. */
export const OpenInAClone: Story = {
  args: { start: { ...inWorkspace("smart-ide"), where: "purlis" } },
  parameters: { screenshot: "page" },
  play: async ({ canvas, userEvent }) => {
    const picker = canvas.getByRole("button", { name: "A chat in smart-ide · in purlis" });
    await userEvent.tab();
    await untilFocused(() => picker, "the picker");
    await userEvent.keyboard("{ArrowDown}");
    const menu = await screen.findByRole("menu", { name: MENU });
    await untilFocusWithin(() => menu, "the menu");
    expectFocusInSight();
    await expect(within(menu).getByRole("menuitemradio", { name: "In purlis" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    await expect(
      within(menu).getByRole("menuitemradio", { name: "A chat in smart-ide" }),
    ).toHaveAttribute("aria-checked", "true");
  },
};

/** **Many workspaces**: the menu opens on its filter row, and what is typed narrows Whose chats
 *  while Where it starts stays; Escape closes it. The picture is the menu narrowed. */
export const ManyWorkspaces: Story = {
  args: { whose: MANY },
  parameters: { screenshot: "page" },
  play: async ({ canvas, userEvent }) => {
    const picker = canvas.getByRole("button", { name: "A chat in smart-ide · at its root" });
    await userEvent.tab();
    await untilFocused(() => picker, "the picker");
    await userEvent.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: MENU });
    const filter = await screen.findByRole("textbox", { name: "Filter" });
    await untilFocused(() => filter, "the filter");
    expectFocusInSight();
    await userEvent.keyboard("pay");
    await waitFor(() =>
      expect(within(menu).queryByRole("menuitemradio", { name: "A chat in billing" })).toBeNull(),
    );
    await expect(
      within(menu).getByRole("menuitemradio", { name: "A chat in payments" }),
    ).toBeVisible();
    await expect(within(menu).getByRole("menuitemradio", { name: "In purlis" })).toBeVisible();
  },
};

/** **No clone holds repo skills**: the select it always was. Tab reaches it, Enter opens its
 *  list, Escape closes it, and Tab leaves. */
export const NoPlace: Story = {
  args: { places: [] },
  play: async ({ canvas, userEvent }) => {
    const select = canvas.getByRole("button", { name: /Whose chats/ });
    await userEvent.tab();
    await untilFocused(() => select, "the select");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await screen.findByRole("listbox");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull(), CLOSES);
    await untilFocused(() => select, "the select, after Escape");
    await expect(canvas.queryByText("Where it starts")).toBeNull();
    await userEvent.tab();
    await waitFor(() => expect(select).not.toHaveFocus());
  },
};
