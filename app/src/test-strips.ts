// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { vi } from "vitest";
import { fireEvent, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";

/**
 * **A layout for jsdom, which lays nothing out**, for the tests that drag a tab (SI-6): a drag
 * is decided by where things are, and every box jsdom reports is empty.
 *
 * Every element is given a box by its place among its siblings — one tab a hundred pixels to
 * the right of the last — and a tablist one wide enough to hold them all, because `dnd-kit`
 * keeps a dragged tab inside its strip. Nothing else is faked: the sensor, the drop and what
 * the window then tells the core are the real ones.
 *
 * Undone by `vi.restoreAllMocks()`.
 */
export function laidOutInARow() {
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    const strip = this.hasAttribute("data-strip");
    // Among the cells only: a strip's tablist is an element of its own inside it (#1204).
    const at = this.parentElement
      ? [...this.parentElement.children]
          .filter((one) => one.getAttribute("role") !== "tablist")
          .indexOf(this)
      : 0;
    const left = strip ? 0 : at * 100;
    const width = strip ? 1000 : 90;
    return {
      x: left,
      y: 0,
      left,
      top: 0,
      right: left + width,
      bottom: 30,
      width,
      height: 30,
      toJSON: () => ({}),
    } as DOMRect;
  });
}

/**
 * {@link laidOutInARow}, down a column: the project rail's (#2915), whose buttons move up and down
 * it. Every element is given a box by its place among its siblings, one a hundred pixels below the
 * last, and a strip one tall enough to hold them all.
 */
export function laidOutInAColumn() {
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    const strip = this.hasAttribute("data-strip");
    const at = this.parentElement
      ? [...this.parentElement.children]
          .filter((one) => one.getAttribute("role") !== "tablist")
          .indexOf(this)
      : 0;
    const top = strip ? 0 : at * 100;
    const height = strip ? 1000 : 90;
    return {
      x: 0,
      y: top,
      left: 0,
      top,
      right: 40,
      bottom: top + height,
      width: 40,
      height,
      toJSON: () => ({}),
    } as DOMRect;
  });
}

/**
 * **The strip whose tablist is called `name`**: the element its tabs, their `×` and gears, and
 * its own controls are drawn in (#1204, `StripTablist.tsx`).
 *
 * Found through the tablist, by its role and name, and not by a class, so a test still says
 * which tablist it means. Not the tablist itself, which holds nothing: it owns its tabs through
 * `aria-owns`, which `within` does not follow.
 */
export function stripNamed(name: string): HTMLElement {
  return holding(screen.getByRole("tablist", { name }));
}

/**
 * **Closes the project `name` from the rail** (#2915): its button focused and Delete pressed, the
 * key that closes a focused project, as the rail has no `×`. Found once it is drawn.
 */
export async function closeFromTheRail(name: string): Promise<void> {
  const tab = await screen.findByRole("tab", { name });
  tab.focus();
  fireEvent.keyDown(tab, { key: "Delete" });
}

/** {@link stripNamed}, once the strip is drawn. */
export async function findStripNamed(name: string): Promise<HTMLElement> {
  return holding(await screen.findByRole("tablist", { name }));
}

/** {@link stripNamed}, or `null` when no tablist is called `name`. */
export function queryStripNamed(name: string): HTMLElement | null {
  const tablist = screen.queryByRole("tablist", { name });
  return tablist && holding(tablist);
}

/** The strip a tablist is in, and it must own exactly the tabs drawn there, in their order. */
function holding(tablist: HTMLElement): HTMLElement {
  const strip = tablist.closest<HTMLElement>("[data-strip]");
  if (!strip) throw new Error(`the tablist ${tablist.getAttribute("aria-label")} is in no strip`);
  const owned = (tablist.getAttribute("aria-owns") ?? "").split(/\s+/).filter(Boolean);
  const drawn = [...strip.querySelectorAll('[role="tab"]')].map((tab) => tab.id);
  if (owned.join(" ") !== drawn.join(" "))
    throw new Error(
      `the tablist owns [${owned.join(" ")}] but the strip draws [${drawn.join(" ")}]`,
    );
  return strip;
}

/** Picks up the focused tab with the keyboard, presses `arrow` once, and puts it down. */
export async function dragWithTheKeyboard(
  arrow: "{ArrowLeft}" | "{ArrowRight}" | "{ArrowUp}" | "{ArrowDown}",
) {
  await userEvent.keyboard("{Shift>}[Space]{/Shift}");
  await userEvent.keyboard(arrow);
  await userEvent.keyboard("[Space]");
}

/**
 * The window's status lines that are saying something.
 *
 * **Not every `role="status"`**: each strip's `DndContext` keeps a live region of that role for
 * what it announces during a drag, and it is empty until a tab is picked up. A test about what
 * charter says asks this rather than for the one status on the page, which there no longer is.
 */
export const sayingSomething = () =>
  screen.queryAllByRole("status").filter((one) => (one.textContent ?? "") !== "");

/**
 * **Opens the Inbox of the project in front** (#1695), as a person presses Notices on the
 * status line: the window's own lines are listed there while a project is in front (D-LB-1).
 */
export async function openTheInbox() {
  await userEvent.click(await screen.findByRole("button", { name: /^Notices: / }));
}

/**
 * **Opens the Explorer view of the left side** (#1673), as a person presses its icon on the
 * activity bar: the left side opens on Chats, and a test about the explorer's rows starts
 * where a person would.
 */
export async function showTheExplorer() {
  const tab = await screen.findByRole("tab", { name: "Explorer" });
  // A press of the open view puts the side away, so it is pressed only when it is not open.
  if (tab.getAttribute("aria-selected") !== "true") await userEvent.click(tab);
  return screen.findByRole("navigation", { name: "Explorer" });
}

/** Shows the Changes view (#1676): the repos' state, which the bottom region drew until then, in
 *  the Panel's left half since #1739. Returns the view's content. */
export async function showTheChanges() {
  const tab = await screen.findByRole("tab", { name: "Changes" });
  if (tab.getAttribute("aria-selected") !== "true") await userEvent.click(tab);
  return screen.findByTestId("changes-view");
}

/** Shows the Repos view (#1737), which shares the Panel's left half with Changes (#1739).
 *  Returns the view's content. */
export async function showTheRepos() {
  const tab = await screen.findByRole("tab", { name: "Repos" });
  if (tab.getAttribute("aria-selected") !== "true") await userEvent.click(tab);
  return screen.findByTestId("repos-view");
}

/**
 * **Opens a view of the right side and finds its panel** (#1678), as a person presses the view's
 * icon on the activity bar: the right sidebar opens on the Inbox and the Panel's right half on
 * Memory (#1738), and a test about another panel starts where a person would. `view` is the
 * tab's name (`Sessions`); the panel is found by its test id (`panel-sessions`) inside the view.
 *
 * **A view in the Panel's right half answers as its half** (Memory, Todos, Sessions): the line
 * heading the half holds the view's own buttons (P-11), so a test reaching for the heading's
 * `+` finds it where a person sees it. The half's other views are hidden, so a query by role
 * finds only the open one's.
 */
export async function showThePanel(view: string, testid = `panel-${view.toLowerCase()}`) {
  const lower = await screen.findByRole("tablist", { name: "Panel, right half" });
  const inHalf = within(lower).queryByRole("tab", { name: view }) !== null;
  const bar = inHalf ? lower : await screen.findByRole("tablist", { name: "Attention" });
  const tab = await within(bar).findByRole("tab", { name: view });
  // A press of the open view puts the side away, so it is pressed only when it is not open.
  if (tab.getAttribute("aria-selected") !== "true") fireEvent.click(tab);
  const shown = await screen.findByRole("tabpanel", { name: view });
  const panel = await within(shown).findByTestId(testid);
  return inHalf ? (shown.closest(".panel-half") as HTMLElement) : panel;
}
