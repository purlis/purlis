// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Chats view in its side panel, its list scrolled under its head** (#3331): the title and
 * the filter stay at the top while the rows scroll under them (#1499), and the head is filled
 * with the panel's own fill (`--panel-fill`), so in Purlis, whose panels are a step up from the
 * window, it is one piece with the rows and not a band of the window's colour.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor } from "storybook/test";
import { ChatsSection } from "./ChatsSection";
import { ChatsHere, fixedChats, moved, nothingKnown } from "./chatState";
import { chatsTree, type ListedChat } from "./chatsTree";
import { panelRoot } from "./ui/panel/Panel";

/** A chat as the core lists it, open in a tab. */
function listed(session: number, more: Partial<ListedChat> = {}): ListedChat {
  return {
    session,
    name: `steward ${session}`,
    persona: "steward",
    workspace: "smart-ide",
    shell: false,
    parent: null,
    mode: null,
    from: null,
    tab: true,
    branch: null,
    report: null,
    outcome: null,
    asking: null,
    harness: "Claude Code",
    ...more,
  };
}

/** More chats than the panel holds, so the list scrolls. */
const ROWS = chatsTree(Array.from({ length: 30 }, (_, at) => listed(at + 1)));

/** The side panel the window draws the Chats view in: a panel of the family, as tall as a short
 *  window's, the view its one scroller. */
function ChatsInItsPanel() {
  return (
    <ChatsHere.Provider value={fixedChats(nothingKnown)}>
      {/* A panel root as the window's slot carries it (`RegionFrame`), sized as a short
          window's side panel is. */}
      <div
        {...panelRoot("side")}
        aria-label="Navigation"
        role="region"
        style={{ display: "flex", flexDirection: "column", inlineSize: 260, blockSize: 320 }}
      >
        <ChatsSection rows={ROWS} front={1} onOpen={() => {}} />
      </div>
    </ChatsHere.Provider>
  );
}

const meta = {
  component: ChatsInItsPanel,
} satisfies Meta<typeof ChatsInItsPanel>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **Scrolled, the head over the rows**: the list is scrolled a way down, the head still stands at
 * the top of the view, and the fill drawn under its words is the panel's, whichever the theme
 * gives its panels. The picture is the scrolled list.
 */
export const ScrolledUnderItsHead: Story = {
  play: async ({ canvasElement }) => {
    const section = canvasElement.querySelector<HTMLElement>(".chats-section");
    const head = canvasElement.querySelector<HTMLElement>('.chats-section > [data-part="top"]');
    const panel = canvasElement.querySelector<HTMLElement>(
      '[data-scope="panel"][data-part="root"]',
    );
    await expect(section).not.toBeNull();
    await expect(head).not.toBeNull();
    await expect(panel).not.toBeNull();
    const view = section as HTMLElement;

    view.scrollTop = 200;
    await waitFor(() => expect(view.scrollTop).toBeGreaterThan(0));

    // Sticky: the head stands at the top of the scrolled view.
    const top = (head as HTMLElement).getBoundingClientRect().top;
    await expect(Math.abs(top - view.getBoundingClientRect().top)).toBeLessThanOrEqual(1);
    // Filled with the panel's own colour, which is opaque, so no row shows through it.
    const fill = getComputedStyle(head as HTMLElement).backgroundColor;
    await expect(fill).toBe(getComputedStyle(panel as HTMLElement).backgroundColor);
  },
};

/** A chat at work with a task of it below that needs the person: the row wears the rolled-up
 *  hand. */
const ASKED = [
  listed(1, { name: "cancel mid-turn" }),
  listed(2, {
    name: "live check of staging",
    persona: "devops",
    parent: 1,
    mode: "task",
    from: "chat 1",
    tab: false,
    report: "owed",
  }),
];

/** Chat 1 at work, and its task waiting for the person. */
const WAITING = [
  [1, "running"],
  [2, "waiting"],
].reduce(
  (known, [session, state], at) =>
    moved(known, {
      plane: "/plane",
      session: session as number,
      state: state as "running" | "waiting",
      needs_you: session === 2,
      queue: [2],
      moved_at: at + 1,
      sequence: at + 1,
      reports: [],
      refusals: [],
      children: [],
      needs: null,
      stopped: null,
    }),
  nothingKnown,
);

/** The two rows in the side panel. */
function HandInItsPanel() {
  return (
    <ChatsHere.Provider value={fixedChats(WAITING)}>
      <div
        {...panelRoot("side")}
        aria-label="Navigation"
        role="region"
        style={{ display: "flex", flexDirection: "column", inlineSize: 260, blockSize: 160 }}
      >
        <ChatsSection rows={chatsTree(ASKED)} front={1} onOpen={() => {}} />
      </div>
    </ChatsHere.Provider>
  );
}

/**
 * **The rolled-up hand** (#3523): beside the row of a chat whose task below needs the person,
 * the button family's quiet icon button, its hand in the needs-you colour, named by where it
 * goes, as its tooltip says. It is out of the arrows' way (`tabIndex` -1). The picture is the two
 * rows at rest.
 */
export const RolledUpHand: Story = {
  render: () => <HandInItsPanel />,
  // The hand stands in the row's `li`, so the tree owns a button that is not a treeitem: a
  // structure older than the family's button, which this story is the first to draw (#3658).
  parameters: {
    a11y: { options: { rules: { "aria-required-children": { enabled: false } } } },
  },
  play: async ({ canvasElement }) => {
    const hand = canvasElement.querySelector<HTMLElement>("button.rolled-up");
    await expect(hand).not.toBeNull();
    const button = hand as HTMLElement;
    await expect(button).toHaveAttribute("data-scope", "button");
    await expect(button).toHaveAttribute("data-variant", "quiet");
    await expect(button).toHaveAttribute("tabindex", "-1");
    await expect(button.getAttribute("aria-label")).toMatch(/^Go to live check of staging/);
    // A target of at least 24px, as the family's sm is (WCAG 2.5.8).
    await expect(button.getBoundingClientRect().height).toBeGreaterThanOrEqual(24);
  },
};
