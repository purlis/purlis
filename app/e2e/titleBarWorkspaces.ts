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
const LISTED = '[role="listbox"][aria-label^="Workspaces in "]';

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

/** Opens the workspace popover, if it is not open, and waits for its rows. */
async function opened(): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute(
        (list: string, all: string) => {
          if (document.querySelector(list) !== null) return true;
          document.querySelector<HTMLElement>(all)?.click();
          return false;
        },
        LISTED,
        ALL_WORKSPACES,
      ),
    { timeout: 20_000, interval: 200, timeoutMsg: "the title bar's ⌄ opened no workspaces" },
  );
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
