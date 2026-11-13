// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act, screen, waitFor } from "@testing-library/react";
import { expect, vi } from "vitest";
import { UNDO_MS } from "./undo";
import { forgetThisLaunch } from "./regions";
import { GLOBAL } from "./windowprefs";
import type { PaneNoticesShown } from "./paneNotices";

/**
 * **A window whose Panel's right half opens on Notices**, for the window tests of a project's
 * Notices: they stood under the tab strip, always on screen, since #1695 they were listed in
 * the Inbox, and since #5645 in the Notices view, which a person opens under Asks. The layout
 * file the window is handed at its creation names Notices as the half's view, so what the
 * person would see is what the test can press.
 */
export function inboxOpenAtLaunch(): void {
  asideOpenAtLaunch("asks", "expanded", "notices");
}

/**
 * **A window whose right side opens on Personas, the Inbox behind it**: for the window tests of
 * a pane's own Notices, which the Inbox lists too. The side opens on the Inbox since #1738, and a
 * Notice on a pane and its line in the Inbox are two of the same name on screen.
 */
export function inboxAwayAtLaunch(notices?: unknown): void {
  // Passed as a hook (`beforeEach(inboxAwayAtLaunch)`), its argument is the test's context.
  asideOpenAtLaunch("personas", notices === "minimised" ? "minimised" : "expanded");
}

/**
 * The layout file a test's window is handed. **Its panes' notices are expanded** (#4754), so a
 * test of a note on a pane reads the note; a test of the fold hands the file without them, as a
 * new machine's has (`minimised`, the window's default, which the file leaves out).
 */
function asideOpenAtLaunch(
  view: "asks" | "personas",
  notices: PaneNoticesShown = "expanded",
  half?: "notices",
): void {
  forgetThisLaunch();
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: {
      path: "/home/op/.config/purlis/layout.json",
      found: true,
      document: {
        version: 2,
        regions: [
          { id: "navigation", side: "left", order: 0, collapsed: false },
          { id: "aside", side: "right", order: 0, collapsed: false, view },
          ...(half === undefined
            ? []
            : [{ id: "panel-right", side: "right", order: 0, collapsed: false, view: half }]),
        ],
        ...(notices === "expanded" ? { notices } : {}),
      },
      trouble: null,
    },
    theme: { path: "", found: false, document: null, trouble: null },
  };
}

/** Takes back what {@link inboxOpenAtLaunch} handed the next window. */
export function forgetInboxOpen(): void {
  Reflect.deleteProperty(globalThis, GLOBAL);
  forgetThisLaunch();
}

/**
 * **Waits out the Undo of a Dismiss** (#5650): a line dismissed in Updates or Notices goes at
 * once, and its own Dismiss is carried out (kept on this machine) only when its Undo has stood
 * for {@link UNDO_MS}. Needs the test's timers faked, `setTimeout` among them.
 */
export function waitOutTheUndo(): void {
  act(() => {
    vi.advanceTimersByTime(UNDO_MS + 1);
  });
}

/**
 * **Opens every ask's Answer** (#5651), for the tests of what an ask's form does: its answers,
 * its reply box, its Notice. Since #5651 an ask's form is drawn only once its Answer link is
 * pressed; these tests are about the form, not the link, so each is opened as the person would
 * open it, and the keyboard the opening moved into the form is handed back.
 */
export function openEveryAnswer(): void {
  const closed = [
    ...document.querySelectorAll<HTMLElement>('.inbox-ask button[aria-expanded="false"]'),
  ].filter((one) => one.textContent === "Answer");
  if (closed.length === 0) return;
  act(() => {
    for (const one of closed) one.click();
  });
  act(() => {
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
  });
}

/**
 * **A chat's group in the Inbox's Asks** (#3123): since Asks is the list family's grid, a group of
 * rows under its chain (a `rowgroup` named by its heading), where it was a `region` of the page.
 * The chat's chain names it, as before; a chat elsewhere's group is named by the chat alone,
 * where it lives being said in its row.
 */
export const chatGroup = (name: string | RegExp): HTMLElement =>
  screen.getByRole("rowgroup", { name });

/** The same, or none where no group has that name. */
export const chatGroupOrNone = (name: string | RegExp): HTMLElement | null =>
  screen.queryByRole("rowgroup", { name });

/** **An ask's row, or an update's** (#3123): a row of the list family's grid, named as it was. */
export const inboxRow = (name: string | RegExp): HTMLElement => screen.getByRole("row", { name });

/** Every ask's row in the Inbox drawn now, in its order. */
export const askRows = (): HTMLElement[] => [
  ...document.querySelectorAll<HTMLElement>('[role="row"].inbox-ask'),
];

/** Every update's row in the Inbox drawn now, in its order. */
export const updateRows = (): HTMLElement[] => [
  ...document.querySelectorAll<HTMLElement>('[role="row"].inbox-update'),
];

/** The rows of a list of the Inbox's under `within`, in their order: never a group's name, which
 *  React Aria draws as a row of its own. */
export const rowsIn = (within: Element): HTMLElement[] => [
  ...within.querySelectorAll<HTMLElement>('[role="row"][data-key]'),
];

/**
 * **What the Inbox lists, by name, in its order** (#3123): each chat's group by its chain (the
 * name of its rows, a `rowheader`), and each section of the Inbox's own by its title (Notices,
 * Updates, a heading).
 */
export const groupNames = (): string[] =>
  [...document.querySelectorAll<HTMLElement>('[role="rowheader"], h3')].map(
    (one) => one.textContent ?? "",
  );

/**
 * **Asks widened to where the ask waits** (ADR 0121, amended 2026-11-12, decision 4): Asks opens
 * on Chat, and an ask of a chat that is not the focused one's is offered by the empty scope's way
 * out, "N waiting in this workspace", which nothing presses by itself. For a test whose ask is
 * not of the focused chat: it presses the way out, as the person does, until a scope lists it.
 */
export async function widenAsks(panel?: ParentNode): Promise<void> {
  const asks = panel ?? (await screen.findByRole("tabpanel", { name: "Asks" }));
  const wayOut = () =>
    [...asks.querySelectorAll<HTMLButtonElement>(".inbox-scope-empty button")].find((one) =>
      / waiting in this (workspace|project)$/.test(one.textContent ?? ""),
    );
  for (let presses = 0; presses < 2; presses += 1) {
    // Until the asks are read: a row listed, or the way out offered.
    await waitFor(() =>
      expect(asks.querySelector('[role="row"].inbox-ask') ?? wayOut() ?? null).not.toBeNull(),
    );
    const out = wayOut();
    if (out === undefined) return;
    act(() => out.click());
  }
}
