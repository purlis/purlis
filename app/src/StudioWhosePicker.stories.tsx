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
import { expectFocusInSight, untilFocusWithin } from "./stories.testkit";

const PLACES: StudioClone[] = [
  { clone: "purlis", repo_skills: 12, preview: "" },
  { clone: "charter-app", repo_skills: 3, preview: "" },
];

const WHOSE: StudioScope[] = [AT_THE_ROOT, inWorkspace("smart-ide"), asPersona("steward")];

/** The picker as the studio's header holds it: what is picked is drawn again. */
function Picker({ places, start }: { places: StudioClone[]; start: StudioScope }) {
  const [scope, setScope] = useState(start);
  return (
    <div style={{ display: "flex", justifyContent: "flex-end", inlineSize: 560 }}>
      <StudioWhosePicker scope={scope} whose={WHOSE} places={places} onScope={setScope} />
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

/**
 * **At the workspace's root**: Tab reaches the picker, Enter opens its menu on Whose chats, ↓
 * walks down to Where it starts and In purlis, Enter picks it and the picker says it; Escape
 * closes the menu again without a change. The picture is the picker at the root.
 */
export const AtTheRoot: Story = {
  play: async ({ canvas, userEvent }) => {
    const picker = canvas.getByRole("button", { name: "A chat in smart-ide · at its root" });
    await userEvent.tab();
    await expect(picker).toHaveFocus();
    expectFocusInSight();

    await userEvent.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: "Whose chats and where they start" });
    await untilFocusWithin(menu);
    await expect(within(menu).getByText("Its 12 repo skills count too.")).toBeVisible();
    // Past the three of Whose chats and the workspace's root, to purlis.
    for (let step = 0; step < 4; step++) await userEvent.keyboard("{ArrowDown}");
    const purlis = within(menu).getByRole("menuitemradio", { name: /In purlis/ });
    await waitFor(() => expect(purlis).toHaveFocus());
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), CLOSES);
    const there = canvas.getByRole("button", { name: "A chat in smart-ide · in purlis" });
    await waitFor(() => expect(there).toHaveFocus());

    await userEvent.keyboard("{Enter}");
    const again = await screen.findByRole("menu", { name: "Whose chats and where they start" });
    await expect(within(again).getByRole("menuitemradio", { name: /In purlis/ })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), CLOSES);
    await expect(there).toHaveFocus();
    (document.activeElement as HTMLElement | null)?.blur();
  },
};

/** **Open, in purlis**: the menu as the person sees it, the place ticked. */
export const OpenInAClone: Story = {
  args: { start: { ...inWorkspace("smart-ide"), where: "purlis" } },
  play: async ({ canvas, userEvent }) => {
    await userEvent.click(canvas.getByRole("button", { name: "A chat in smart-ide · in purlis" }));
    const menu = await screen.findByRole("menu", { name: "Whose chats and where they start" });
    await expect(within(menu).getByRole("menuitemradio", { name: /In purlis/ })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    await expect(
      within(menu).getByRole("menuitemradio", { name: "A chat in smart-ide" }),
    ).toHaveAttribute("aria-checked", "true");
  },
};

/** **No clone holds repo skills**: the select it always was, with nothing to choose of where. */
export const NoPlace: Story = {
  args: { places: [] },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("button", { name: /Whose chats/ })).toBeVisible();
    await expect(canvas.queryByText("Where it starts")).toBeNull();
  },
};
