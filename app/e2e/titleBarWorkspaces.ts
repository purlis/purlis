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
 *  for a press this step did not make, and whether the popover of the last press came and went;
 *  its popover open with no list drawn yet; behind a modal overlay, by its role and name; or not
 *  drawn at all. */
export type Chevron =
  | { at: "listed" }
  | { at: "pressed" }
  | { at: "ready"; cameAndWent: boolean }
  | { at: "open" }
  | { at: "behind"; role: string; name: string }
  | { at: "undrawn" };

/**
 * **One step of opening the workspace popover, in the page** (#5035): the ⌄ is pressed only when
 * a person could press it, and only when `mayPress`. Never while its popover is open, for a
 * press of the ⌄ closes what it opened; never behind a modal overlay (a dialog, a menu), which
 * hides the ⌄ from the operator and the screen reader (React Aria's `aria-hidden` on all outside
 * it) and takes the keyboard back from any popover opened under it, so that popover goes the
 * moment it comes. A press watches for the list, so a later step can say whether the popover it
 * opened came and went or never came. Self-contained, for `browser.execute`.
 */
export function pressedWhenReady(list: string, all: string, mayPress: boolean): Chevron {
  const watched = window as Window & { e2eWorkspacesCame?: boolean };
  if (document.querySelector(list) !== null) return { at: "listed" };
  const chevron = document.querySelector<HTMLElement>(all);
  if (chevron === null) return { at: "undrawn" };
  const hidden = '[aria-hidden="true"], [inert]';
  if (chevron.closest(hidden) !== null) {
    const over = [
      ...document.querySelectorAll<HTMLElement>(
        '[role="dialog"], [role="alertdialog"], [role="menu"], [role="listbox"]',
      ),
    ].find((one) => one.closest(hidden) === null);
    const label = over?.getAttribute("aria-label") ?? "";
    const name =
      label !== ""
        ? label
        : (over?.getAttribute("aria-labelledby") ?? "")
            .split(/\s+/)
            .map((id) => document.getElementById(id)?.textContent?.trim() ?? "")
            .filter((part) => part !== "")
            .join(" ");
    return { at: "behind", role: over?.getAttribute("role") ?? "overlay", name };
  }
  if (chevron.getAttribute("aria-expanded") === "true") return { at: "open" };
  if (!mayPress) return { at: "ready", cameAndWent: watched.e2eWorkspacesCame === true };
  // React draws the popover after the press returns, so whether it came is seen as it comes.
  watched.e2eWorkspacesCame = false;
  const watch = new MutationObserver(() => {
    if (document.querySelector(list) === null) return;
    watched.e2eWorkspacesCame = true;
    watch.disconnect();
  });
  watch.observe(document.body, { childList: true, subtree: true });
  chevron.click();
  return { at: "pressed" };
}

/** Why {@link opened} found no list, from the last step it took, its presses and the last error
 *  a step met. */
function whyNotOpened(last: Chevron, presses: number, trouble: unknown): string {
  const times = presses === 1 ? "once" : presses === 2 ? "twice" : `${presses} times`;
  const said = (() => {
    switch (last.at) {
      case "behind":
        return `the ⌄ is behind the ${last.role} "${last.name || "with no name"}", still up when the wait ended`;
      case "undrawn":
        return "no ⌄ is drawn: the title bar has not read the project's workspaces";
      case "open":
        return "its popover is open and draws no list";
      case "ready":
        return last.cameAndWent
          ? `pressed ${times}, and its popover came and went`
          : `pressed ${times}, and no popover came`;
      default:
        return `pressed ${times}, and the wait ended before its popover drew a list`;
    }
  })();
  return trouble === undefined ? said : `${said}; a step failed with: ${String(trouble)}`;
}

/** How often {@link opened} presses the ⌄ before it says why no list came: a popover lost to the
 *  title bar drawing again at launch came back on the next press (main run 43423866685). */
const PRESSES = 3;

/**
 * Opens the workspace popover, if it is not open, and waits for its rows (#5035): once the ⌄ is
 * ready for a press, a press, and at most {@link PRESSES} in all, each only once the ⌄ is ready
 * again with nothing over it: a press the page did not take (a ⌄ drawn again as it was pressed),
 * or a popover that came and went with no overlay up, as when the title bar draws again just
 * after launch (a window just resized, the project's first read). A dialog or menu over the ⌄ is
 * never pressed through: {@link pressedWhenReady} reports it as `behind`, and the failure names
 * it. Each press toggles, and the old step, which pressed whenever it saw no list, opened and lost
 * the popover under a dialog left up, over and over, for the whole wait.
 */
async function opened(): Promise<void> {
  let last = { at: "undrawn" } as Chevron;
  let presses = 0;
  let trouble: unknown;
  const settled = await browser
    .waitUntil(
      async () => {
        try {
          last = await browser.execute(pressedWhenReady, LISTED, ALL_WORKSPACES, presses < 1);
        } catch (error) {
          trouble = error;
          return false;
        }
        if (last.at === "pressed") presses += 1;
        if (last.at === "ready" && presses > 0 && presses < PRESSES) {
          last = await browser.execute(pressedWhenReady, LISTED, ALL_WORKSPACES, true);
          if (last.at === "pressed") presses += 1;
        }
        // Listed; or pressed for as often as it may be, and nothing came of it.
        return last.at === "listed" || (last.at === "ready" && presses >= PRESSES);
      },
      { timeout: 20_000, interval: 200 },
    )
    .catch(() => false);
  if (settled && last.at === "listed") return;
  throw new Error(
    `the title bar's ⌄ opened no workspaces: ${whyNotOpened(last, presses, trouble)}`,
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
