// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **PROTOTYPE v2, NOT FOR MERGE: Asks scoped to Chat, Workspace, Project** (replaces #6310). The
 * window's own rail, Asks view and ✋, with the scope chips, their counts' one slide and the ✋'s
 * bounces, moved by the box of controls under them (`AsksScopes.prototype.tsx`). Light and dark
 * are the toolbar's Appearance. Not pictured: a prototype draws no baseline.
 */
import "./AsksScopes.prototype.css";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { AsksScopesPrototype, type Counts } from "./AsksScopes.prototype";

const meta = {
  title: "Prototype/Asks scopes v2",
  component: AsksScopesPrototype,
  parameters: { screenshot: false },
} satisfies Meta<typeof AsksScopesPrototype>;

export default meta;
type Story = StoryObj<typeof meta>;

/** One in the focused chat, two more in this workspace, one in another workspace; website has
 *  two of its own. The chips read 1, 3, 4, and the ✋ 4. */
const SOME: Counts = {
  chat: 1,
  workspaceRest: 2,
  otherWorkspaces: 1,
  others: { website: 2, infra: 0 },
};

const NONE: Counts = {
  chat: 0,
  workspaceRest: 0,
  otherWorkspaces: 0,
  others: { website: 0, infra: 0 },
};

/** The ✋'s wrapper, which carries the bounce class and how many bounces were drawn. */
const handOf = (root: HTMLElement) => {
  const hand = root.querySelector<HTMLElement>(".asks-proto-hand");
  if (hand === null) throw new Error("the ✋ is not drawn");
  return hand;
};

/** Every word drawn for a screen reader alone (the families' `said` parts, `sr-only`) is clipped
 *  to nothing on screen, and no count draws a word: a chip shows its name and its number. */
async function expectNoHiddenWordDrawn(root: HTMLElement) {
  const hidden = [...root.querySelectorAll<HTMLElement>('.sr-only, [data-part="said"]')];
  await expect(hidden.length).toBeGreaterThan(0);
  for (const one of hidden) {
    const box = one.getBoundingClientRect();
    await expect({ said: one.textContent, wide: box.width <= 1 && box.height <= 1 }).toEqual({
      said: one.textContent,
      wide: true,
    });
  }
  for (const count of root.querySelectorAll<HTMLElement>(
    '[data-scope="count"][data-part="root"]',
  )) {
    await expect((count.innerText ?? count.textContent ?? "").trim()).toMatch(/^(\d{1,2}|99\+)$/);
  }
}

/** The drawn width of a chip's count, and of the ✋'s. */
const widths = (root: HTMLElement) => ({
  chip: root.querySelector('[data-scope="segment"] [data-scope="count"]')?.getBoundingClientRect()
    .width,
  hand: root
    .querySelector('.asks-proto-hand [data-scope="activity"][data-part="count"]')
    ?.getBoundingClientRect().width,
});

/** **Something waits in every scope**: Asks opens on Chat; nothing hidden is drawn; a count
 *  keeps its width from 1 to 99+; the ✋ bounces when its count goes up, and only then. */
export const SomethingWaitsEverywhere: Story = {
  args: { start: SOME },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("radio", { name: "Chat, 1 waiting on you" })).toBeChecked();
    await expect(
      canvas.getByRole("radio", { name: "Workspace, 3 waiting on you" }),
    ).toBeInTheDocument();
    await expect(
      canvas.getByRole("radio", { name: "Project, 4 waiting on you" }),
    ).toBeInTheDocument();
    await expect(canvas.getByRole("tab", { name: "Asks 4 waiting on you" })).toBeInTheDocument();
    await expectNoHiddenWordDrawn(canvasElement);

    // The room of 99+: the same width at 1 and at 101.
    const before = widths(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Focused chat and subtree +100" }));
    await waitFor(() =>
      expect(canvas.getByRole("radio", { name: "Chat, 101 waiting on you" })).toBeChecked(),
    );
    await expect(widths(canvasElement)).toEqual(before);
    await expectNoHiddenWordDrawn(canvasElement);

    // The bounce, on an increase only: watched as it is drawn, since under the system's reduced
    // motion it ends as soon as it starts. The +100 above bounced it; in a browser, its three
    // bounces end and take the class off. (A page with no animations, jsdom, never ends one, so
    // there only the count of bounces drawn is read.)
    const hand = handOf(canvasElement);
    const animates = typeof document.getAnimations === "function";
    let bounced = false;
    const watch = new MutationObserver(() => {
      if (hand.classList.contains("asks-proto-bounce")) bounced = true;
    });
    watch.observe(hand, { attributes: true, attributeFilter: ["class"] });
    if (animates)
      await waitFor(() => expect(hand).not.toHaveClass("asks-proto-bounce"), { timeout: 3000 });
    bounced = false;
    const drawn = Number(hand.dataset.bounces);
    await userEvent.click(canvas.getByRole("button", { name: "Focused chat and subtree −1" }));
    await waitFor(() =>
      expect(canvas.getByRole("radio", { name: "Chat, 100 waiting on you" })).toBeChecked(),
    );
    await expect(bounced).toBe(false);
    await expect(Number(hand.dataset.bounces)).toBe(drawn);
    await userEvent.click(canvas.getByRole("button", { name: "Rest of this workspace +1" }));
    await waitFor(() => expect(Number(hand.dataset.bounces)).toBe(drawn + 1));
    if (animates) await waitFor(() => expect(bounced).toBe(true));
    else await expect(hand).toHaveClass("asks-proto-bounce");
    watch.disconnect();
  },
};

/** **The chat is empty**: Chat opens empty and offers the workspace's, and never switches by
 *  itself; the line widens when pressed. */
export const TheChatIsEmpty: Story = {
  args: { start: { ...SOME, chat: 0 } },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("radio", { name: "Chat" })).toBeChecked();
    await expect(canvas.getByText("Nothing is waiting on you in this chat.")).toBeVisible();
    await userEvent.click(canvas.getByRole("button", { name: "2 waiting in this workspace ›" }));
    await waitFor(() =>
      expect(canvas.getByRole("radio", { name: "Workspace, 2 waiting on you" })).toBeChecked(),
    );
  },
};

/** **One workspace**: no Workspace chip; Chat and Project count the same project. */
export const OneWorkspace: Story = {
  args: { start: SOME, setup: { oneWorkspace: true } },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.queryByRole("radio", { name: /^Workspace/ })).not.toBeInTheDocument();
    await expect(
      canvas.getByRole("radio", { name: "Project, 3 waiting on you" }),
    ).toBeInTheDocument();
  },
};

/** **No chat focused**: Chat is greyed, and Asks uses Workspace. */
export const NoChatFocused: Story = {
  args: { start: SOME, setup: { noChatFocused: true } },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("radio", { name: "Chat" })).toBeDisabled();
    await expect(canvas.getByRole("radio", { name: "Workspace, 3 waiting on you" })).toBeChecked();
  },
};

/** **No chat focused, one workspace**: Chat is greyed, and Asks uses Project. */
export const NoChatFocusedOneWorkspace: Story = {
  args: { start: SOME, setup: { noChatFocused: true, oneWorkspace: true } },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("radio", { name: "Project, 3 waiting on you" })).toBeChecked();
  },
};

/** **Over 99**: the chips, this project's tab and the ✋ say 99+, in the room they always had. */
export const OverNinetyNine: Story = {
  args: { start: { ...SOME, chat: 120 } },
  play: async ({ canvasElement }) => {
    await expectNoHiddenWordDrawn(canvasElement);
    await expect(
      canvasElement.querySelector('.asks-proto-hand [data-part="count"]'),
    ).toHaveAttribute("data-count", "99+");
  },
};

/** **Nothing waits**: no chip, tab or ✋ draws a number, and no reminder is scheduled. */
export const NothingWaits: Story = {
  args: { start: NONE },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("radio", { name: "Chat" })).toBeChecked();
    await expect(canvas.getByText("No reminders: no Blocking ask waits.")).toBeVisible();
    await expect(
      canvasElement.querySelector('[data-part="count"], [data-part="badge"]'),
    ).toBeNull();
  },
};

/** **Reminders, Gently, at 600×**: the first comes 5 minutes on, half a second here; the next
 *  line then says the 10-minute gap. */
export const RemindersGently: Story = {
  args: { start: SOME, setup: { speed: 600, remind: "gently" } },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await waitFor(() => expect(canvas.getByText(/last bounce: Reminder 1 at/)).toBeVisible(), {
      timeout: 3000,
    });
    await waitFor(() => expect(canvas.getByText(/\(gap 10 min\)/)).toBeVisible());
  },
};

/** **Reminders, Often, at 600×**: every 5 minutes, without backing off. */
export const RemindersOften: Story = {
  args: { start: SOME, setup: { speed: 600, remind: "often" } },
};

/** **Remind me Off**: the ✋ still bounces on a new ask, and never again by itself. */
export const RemindMeOff: Story = {
  args: { start: SOME, setup: { remind: "off" } },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("No reminders: Remind me is Off.")).toBeVisible();
  },
};

/** **Quiet**: a new ask moves the counts, and the ✋ holds still. */
export const QuietHoldsTheHand: Story = {
  args: { start: SOME, setup: { quiet: true } },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const hand = handOf(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Focused chat and subtree +1" }));
    await waitFor(() => expect(canvas.getByText(/New ask at .*, held: Quiet is on/)).toBeVisible());
    await expect(hand.dataset.bounces).toBe("0");
  },
};

/** **Typing**: the same, while the person types. */
export const TypingHoldsTheHand: Story = {
  args: { start: SOME, setup: { typing: true } },
};

/** **The window behind**: the same, while another app is in front. */
export const WindowBehind: Story = {
  args: { start: SOME, setup: { inFront: false } },
};

/** **Reduced motion**: counts swap without sliding, and the ✋ holds still. */
export const ReducedMotion: Story = {
  args: { start: SOME, setup: { reduced: true } },
};
