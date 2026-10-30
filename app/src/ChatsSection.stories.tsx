// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Chats view in its side panel, its list scrolled under its head** (#3331): the title and
 * the filter stay at the top while the rows scroll under them (#1499), and the head is filled
 * with the panel's own fill (`--panel-fill`), so in Purlis, whose panels are a step up from the
 * window, it is one piece with the rows and not a band of the window's colour.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { aCardIsUp } from "./chatCard";
import { CARD_LEAVE_MS, ChatsSection, ON_MEMORY_SAID } from "./ChatsSection";
import { ChatsHere, fixedChats, moved, nothingKnown } from "./chatState";
import { chatsTree, type ListedChat } from "./chatsTree";
import { expectFocusInSight, untilFocused } from "./stories.testkit";
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
 * goes, as its tooltip says. It is out of the arrows' way (`tabIndex` -1), and the pointer's
 * alone (`aria-hidden`, #3658): the tree owns only its rows, and the row says which chat below
 * needs you. The picture is the two rows at rest.
 */
export const RolledUpHand: Story = {
  render: () => <HandInItsPanel />,
  play: async ({ canvasElement }) => {
    const hand = canvasElement.querySelector<HTMLElement>("button.rolled-up");
    await expect(hand).not.toBeNull();
    const button = hand as HTMLElement;
    await expect(button).toHaveAttribute("data-scope", "button");
    await expect(button).toHaveAttribute("data-variant", "quiet");
    await expect(button).toHaveAttribute("tabindex", "-1");
    await expect(button.closest('[aria-hidden="true"]')).not.toBeNull();
    await expect(button.getAttribute("aria-label")).toMatch(/^Go to live check of staging/);
    // A target of at least 24px, as the family's sm is (WCAG 2.5.8).
    await expect(button.getBoundingClientRect().height).toBeGreaterThanOrEqual(24);
  },
};

/** Two chats, and the one that arrives while the person is reading the first's card. */
const BEFORE = chatsTree([listed(1), listed(2)]);
const AFTER = chatsTree([listed(1), listed(2), listed(3)]);

/** What brings the third chat in: the story's own, as the core's answer would. */
let arrive = () => {};

/** The two rows in the side panel, as tall as a short window's, and a chat on its way. */
function ArrivingInItsPanel() {
  const [rows, setRows] = useState(BEFORE);
  useEffect(() => {
    arrive = () => setRows(AFTER);
  }, []);
  return (
    <ChatsHere.Provider value={fixedChats(nothingKnown)}>
      <div
        {...panelRoot("side")}
        aria-label="Navigation"
        role="region"
        style={{ display: "flex", flexDirection: "column", inlineSize: 260, blockSize: 160 }}
      >
        <ChatsSection rows={rows} front={1} onOpen={() => {}} />
      </div>
    </ChatsHere.Provider>
  );
}

/** The rows the list draws. */
const rowsIn = (canvas: HTMLElement) => canvas.querySelectorAll('[role="treeitem"]').length;

/** One pointer for a whole walk, which says what it leaves as it moves, as a real one does:
 *  each call of the bare `userEvent` starts a pointer of its own, from nowhere. */
type Pointer = ReturnType<typeof userEvent.setup>;

/** Moves the pointer to a point of the page, onto whatever the page draws there. */
async function pointAt(user: Pointer, x: number, y: number) {
  const there = document.elementFromPoint(x, y);
  await expect(there).not.toBeNull();
  await user.pointer({ target: there as Element, coords: { clientX: x, clientY: y } });
}

/**
 * **The hold's wait, in the story's hands** (#3951): every timer set for the card's wait
 * (`CARD_LEAVE_MS`) while it is in place is parked, not run, until {@link release}. So the hold
 * waits as long as the story takes, however slow the runner, and lets go only when the story
 * says the wait is over. Any other timer runs as it would.
 */
function parkTheCardsWait() {
  const set = window.setTimeout;
  const clear = window.clearTimeout;
  const parked = new Map<number, () => void>();
  window.setTimeout = ((run: TimerHandler, ms?: number, ...args: unknown[]) => {
    if (ms !== CARD_LEAVE_MS || typeof run !== "function") return set(run, ms, ...args);
    // An id of the window's own, never run, so a clear of it is a clear.
    const id = set(() => {}, 2 ** 31 - 1);
    parked.set(id, () => (run as (...given: unknown[]) => void)(...args));
    return id;
  }) as typeof window.setTimeout;
  window.clearTimeout = ((id?: number) => {
    if (id !== undefined) parked.delete(id);
    clear(id);
  }) as typeof window.clearTimeout;
  return {
    /** How many of the card's waits are running. */
    waiting: () => parked.size,
    /** Each wait running is over: what it was waiting to do is done. */
    release: () => {
      const due = [...parked.entries()];
      parked.clear();
      for (const [id, run] of due) {
        clear(id);
        run();
      }
    },
    restore: () => {
      window.setTimeout = set;
      window.clearTimeout = clear;
    },
  };
}

/**
 * **The window stays the one in use** (#3951): the story's frame shares the runner's page with
 * other stories' frames, and one of them taking the focus blurs this window, which lets every
 * hold go at once, as it should for the person. That is not what this story holds to, so the
 * window's blur is kept from the hold while the story walks; its own effect is held by
 * `heldStill.test.tsx`.
 */
function keepTheWindowInUse() {
  const keep = (event: Event) => event.stopImmediatePropagation();
  // A listener on the window's capture runs before the hold's own, at the window itself.
  window.addEventListener("blur", keep, true);
  return () => window.removeEventListener("blur", keep, true);
}

/** Two frames: whatever a change asked to draw has been drawn. */
const drawn = () => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)));

/**
 * **The pointer crosses from a row to its card, and the list holds still** (#3773, V100-47):
 * the card stands a step to the side of its row, over the view's own inset, which is not the
 * list. A pointer that crosses there on its way to the card keeps the rows where they were, so
 * a chat arriving meanwhile is drawn only once the pointer has left for anywhere else. Laid out
 * by the browser: where the gap is, and what is under it, is the page's layout, which a window
 * test cannot see. It waits on what it checks, never on the clock (#3951): the card is up
 * before the chat arrives, and the hold's wait is over only when the story says so.
 */
export const HeldAcrossTheGapToTheCard: Story = {
  render: () => <ArrivingInItsPanel />,
  // What it holds to is what the pointer does, and its rows at rest are `ScrolledUnderItsHead`'s.
  parameters: { screenshot: false },
  play: async ({ canvasElement }) => {
    const user = userEvent.setup();
    const row = canvasElement.querySelector<HTMLElement>('[role="treeitem"][data-session="1"]');
    await expect(row).not.toBeNull();
    const inUse = keepTheWindowInUse();
    const clock = parkTheCardsWait();
    try {
      await user.hover(row as HTMLElement);
      // The card is up, as the list's hold reads it, before anything arrives.
      const card = await waitFor(
        () => {
          const up = document.querySelector<HTMLElement>('[data-testid="chat-card-1"]');
          expect(up).not.toBeNull();
          expect(aCardIsUp()).toBe(true);
          return up as HTMLElement;
        },
        { timeout: 3000 },
      );
      // A chat arrives while the pointer is on the list: held.
      arrive();
      await drawn();
      await expect(rowsIn(canvasElement)).toBe(2);

      // The gap between the row and its card, and what the page draws there: not the list.
      const from = (row as HTMLElement).getBoundingClientRect();
      const to = card.getBoundingClientRect();
      await expect(to.left - from.right).toBeGreaterThan(1);
      const y = from.top + from.height / 2;
      const gap = (from.right + to.left) / 2;
      await expect(
        canvasElement.querySelector(".chats-list")?.contains(document.elementFromPoint(gap, y)),
      ).toBe(false);

      // Onto the gap: the pointer has left the list, and the hold waits for it.
      await pointAt(user, gap, y);
      await drawn();
      await expect(clock.waiting()).toBeGreaterThan(0);
      await expect(rowsIn(canvasElement)).toBe(2);
      // Then onto the card, which is the list's: still held, however long that took.
      await pointAt(user, to.left + 8, to.top + 8);
      await expect(card).toBeVisible();
      await drawn();
      await expect(rowsIn(canvasElement)).toBe(2);

      // Off the card for anywhere else: once the wait is over, the list lets go, and the chat
      // that arrived is drawn.
      await pointAt(user, to.left + 8, to.bottom + 40);
      await drawn();
      clock.release();
      await waitFor(() => expect(rowsIn(canvasElement)).toBe(3), { timeout: 2000 });
    } finally {
      clock.restore();
      inUse();
    }
  },
};

/** The two rows drawn with what is said under each: chat 1 at its task limit, with a dispatch
 *  waiting on memory, and chat 2, which has no tab, with a restart it asked for refused. */
const UNDER = chatsTree([
  listed(1, {
    atLimit: {
      limit: 6,
      row: "at its task limit (6)",
      said: "steward 1 is at its task limit: 6 tasks run at once. Change it in Settings › Project › Dispatch.",
      own: true,
    },
    waitingOnMemory: 1,
  }),
  listed(2, { tab: false }),
]);

/** What chat 2's restart says. */
const RESTART = "Not restarted: steward 2 ended while the restart was asked.";

/** The rows in the side panel, with what is said under them. */
function SaidUnderInItsPanel() {
  return (
    <ChatsHere.Provider value={fixedChats(nothingKnown)}>
      <div
        {...panelRoot("side")}
        aria-label="Navigation"
        role="region"
        style={{ display: "flex", flexDirection: "column", inlineSize: 260, blockSize: 260 }}
      >
        <ChatsSection rows={UNDER} front={1} onOpen={() => {}} restarts={{ 2: RESTART }} />
      </div>
    </ChatsHere.Provider>
  );
}

/** The tree's row of `session`. */
const rowOf = (canvas: HTMLElement, session: number) =>
  canvas.querySelector<HTMLElement>(`[role="treeitem"][data-session="${session}"]`);

/**
 * **A row at its task limit, and its dispatch waiting on memory** (#1498, #1617, #3759): the
 * line under the row says which limit binds, with the link to Dispatch settings, and the next
 * says what waits on memory. Both are the pointer's and the eye's: the tree owns its rows alone
 * (axe's `aria-required-children`, with no rule turned off), so the row says the limit's whole
 * sentence, which names where it is changed, and what waits. Tab goes from the row out of the
 * list, past them. The picture is the two lines under the first row.
 */
export const AtItsTaskLimit: Story = {
  render: () => <SaidUnderInItsPanel />,
  play: async ({ canvasElement }) => {
    const row = rowOf(canvasElement, 1);
    await expect(row).not.toBeNull();
    await expect(row).toHaveAttribute(
      "aria-description",
      `${UNDER[0].atLimit?.said} 1 dispatch waits on memory. ${ON_MEMORY_SAID}`,
    );
    const link = canvasElement.querySelector<HTMLElement>(".at-limit-settings");
    await expect(link).not.toBeNull();
    await expect(link).toHaveTextContent("Dispatch settings");
    await expect(link).toHaveAttribute("tabindex", "-1");
    await expect((link as HTMLElement).closest('[aria-hidden="true"]')).not.toBeNull();

    // The keyboard's way: past the view's head, the row is the list's one stop, and Tab goes on
    // past its lines.
    for (let step = 0; step < 12 && document.activeElement !== row; step += 1) {
      await userEvent.tab();
    }
    await untilFocused(() => rowOf(canvasElement, 1), "the first row");
    expectFocusInSight();
    await userEvent.tab();
    await waitFor(() =>
      expect(canvasElement.querySelector(".chats-list")?.contains(document.activeElement)).toBe(
        false,
      ),
    );
  },
};

/**
 * **A row with a restart said** (#1462, #3759): a chat with no pane, whose restart was refused,
 * says why on a line under its row. The tree may own no status, so the words are announced from
 * beside it, named for the chat, and the row says them too. The picture is the line under the
 * second row.
 */
export const RestartSaid: Story = {
  render: () => <SaidUnderInItsPanel />,
  play: async ({ canvasElement }) => {
    const said = within(canvasElement).getByRole("status", { name: "Restarts" });
    await expect(said).toHaveTextContent(RESTART);
    await expect(said.closest('[role="tree"]')).toBeNull();
    await expect(rowOf(canvasElement, 2)).toHaveAttribute("aria-description", RESTART);
    const line = canvasElement.querySelector<HTMLElement>(".restart-said");
    await expect(line).toHaveTextContent(RESTART);
    await expect(line).toHaveAttribute("aria-hidden", "true");
  },
};

/**
 * **What a row says, in its card** (#3886): the keyboard rests on the row at its task limit,
 * and its card comes up with the limit's whole sentence and what waits on memory, with where
 * they are named. The card describes the row while it is up, so a screen reader is told them
 * there, and a sighted keyboard user reads them with no hover. Held to axe with the card up.
 */
export const SaidInItsCard: Story = {
  // The card stands over the canvas's edge, where a picture would be of the popover's place.
  parameters: { screenshot: false },
  render: () => <SaidUnderInItsPanel />,
  play: async ({ canvasElement }) => {
    const row = rowOf(canvasElement, 1);
    await expect(row).not.toBeNull();
    for (let step = 0; step < 12 && document.activeElement !== row; step += 1) {
      await userEvent.tab();
    }
    await untilFocused(() => rowOf(canvasElement, 1), "the first row");
    const card = await waitFor(
      () => {
        const up = document.querySelector<HTMLElement>('.chat-card [role="tooltip"]');
        if (up === null) throw new Error("no card is up");
        return up;
      },
      { timeout: 3000 },
    );
    // Once it has arrived: the popover family fades it in from nothing (`data-entering`), and
    // axe, after the play, reads its words' contrast as they are drawn.
    await waitFor(
      () => {
        expect(card.closest("[data-entering]")).toBeNull();
        expect(card).toBeVisible();
      },
      { timeout: 3000 },
    );
    await expect(card).toHaveTextContent(UNDER[0].atLimit?.said ?? "");
    await expect(card).toHaveTextContent("1 dispatch waits on memory");
    await expect(card).toHaveTextContent(ON_MEMORY_SAID);
    await expect(row).toHaveAttribute("aria-describedby", card.id);
  },
};
