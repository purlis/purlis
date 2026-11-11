// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";

/**
 * **The project in front's workspaces, as the title bar has them** (#2916): its pins as chips,
 * and every workspace in the popover its ⌄ opens, the plane root first. The workspace strip that
 * drew them under the bar is gone; a spec reaches a workspace as a person does, through the ⌄,
 * each step a press from the page and each read one `browser.execute` (`reading.ts`).
 */

/** The ⌄ that opens every workspace. */
export const ALL_WORKSPACES = '[data-testid="title-bar"] [data-titlebar-all]';

/** The workspace popover's list, while it is open. */
export const LISTED = '[role="listbox"][aria-label^="Workspaces in "]';

/** A workspace as the popover lists it: its name (the root's is `Plane root`), and whether it is
 *  the one in front. */
export type Listed = { name: string; selected: boolean };

/** The chips' names, `current` on the one in front. */
export async function chipsSeen(): Promise<{ names: string[]; current: string | null }> {
  return browser.execute(() => {
    const chips = [
      ...document.querySelectorAll<HTMLElement>(
        '[data-testid="title-bar"] [data-scope="titlebar"][data-part="workspace"]:not([data-variant="root"])',
      ),
    ];
    const name = (chip: HTMLElement) =>
      chip.querySelector('[data-part="name"]')?.textContent?.trim() ?? "";
    const current = chips.find((chip) => chip.getAttribute("aria-current") === "true");
    return { names: chips.map(name), current: current === undefined ? null : name(current) };
  });
}

/**
 * **Whether `workspace`'s chip wears the working mark** (#3127), the chat mark's ring in its slot,
 * and the description a screen reader hears on the chip.
 */
export async function workingSeen(
  workspace: string,
): Promise<{ marked: boolean; description: string }> {
  return browser.execute((name: string) => {
    const chip = document.querySelector<HTMLElement>(
      `[data-testid="title-bar"] [data-scope="titlebar"][data-part="workspace"][data-chip-of="${CSS.escape(name)}"]`,
    );
    const marked =
      chip
        ?.closest('[data-part="slot"]')
        ?.querySelector('[data-scope="chat-mark"][data-variant="working"]') != null;
    const description = (chip?.getAttribute("aria-describedby") ?? "")
      .split(" ")
      .filter(Boolean)
      .map((id) => document.getElementById(id)?.textContent ?? "")
      .join(" ");
    return { marked, description };
  }, workspace);
}

/** Where the ⌄ stood at one step of {@link opened}: its list drawn; pressed by this step; ready
 *  for a press this step did not make; its popover open with no list drawn yet; behind a modal
 *  dialog, named; or not drawn at all. */
export type Chevron =
  | { at: "listed" }
  | { at: "pressed" }
  | { at: "ready" }
  | { at: "open" }
  | { at: "behind"; dialog: string }
  | { at: "undrawn" };

/**
 * **One step of opening the workspace popover, in the page** (#5035): the ⌄ is pressed only when
 * a person could press it, and only when `mayPress`. Never while its popover is open, for a
 * press of the ⌄ closes what it opened; never behind a modal dialog, which hides the ⌄ from the
 * operator and the screen reader (React Aria's `aria-hidden` on all outside it) and takes the
 * keyboard back from any popover opened under it, so that popover goes the moment it comes.
 * Self-contained, for `browser.execute`.
 */
export function pressedWhenReady(list: string, all: string, mayPress: boolean): Chevron {
  if (document.querySelector(list) !== null) return { at: "listed" };
  const chevron = document.querySelector<HTMLElement>(all);
  if (chevron === null) return { at: "undrawn" };
  const hidden = '[aria-hidden="true"], [inert]';
  if (chevron.closest(hidden) !== null) {
    const over = [
      ...document.querySelectorAll<HTMLElement>('[role="dialog"], [role="alertdialog"]'),
    ].find((one) => one.closest(hidden) === null);
    const named =
      over?.getAttribute("aria-label") ??
      (over?.getAttribute("aria-labelledby") ?? "")
        .split(" ")
        .map((id) => document.getElementById(id)?.textContent ?? "")
        .join(" ")
        .trim();
    return { at: "behind", dialog: named === "" ? "with no name" : named };
  }
  if (chevron.getAttribute("aria-expanded") === "true") return { at: "open" };
  if (!mayPress) return { at: "ready" };
  chevron.click();
  return { at: "pressed" };
}

/** What the last step of {@link opened} saw, said after "the title bar's ⌄ opened no workspaces". */
function said(last: Chevron, presses: number): string {
  switch (last.at) {
    case "behind":
      return `the ⌄ is behind the dialog "${last.dialog}", still up (a step before this one left it)`;
    case "undrawn":
      return "no ⌄ is drawn: the title bar has not read the project's workspaces";
    case "open":
      return "its popover is open and draws no list";
    default:
      return `pressed ${presses} times, and its popover went each time`;
  }
}

/**
 * Opens the workspace popover, if it is not open, and waits for its rows: once the ⌄ is ready
 * for a press, a press, and one more only if no popover came of it or it went at once (#5035).
 * The one more is for a dialog just gone: React Aria hands the keyboard back to the dialog's
 * opener a frame after it goes, and a popover opened in that frame loses the keyboard to it and
 * closes. Never a third: each press toggles, so a press at every step while the list was not yet
 * seen was a popover opened and lost, over and over, for the whole wait.
 */
async function opened(): Promise<void> {
  let last: Chevron = { at: "undrawn" };
  let presses = 0;
  await browser
    .waitUntil(
      async () => {
        last = await browser.execute(pressedWhenReady, LISTED, ALL_WORKSPACES, presses < 2);
        if (last.at === "pressed") presses += 1;
        return last.at === "listed";
      },
      { timeout: 20_000, interval: 200 },
    )
    .catch(() => {
      throw new Error(`the title bar's ⌄ opened no workspaces: ${said(last, presses)}`);
    });
}

/** Closes the workspace popover, by Escape in its filter, and waits until it has gone. */
export async function closed(): Promise<void> {
  await browser.execute((list: string) => {
    if (document.querySelector(list) === null) return;
    document
      .querySelector<HTMLElement>('[role="combobox"][aria-label^="Go to a workspace in "]')
      ?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  }, LISTED);
  await browser.waitUntil(
    async () => browser.execute((list: string) => document.querySelector(list) === null, LISTED),
    { timeout: 10_000, interval: 100, timeoutMsg: "the workspace popover did not close" },
  );
}

/**
 * **Every workspace of the project in front**, the plane root first, as the ⌄'s popover lists
 * them; the one in front is the one its keys start on. Opens the popover and closes it again.
 */
export async function workspacesListed(): Promise<Listed[]> {
  await opened();
  const rows = await browser.execute(
    (list: string) =>
      [...(document.querySelector(list)?.querySelectorAll<HTMLElement>('[role="option"]') ?? [])]
        .map((row) => ({
          name: row.querySelector("[data-titlebar-name]")?.textContent?.trim() ?? "",
          selected: row.getAttribute("aria-selected") === "true",
        }))
        // The last row makes one, and is no workspace.
        .filter((row) => row.name !== "New workspace…"),
    LISTED,
  );
  await closed();
  return rows;
}

/** Goes to the workspace listed `at` (the order {@link workspacesListed} reads), by its row. */
export async function goToWorkspaceAt(at: number): Promise<void> {
  await opened();
  await browser.execute(
    (list: string, index: number) => {
      const rows = document.querySelector(list)?.querySelectorAll<HTMLElement>('[role="option"]');
      rows?.item(index)?.click();
    },
    LISTED,
    at,
  );
}

/** The chip of the workspace in front, while it is not the plane root (whose house is no
 *  workspace's chip, #3654). */
export const CURRENT_CHIP =
  '[data-testid="title-bar"] [data-scope="titlebar"][data-part="workspace"][aria-current="true"]:not([data-variant="root"])';

/** Goes to the workspace called `name` (`Plane root` for the root), and waits until it is in
 *  front. */
export async function goToWorkspace(name: string): Promise<void> {
  await browser.waitUntil(
    async () => {
      const listed = await workspacesListed();
      const at = listed.findIndex((one) => one.name === name);
      if (at >= 0 && listed[at].selected) return true;
      if (at >= 0) await goToWorkspaceAt(at);
      return false;
    },
    { timeout: 30_000, interval: 250, timeoutMsg: `the title bar never brought ${name} forward` },
  );
}

/** Goes to the project's first workspace that is not the plane root, and answers its name. */
export async function goToAWorkspace(): Promise<string> {
  let name = "";
  await browser.waitUntil(
    async () => {
      name = (await workspacesListed()).find((one) => one.name !== "Plane root")?.name ?? "";
      return name !== "";
    },
    { timeout: 30_000, interval: 250, timeoutMsg: "the project listed no workspace" },
  );
  await goToWorkspace(name);
  return name;
}
