// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Skill studio's scope picker, with Where** (#4227; variant A of
 * `app/src/SkillStudioPlace.prototype.html`, commit 71b51f075): Whose chats, and where the chat
 * the studio previews starts, at the workspace's root or in each clone that holds repo skills.
 * **And the project's personas** (#2935; variant A of the `prototype/studio-whose-personas`
 * branch): As a persona, and with one picked, In which workspace it was dispatched.
 * Walked by the keyboard, held to axe, and pictured in every pictured theme cell.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import type { StudioClone, StudioPersona } from "./bindings";
import { AT_THE_ROOT, asPersona, asPersonaIn, inWorkspace, type StudioScope } from "./skillStudio";
import { StudioWhosePicker } from "./StudioWhosePicker";
import { expectFocusInSight, pictureNow, untilFocusWithin, untilFocused } from "./stories.testkit";

const PLACES: StudioClone[] = [
  { clone: "purlis", repo_skills: 12, preview: "" },
  { clone: "charter-app", repo_skills: 3, preview: "" },
];

const WHOSE: StudioScope[] = [AT_THE_ROOT, inWorkspace("smart-ide")];

/** The prototype's five personas, each with its role and where it was dispatched. */
const PERSONAS: StudioPersona[] = [
  {
    name: "steward",
    role: "Control plane steward",
    dispatched_into: [
      { workspace: "smart-ide", chats: 41 },
      { workspace: "billing", chats: 3 },
    ],
  },
  {
    name: "reviewer",
    role: "Code reviewer",
    dispatched_into: [{ workspace: "smart-ide", chats: 18 }],
  },
  {
    name: "devops",
    role: "Infrastructure and CI",
    dispatched_into: [
      { workspace: "infra", chats: 12 },
      { workspace: "smart-ide", chats: 2 },
    ],
  },
  {
    name: "writer",
    role: "Docs and changelog",
    dispatched_into: [{ workspace: "docs-site", chats: 9 }],
  },
  { name: "qa", role: "Scenario specs", dispatched_into: [] },
];

/** Twelve personas: a list long enough to be filtered. */
const MANY_PERSONAS: StudioPersona[] = [
  ...PERSONAS,
  ...["designer", "triager", "release", "security", "data", "support", "mobile"].map((name) => ({
    name,
    role: null,
    dispatched_into: [],
  })),
];

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
  personas = [],
}: {
  places: StudioClone[];
  start: StudioScope;
  whose?: StudioScope[];
  personas?: StudioPersona[];
}) {
  const [scope, setScope] = useState(start);
  return (
    <div style={{ display: "flex", justifyContent: "flex-end", inlineSize: 560 }}>
      <StudioWhosePicker
        scope={scope}
        whose={whose}
        personas={personas}
        places={places}
        onScope={setScope}
      />
    </div>
  );
}

const meta = {
  component: Picker,
  args: { places: PLACES, start: inWorkspace("smart-ide"), personas: [] },
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
    // Past the two of Whose chats and the workspace's root, to purlis.
    for (let step = 0; step < 3; step++) {
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

/**
 * **As a persona, then in a workspace it was dispatched into** (#2935): Tab reaches the picker,
 * Enter opens its menu, ↓ walks past Whose chats to As a persona and steward, Enter picks it and
 * the picker says it. Enter opens it again with In which workspace in place of Where it starts,
 * ↓ walks to Dispatched into smart-ide, Enter picks it; Escape closes the menu opened again with
 * nothing changed, and Shift+Tab leaves.
 */
export const AsAPersona: Story = {
  args: { places: [], personas: PERSONAS },
  play: async ({ canvas, userEvent }) => {
    const picker = canvas.getByRole("button", { name: "A chat in smart-ide" });
    await userEvent.tab();
    await untilFocused(() => picker, "the picker");
    expectFocusInSight();

    await userEvent.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: MENU });
    await untilFocusWithin(() => menu, "the menu");
    await expect(within(menu).queryByText("Where it starts")).toBeNull();
    // Past the two of Whose chats, to steward.
    for (let step = 0; step < 2; step++) {
      await userEvent.keyboard("{ArrowDown}");
      expectFocusInSight();
    }
    const steward = within(menu).getByRole("menuitemradio", { name: "A chat as steward" });
    await untilFocused(() => steward, "A chat as steward");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), CLOSES);
    const as = canvas.getByRole("button", { name: "A chat as steward" });
    await untilFocused(() => as, "the picker, as steward");
    expectFocusInSight();

    await userEvent.keyboard("{Enter}");
    const again = await screen.findByRole("menu", { name: MENU });
    await untilFocusWithin(() => again, "the menu again");
    // Past Whose chats' two and the five personas, then the project's root, to smart-ide.
    for (let step = 0; step < 8; step++) {
      await userEvent.keyboard("{ArrowDown}");
      expectFocusInSight();
    }
    const into = within(again).getByRole("menuitemradio", { name: "Dispatched into smart-ide" });
    await untilFocused(() => into, "Dispatched into smart-ide");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), CLOSES);
    const there = canvas.getByRole("button", { name: "A chat as steward in smart-ide" });
    await untilFocused(() => there, "the picker, as steward in smart-ide");
    expectFocusInSight();

    await userEvent.keyboard("{Enter}");
    await screen.findByRole("menu", { name: MENU });
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), CLOSES);
    await untilFocused(() => there, "the picker, after Escape");
    expectFocusInSight();
    await userEvent.tab({ shift: true });
    await waitFor(() => expect(there).not.toHaveFocus());
  },
};

/** **Open, as steward in smart-ide**: the menu as the person sees it, opened from the keyboard,
 *  the persona and the workspace it was dispatched into ticked, each with what is under it. */
export const OpenAsAPersonaInAWorkspace: Story = {
  args: { places: [], personas: PERSONAS, start: asPersonaIn("steward", "smart-ide") },
  parameters: { screenshot: "page" },
  play: async ({ canvas, userEvent }) => {
    const picker = canvas.getByRole("button", { name: "A chat as steward in smart-ide" });
    await userEvent.tab();
    await untilFocused(() => picker, "the picker");
    await userEvent.keyboard("{ArrowDown}");
    const menu = await screen.findByRole("menu", { name: MENU });
    await untilFocusWithin(() => menu, "the menu");
    expectFocusInSight();
    await expect(
      within(menu).getByRole("menuitemradio", { name: "A chat as steward" }),
    ).toHaveAttribute("aria-checked", "true");
    await expect(
      within(menu).getByRole("menuitemradio", { name: "Dispatched into smart-ide" }),
    ).toHaveAttribute("aria-checked", "true");
    await expect(
      within(menu).queryByRole("menuitemradio", { name: "Dispatched into infra" }),
    ).toBeNull();
  },
};

/** **Twelve personas**: the menu opens on its filter row, and what is typed narrows Whose chats
 *  and As a persona while In which workspace stays; Escape closes it. The picture is the menu
 *  narrowed. */
export const ManyPersonas: Story = {
  args: { places: [], personas: MANY_PERSONAS, start: asPersona("steward") },
  parameters: { screenshot: "page" },
  play: async ({ canvas, userEvent }) => {
    const picker = canvas.getByRole("button", { name: "A chat as steward" });
    await userEvent.tab();
    await untilFocused(() => picker, "the picker");
    await userEvent.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: MENU });
    const filter = await screen.findByRole("textbox", { name: "Filter" });
    await untilFocused(() => filter, "the filter");
    expectFocusInSight();
    await userEvent.keyboard("re");
    await waitFor(() =>
      expect(within(menu).queryByRole("menuitemradio", { name: "A chat as qa" })).toBeNull(),
    );
    await expect(
      within(menu).getByRole("menuitemradio", { name: "A chat as reviewer" }),
    ).toBeVisible();
    await expect(
      within(menu).getByRole("menuitemradio", { name: "A chat as release" }),
    ).toBeVisible();
    await expect(
      within(menu).getByRole("menuitemradio", { name: "Dispatched into smart-ide" }),
    ).toBeVisible();
    // The picture is the menu narrowed, taken before Escape closes it.
    await pictureNow();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), CLOSES);
    await untilFocused(() => picker, "the picker, after Escape");
  },
};
