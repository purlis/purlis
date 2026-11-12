// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { LEAST, leastAt } from "./fits";
import { DEFAULT_TEXT } from "./textSize";
import type { LaidOut, Moved, OpenChat } from "./bindings";
import { stripNamed } from "./test-strips";
import { aTabForEachChat } from "./laidOut.testkit";
import { hoverTipOf } from "./ui/tooltip/tooltip.testkit";
import { measuring, type Room } from "./stripRoom.testkit";
import { aBatchOf } from "./chatEvents.testkit";
import { CHAT_EVENTS } from "./chatEvents";

/**
 * The chat strip when it holds more than it has room for (ADR 0039, as amended).
 *
 * Two rules, and they are opposites on purpose:
 *
 * - **the strip's order never changes**, because a tab that moves under the cursor breaks
 *   aiming, and
 * - **the show-more menu is sorted by last activity**, because it is a list you read rather
 *   than a surface you aim at.
 *
 * And one that replaces the scroller: **what the strip has no room for is not drawn**, so the
 * menu is the pointer route to it. The record's own constraint — charter-app#130's fifty tabs
 * with no way to reach the last of them — is what "reaches a hidden tab, and closes it" below
 * is for, and it is the test that would go red if the collapse made a tab unreachable.
 *
 * **jsdom lays nothing out** (#149), so what is measured is stubbed here rather than pretended
 * at: `clientWidth` answers what a test set, and a `ResizeObserver` of this file's own tells
 * the app when it changed. What the real browser answers is `panes.e2e.ts`'s question, and it
 * is the only place it can be asked.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

/** A chat the core says it has open, with everything not under test left plain. */
function chat(session: number): OpenChat {
  return {
    session,
    name: `ide.${session}`,
    cwd: "/home/dev/plane/workspaces/ide",
    // No harness and no persona, so its tab is its own name alone and the assertions below read
    // the names they gave it (the default before the name is charter-app#254's, tested there).
    harness: null,
    in_front: session === 1,
    resumed: null,
    fresh: null,
    profile: null,
    persona: null,
    unreported: null,
    card: null,
    guessed: null,
    pinned: false,
    label: null,
    from: null,
  };
}

/** A layout with a tab for each of `open`, pinned as its chat is. */
function eachInATab(open: OpenChat[]): LaidOut {
  const laid = aTabForEachChat(open);
  return {
    ...laid,
    tabs: laid.tabs.map((tab, at) => ({ ...tab, pinned: open[at]?.pinned ?? false })),
  };
}

/** The core, holding `open` chats and nothing else. There is no sidebar, so every chat is on
 *  one strip — which is what a window that has not read the plane yet draws (ADR 0036). The
 *  layout stored gives each chat a tab, pinned where the chat is (#4758): a chat gets a tab
 *  only from the layout now, and these tests are about the strip, not about how it got its
 *  tabs. */
function core(
  open: OpenChat[],
  laid: LaidOut | null = eachInATab(open),
): { move: (moved: Moved) => void; ended: number[] } {
  const listeners = new Map<string, number>();
  /** Every listener for the chat events: each chat store, and the asks, listen for them. */
  const batches: number[] = [];
  const ended: number[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "plugin:event|listen") {
      const { event, handler } = args as { event: string; handler: number };
      listeners.set(event, handler);
      if (event === CHAT_EVENTS) batches.push(handler);
      return 1;
    }
    if (cmd === "plane_at_launch")
      return { plane: "/home/dev/plane", from: "/home/dev/plane", why: null };
    if (cmd === "opened_chats") return open;
    if (cmd === "reopened_layout") return laid;
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "close_session") {
      ended.push((args as { session: number }).session);
      return null;
    }
    return null;
  });
  return {
    ended,
    move: (moved: Moved) => {
      if (batches.length === 0) throw new Error("the window is not listening for moves");
      for (const handler of batches)
        window.__TAURI_INTERNALS__.runCallback(handler, {
          event: CHAT_EVENTS,
          id: 1,
          payload: aBatchOf("moved", moved),
        });
    },
  };
}

declare global {
  interface Window {
    __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
  }
}

/** One move, as the core pushes it. `at` is the board's count — bigger is more recent. */
function moving(session: number, at: number): Moved {
  return {
    plane: "/home/dev/plane",
    session,
    state: "waiting",
    needs_you: false,
    queue: [],
    moved_at: at,
    // The board numbers a snapshot at least as late as the move it reports.
    sequence: at,
    reports: [],
    refusals: [],
    children: [],
  };
}

const strip = () => stripNamed("Tabs");
const tabNames = () =>
  within(strip())
    .getAllByRole("tab")
    .map((tab) => tab.querySelector(".tab-name")?.textContent);

/** What the show-more menu lists, top to bottom. */
const menuNames = () =>
  screen.getAllByRole("menuitem").map((row) => row.querySelector(".tab-name")?.textContent);

/** Every close button the strip is drawing, left to right. */
/** The tabs' closes: each closes its tab and ends nothing (#4756), so nothing is asked. */
const closers = () => within(strip()).queryAllByRole("button", { name: /^Close / });

describe("the chat strip when it holds more than it has room for", () => {
  let room: Room;

  beforeEach(() => {
    room = measuring();
  });

  afterEach(() => {
    room.stop();
    cleanup();
    clearMocks();
  });

  /** Four chats open, with the first in front, and nothing measured yet. */
  async function fourChats() {
    const made = core([chat(1), chat(2), chat(3), chat(4)]);
    render(<App />);
    await vi.waitFor(() => expect(tabNames()).toEqual(["ide.1", "ide.2", "ide.3", "ide.4"]));
    return made;
  }

  it("draws every tab while nobody has said how wide the strip is", async () => {
    // A width of zero is the absence of a measurement, not a strip with no room — and every
    // environment with no layout answers zero. Drawing nothing there would be a window with
    // no tabs in it.
    await fourChats();

    expect(tabNames()).toEqual(["ide.1", "ide.2", "ide.3", "ide.4"]);
    expect(screen.queryByRole("button", { name: /the strip is not showing/ })).toBeNull();
  });

  it("says nothing while every tab fits", async () => {
    await fourChats();

    room.roomFor(4);

    // No button, not a disabled one: the whole point is that it is the first thing on the
    // strip that SAYS there are more, so it must not be there when there are not.
    await vi.waitFor(() => expect(tabNames()).toHaveLength(4));
    expect(screen.queryByRole("button", { name: /the strip is not showing/ })).toBeNull();
  });

  it("draws only what fits, and says how many it did not", async () => {
    await fourChats();

    room.roomFor(2);

    const more = await screen.findByRole("button", {
      name: "Show 2 tabs the strip is not showing",
    });
    expect(more).toHaveTextContent("2 more");
    expect(tabNames()).toEqual(["ide.1", "ide.2"]);
  });

  it("counts one hidden tab in the singular", async () => {
    await fourChats();

    room.roomFor(3);

    expect(
      await screen.findByRole("button", { name: "Show 1 tab the strip is not showing" }),
    ).toBeInTheDocument();
  });

  it("lists the hidden tabs most recently moved first", async () => {
    const { move } = await fourChats();
    // Chat 3 moved, then 4, then 2. The strip holds them in the order they opened.
    move(moving(3, 10));
    move(moving(4, 11));
    move(moving(2, 12));

    room.roomFor(1);
    await userEvent.click(
      await screen.findByRole("button", { name: "Show 3 tabs the strip is not showing" }),
    );

    expect(menuNames()).toEqual(["ide.2", "ide.4", "ide.3"]);
  });

  it("leaves the strip in its own order while the menu re-sorts", async () => {
    // **The two rules at once, which is the whole record.** The same activity that puts a
    // chat at the top of the menu must not move its tab: an operator going back to the chat
    // that was third from the left goes there with their hand, not by reading.
    const { move } = await fourChats();

    move(moving(4, 9));
    move(moving(3, 10));
    room.roomFor(2);
    await userEvent.click(
      await screen.findByRole("button", { name: "Show 2 tabs the strip is not showing" }),
    );

    expect(menuNames()).toEqual(["ide.3", "ide.4"]);
    expect(tabNames()).toEqual(["ide.1", "ide.2"]);
  });

  it("keeps the strip's order in the menu for the chats nothing has been heard about", async () => {
    // At a launch nothing has moved, so every tab ties. A menu whose rows moved between two
    // openings for no reason the operator can see is the aiming defect in the one surface
    // that was allowed to sort.
    await fourChats();

    room.roomFor(1);
    await userEvent.click(
      await screen.findByRole("button", { name: "Show 3 tabs the strip is not showing" }),
    );

    expect(menuNames()).toEqual(["ide.2", "ide.3", "ide.4"]);
  });

  it("draws a tab picked from the menu, with its own close button", async () => {
    // **This is what the collapse owes**, and what the scroller used to owe with
    // `scrollIntoView`: a tab reached through the menu arrives ON the strip, with its close.
    await fourChats();
    room.roomFor(1);
    await userEvent.click(
      await screen.findByRole("button", { name: "Show 3 tabs the strip is not showing" }),
    );

    await userEvent.click(screen.getByRole("menuitem", { name: /ide\.3/ }));

    await vi.waitFor(() => expect(screen.getAllByTestId("pane")[0]).toHaveTextContent("session 3"));
    expect(tabNames()).toEqual(["ide.3"]);
    expect(within(strip()).getByRole("button", { name: "Close ide.3" })).toBeInTheDocument();
  });

  it("reaches and closes every tab with room for only two of four, ending no chat", async () => {
    // **The fifty-tab question, at four.** Pressing the last close button on the strip over and
    // over reaches every tab only because closing a drawn tab gives the strip room for a hidden
    // one. A close ends nothing (#4756), and the strip's last tab keeps one empty pane.
    const { ended } = await fourChats();
    room.roomFor(2);
    await vi.waitFor(() => expect(closers()).toHaveLength(2));

    for (let pressed = 0; pressed < 4; pressed++) {
      const drawn = closers();
      await userEvent.click(drawn[drawn.length - 1]);
    }

    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(ended).toEqual([]);
    expect(tabNames()).toEqual(["Empty tab"]);
    expect(screen.queryByRole("button", { name: /the strip is not showing/ })).toBeNull();
  });

  it("moves between its rows with the arrow keys", async () => {
    // **The first thing built under ADR 0037, so it is the test of it.** Under Radix the
    // picker's radio group did NOT follow the arrow keys under React 19 on its own — Radix
    // learned that an arrow was down from a `keydown` listener on `document`, and React's
    // delegated listeners sit below `document`, so focus had already moved by the time Radix
    // heard it (#137). A menu's roving focus was its own `onKeyDown`, on the content element.
    // Both are React Aria's now, each an `onKeyDown` on its own element (`useRadioGroup`, and
    // the menu's `useSelectableCollection`), and this is what says so for the menu rather than
    // an assumption that the two are alike.
    await fourChats();
    room.roomFor(1);
    await userEvent.click(
      await screen.findByRole("button", { name: "Show 3 tabs the strip is not showing" }),
    );

    await userEvent.keyboard("{ArrowDown}");

    expect(screen.getAllByRole("menuitem")[0]).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    expect(screen.getAllByRole("menuitem")[1]).toHaveFocus();
  });

  it("brings a tab forward when Enter is pressed on its row", async () => {
    await fourChats();
    room.roomFor(1);
    await userEvent.click(
      await screen.findByRole("button", { name: "Show 3 tabs the strip is not showing" }),
    );

    await userEvent.keyboard("{ArrowDown}{ArrowDown}{Enter}");

    // The second row: the strip's order, since nothing has moved.
    await vi.waitFor(() => expect(screen.getAllByTestId("pane")[0]).toHaveTextContent("session 3"));
  });

  it("offers nothing that ends a chat", async () => {
    // A menu that pops up under the cursor with `End chat` in it is charter-app#130's defect
    // with a mouse attached. Every row here brings a tab forward and nothing else.
    await fourChats();
    room.roomFor(1);
    await userEvent.click(
      await screen.findByRole("button", { name: "Show 3 tabs the strip is not showing" }),
    );

    for (const row of screen.getAllByRole("menuitem")) {
      expect(row.textContent ?? "").not.toContain("End chat");
    }
  });

  it("draws New tab right after the last tab, and never collapses it or lists it", async () => {
    // Chrome's `+` (DS-7, #1839): inside the strip, after its last tab. Measured in
    // charter-app#130: `panes.e2e` could not press New tab partway through opening fifty
    // chats, because the strip scrolled it away. The strip no longer scrolls, and the `+` is
    // not one of the things `fitting` hides: it stays drawn when the strip has room for one
    // tab, and it is never a row of the show-more menu.
    await fourChats();

    room.roomFor(1);

    const adding = await screen.findByRole("button", { name: "New tab" });
    expect(strip()).toContainElement(adding);
    const tabs = within(strip()).getAllByRole("tab");
    const last = tabs[tabs.length - 1];
    expect(last.compareDocumentPosition(adding) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(adding.closest(".more")).toBeNull();
    // Not a tab, and not one the tablist owns (#1204).
    expect(adding).not.toHaveAttribute("role", "tab");
    await userEvent.click(
      await screen.findByRole("button", { name: "Show 3 tabs the strip is not showing" }),
    );
    expect(screen.queryByRole("menuitem", { name: /New tab/ })).toBeNull();
  });

  it("takes the `+` off the room the tabs share", async () => {
    // The `+` is inside the strip, so `useRoom` is told its width (`controls`): a strip with
    // room for three tabs and a `+` one tab wide draws two.
    const was = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetWidth");
    Object.defineProperty(HTMLElement.prototype, "offsetWidth", {
      configurable: true,
      get(this: HTMLElement) {
        return this.matches('[data-strip="Tabs"] > .strip-doing')
          ? leastAt(LEAST.chat, DEFAULT_TEXT.window)
          : 0;
      },
    });
    try {
      await fourChats();

      room.roomFor(3);

      await vi.waitFor(() => expect(tabNames()).toEqual(["ide.1", "ide.2"]));
      expect(
        await screen.findByRole("button", { name: "Show 2 tabs the strip is not showing" }),
      ).toBeInTheDocument();
    } finally {
      if (was) Object.defineProperty(HTMLElement.prototype, "offsetWidth", was);
      else Reflect.deleteProperty(HTMLElement.prototype, "offsetWidth");
    }
  });

  it("drops a tab out of the menu when it closes", async () => {
    await fourChats();
    room.roomFor(1);
    await screen.findByRole("button", { name: "Show 3 tabs the strip is not showing" });

    await userEvent.click(closers()[0]);

    expect(
      await screen.findByRole("button", { name: "Show 2 tabs the strip is not showing" }),
    ).toBeInTheDocument();
  });

  it("does not leave an observer behind for every render", async () => {
    // One observer per strip, replaced when the element it watches changes. A hook that
    // rebuilt on every render would pile them up and measure the same strip many times over.
    await fourChats();
    expect(room.live()).toBe(1);

    room.roomFor(1);
    await userEvent.click(
      await screen.findByRole("button", { name: "Show 3 tabs the strip is not showing" }),
    );

    expect(room.live()).toBe(1);
  });
});

describe("the strip's tabs (#4762)", () => {
  let room: Room;

  beforeEach(() => {
    room = measuring();
  });

  afterEach(() => {
    room.stop();
    cleanup();
    clearMocks();
  });

  /** The tab named `name`: what its marks are drawn in. */
  const tabNamed = (name: string) =>
    within(strip())
      .getAllByRole("tab")
      .find((tab) => nameIn(tab) === name);

  it("is drawn with one tab, its New tab beside it", async () => {
    core([chat(1)]);
    render(<App />);

    await waitFor(() => expect(tabNames()).toEqual(["ide.1"]));
    expect(within(strip()).getByRole("button", { name: "New tab" })).toBeInTheDocument();
  });

  it("wears a split's first pane's name whatever is focused, its picture, and says its panes on hover (#5342)", async () => {
    // One tab, two panes side by side, the second focused, as the record put it back.
    core([chat(1), chat(2)], {
      strip: null,
      tabs: [
        {
          workspace: null,
          pinned: false,
          window: 0,
          front: true,
          focused: "b",
          tree: {
            kind: "split",
            direction: "row",
            share: 500,
            a: { kind: "chat", chat: 1 },
            b: { kind: "chat", chat: 2 },
          },
        },
      ],
    });
    render(<App />);

    await waitFor(() => expect(tabNames()).toEqual(["ide.1"]));
    const tab = tabNamed("ide.1");
    // Its picture of the layout in place of `+1`, which a screen reader hears as words.
    expect(tab?.querySelector('.tab-split-more [data-part="picture"]')).not.toBeNull();
    expect(tab?.querySelector(".tab-split-more")?.textContent).toBe(", split, 2 panes");
    // The chat it shows by its number first, as any chat tab's (#6212), then its panes.
    expect(await hoverTipOf(tab)).toBe("#1 · ide.1\n2 panes: ide.1 · ide.2");
  });

  it("draws Tile all at its right end only while the tab in front is split (#4765)", async () => {
    core([chat(1), chat(2)], {
      strip: null,
      tabs: [
        {
          workspace: null,
          pinned: false,
          window: 0,
          front: true,
          focused: "a",
          tree: {
            kind: "split",
            direction: "row",
            share: 500,
            a: { kind: "chat", chat: 1 },
            b: { kind: "chat", chat: 2 },
          },
        },
      ],
    });
    render(<App />);

    expect(await screen.findByRole("button", { name: "Tile all" })).toBeEnabled();
  });

  it("draws no Tile all for a tab of one pane, so it takes no room from the tabs (#4765)", async () => {
    core([chat(1)]);
    render(<App />);

    await waitFor(() => expect(tabNames()).toEqual(["ide.1"]));
    expect(screen.queryByRole("button", { name: "Tile all" })).toBeNull();
  });

  it("always draws the tab in front, in the last place that fits", async () => {
    // `chat()` puts chat 1 in front; here the core says 4 is.
    core([{ ...chat(1), in_front: false }, chat(2), chat(3), { ...chat(4), in_front: true }]);
    render(<App />);
    await waitFor(() => expect(tabNames()).toHaveLength(4));

    room.roomFor(2);

    await waitFor(() => expect(tabNames()).toEqual(["ide.1", "ide.4"]));
  });

  it("says its state in one word after its name, the mark on its icon being only seen", async () => {
    // The scenario specs read `tab.$(".tab-said")` for the state's word (`chat.state.e2e.ts`). It
    // is the row's word (#4935): a turn ended with nothing asked of the person is idle.
    const { move } = core([chat(1)]);
    render(<App />);
    await waitFor(() => expect(tabNames()).toEqual(["ide.1"]));

    move(moving(1, 5));

    const tab = within(strip()).getAllByRole("tab")[0];
    await waitFor(() =>
      expect(tab.querySelector(".tab-said")?.getAttribute("aria-label")).toBe("idle"),
    );
    expect(tab.querySelectorAll(".tab-said")).toHaveLength(1);
  });

  it("draws its pin before its icon", async () => {
    core([chat(1), { ...chat(2), pinned: true }]);
    render(<App />);

    await waitFor(() => expect(tabNames()).toHaveLength(2));
    const marks = [
      ...(tabNamed("ide.2")?.querySelectorAll(
        '.pinned, [data-scope="chat-mark"][data-part="root"]',
      ) ?? []),
    ];
    expect(marks.map((one) => one.classList[0])).toEqual(["pinned", "ds-chat-mark"]);
  });

  it("keeps a pinned tab out of the collapse, with the front tab beside it", async () => {
    // With no sidebar read every chat is on one strip in its own order (ADR 0036): the pin
    // keeps its place, and the room left draws the tab in front.
    core([chat(1), chat(2), chat(3), { ...chat(4), pinned: true }]);
    render(<App />);
    await waitFor(() => expect(tabNames()).toHaveLength(4));

    room.roomFor(2);

    await waitFor(() => expect(tabNames()).toEqual(["ide.1", "ide.4"]));
    expect(
      await screen.findByRole("button", { name: "Show 2 tabs the strip is not showing" }),
    ).toBeInTheDocument();
  });

  it("draws pins as their icons only when they alone would take over half the strip", async () => {
    core([chat(1), chat(2), { ...chat(3), pinned: true }, { ...chat(4), pinned: true }]);
    render(<App />);
    await waitFor(() => expect(tabNames()).toHaveLength(4));

    room.roomFor(4);
    await waitFor(() => expect(tabNames()).toHaveLength(4));
    expect(within(strip()).getAllByRole("tab").filter(iconOnly)).toEqual([]);

    room.roomFor(3);
    await waitFor(() =>
      expect(within(strip()).getAllByRole("tab").filter(iconOnly).map(nameIn)).toEqual([
        "ide.3",
        "ide.4",
      ]),
    );
  });

  it("is closed by Close other tabs from the palette, all but its pinned tabs", async () => {
    const { ended } = core([chat(1), { ...chat(2), pinned: true }, chat(3)]);
    render(<App />);
    await waitFor(() => expect(tabNames()).toHaveLength(3));

    await userEvent.keyboard("{F2}");
    await screen.findByRole("dialog", { name: "Command palette" });
    await userEvent.keyboard("Close other tabs");
    await userEvent.keyboard("{Enter}");

    // It ends nothing (#4756), so nothing is asked.
    await waitFor(() => expect(tabNames()).toEqual(["ide.1", "ide.2"]));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(ended).toEqual([]);
  });

  it("puts the front tab's × next in the keyboard's order, no other tab's, and Enter on it closes the tab (#5341)", async () => {
    // jsdom draws no CSS: that the × is seen at rest on the front tab alone, and takes no room
    // from the name, is the stylesheet's (`tabs.css`) and the tab's story's to measure.
    core([chat(1), chat(2)]);
    render(<App />);
    await waitFor(() => expect(tabNames()).toHaveLength(2));
    const tabs = within(strip()).getAllByRole("tab");
    const front = tabs.find((one) => one.getAttribute("aria-selected") === "true") as HTMLElement;
    const back = tabs.find((one) => one !== front) as HTMLElement;
    const closerOf = (tab: HTMLElement) =>
      tab.closest('[data-part="tab"]')?.querySelector<HTMLElement>('[data-part="closer"]');

    expect(closerOf(back)?.tabIndex).toBe(-1);
    front.focus();
    await userEvent.tab();

    expect(document.activeElement).toBe(closerOf(front));
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(tabNames()).toHaveLength(1));
  });
});

/** Whether `tab` is drawn as its icon alone. */
const iconOnly = (tab: Element) => tab.closest("[data-icon-only]") !== null;
/** The name a tab says. */
const nameIn = (tab: Element) => tab.querySelector(".tab-name")?.textContent;
