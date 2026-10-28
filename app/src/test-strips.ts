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

/**
 * **The title bar's workspace chips** (#2916): each pin of the project in front, and the one in
 * front when it is not pinned (the dashed chip), in the bar's order. The plane root's chip is
 * not one (#3654): it is the house before them.
 */
export function workspaceChips(): HTMLElement[] {
  return [
    ...screen
      .getByTestId("title-bar")
      .querySelectorAll<HTMLElement>(
        '[data-scope="titlebar"][data-part="workspace"]:not([data-variant="root"])',
      ),
  ];
}

/** A chip's workspace name, without its count. */
export function chipName(chip: HTMLElement): string {
  return chip.querySelector('[data-part="name"]')?.textContent ?? "";
}

/** The workspace in front, by its chip in the title bar; `undefined` at the plane root. */
export function workspaceInFront(): string | undefined {
  const current = workspaceChips().find((chip) => chip.getAttribute("aria-current") === "true");
  return current === undefined ? undefined : chipName(current);
}

/**
 * **Opens the title bar's ⌄** (#2916) and answers its list of the project's workspaces: the
 * plane root first, then the pins, then the rest, then New workspace….
 */
export async function openAllWorkspaces(): Promise<HTMLElement> {
  const open = screen.queryByRole("listbox", { name: /^Workspaces in / });
  if (open) return open;
  const strip = await findTheWorkspaces();
  await userEvent.click(within(strip).getByRole("button", { name: "All workspaces" }));
  return screen.findByRole("listbox", { name: /^Workspaces in / }, { timeout: 5000 });
}

/** The names the title bar's ⌄ lists, the plane root's as `Plane root`; the popover is closed
 *  again by Escape in its filter, as a person closes it. */
export async function workspacesListed(): Promise<string[]> {
  const list = await openAllWorkspaces();
  const names = within(list)
    .getAllByRole("option")
    .map((row) => row.querySelector("[data-titlebar-name]")?.textContent ?? "")
    .filter((name) => name !== "New workspace…");
  const filter = screen.queryByRole("combobox", { name: /^Go to a workspace in / });
  if (filter) fireEvent.keyDown(filter, { key: "Escape" });
  return names;
}

/** **Goes to the workspace `name`** from the title bar's ⌄ (`Plane root` for the root). */
export async function goToWorkspace(name: string | RegExp): Promise<void> {
  const list = await openAllWorkspaces();
  const rows = within(list).getAllByRole("option");
  const row = rows.find((one) => {
    const said = one.querySelector("[data-titlebar-name]")?.textContent ?? "";
    return typeof name === "string" ? said === name : name.test(said);
  });
  if (row === undefined) throw new Error(`the ⌄ lists no workspace ${String(name)}`);
  await userEvent.click(row);
}

/** **The title bar's pinned workspaces**, once the project in front's plane is read (#2916):
 *  the strip of chips, which is drawn only then. */
export async function findTheWorkspaces(): Promise<HTMLElement> {
  // The window reads its plane first, which a loaded runner takes its time over.
  return screen.findByRole("toolbar", { name: "Pinned workspaces" }, { timeout: 5000 });
}

/** What waits in a chip or a row, as its count says it: ` , 2 waiting`, or nothing. */
const waitingSaid = (one: Element) => {
  const count = one.querySelector('[data-part="count"]')?.textContent;
  return count ? `, ${count} waiting` : "";
};

/** What the chip of `workspace` says (#2916): its name, and what waits in it (`alpha, 2
 *  waiting`), its count being its description; `null` where it has no chip. */
export function chipSays(workspace: string): string | null {
  const chip = workspaceChips().find((one) => chipName(one) === workspace);
  if (chip === undefined) return null;
  return `${chipName(chip)}${waitingSaid(chip)}`;
}

/** What the title bar's ⌄ says of `workspace`'s row: its name, and what waits in it. */
export async function rowSays(workspace: string): Promise<string | null> {
  const list = await openAllWorkspaces();
  const row = within(list)
    .getAllByRole("option")
    .find((one) => one.querySelector("[data-titlebar-name]")?.textContent === workspace);
  const said = row === undefined ? null : `${workspace}${waitingSaid(row)}`;
  const filter = screen.queryByRole("combobox", { name: /^Go to a workspace in / });
  if (filter) fireEvent.keyDown(filter, { key: "Escape" });
  return said;
}
