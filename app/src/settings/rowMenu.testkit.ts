// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A settings row's ⋯ menu, from a test** (#2919, R56–R59): on a settings page a row's Reset,
 * its moves between files and Copy key are rows of one menu at the row's end, opened by the
 * button "More for ‹label›".
 */
import { expect } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";

/** `userEvent` itself, or a session of it. */
type User = {
  click: (element: Element) => Promise<unknown>;
  keyboard: (text: string) => Promise<unknown>;
};

/** A menu row's words: its label's, without the line under it or its hidden description. */
export function wordsOf(row: Element): string {
  return (row.querySelector('[data-part="label"]') ?? row).textContent?.trim() ?? "";
}

/** Every row of an open menu, by its words. */
export function rowsOf(menu: HTMLElement): string[] {
  return [
    ...within(menu).queryAllByRole("menuitem"),
    ...within(menu).queryAllByRole("menuitemradio"),
  ]
    .sort((a, b) => (a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1))
    .map(wordsOf);
}

/** The row's ⋯ button, found in `inside` (the whole document unless said). */
export function moreFor(label: string, inside: HTMLElement = document.body): HTMLElement {
  return within(inside).getByRole("button", { name: `More for ${label}` });
}

/** Opens the row's ⋯ menu with a press, and answers the menu. */
export async function rowMenu(
  label: string,
  user: User = userEvent,
  inside?: HTMLElement,
): Promise<HTMLElement> {
  await user.click(moreFor(label, inside));
  return screen.findByRole("menu", { name: `More for ${label}` });
}

/** Opens the row's ⋯ menu and chooses its row named `item`. */
export async function chooseInRow(
  label: string,
  item: string | RegExp,
  user: User = userEvent,
  inside?: HTMLElement,
): Promise<void> {
  const menu = await rowMenu(label, user, inside);
  await user.click(within(menu).getByRole("menuitem", { name: item }));
}

/**
 * What the row's ⋯ menu offers, each row by its words, "(off)" after one out of reach; then
 * closes it with Escape. Empty for a row with no ⋯.
 */
export async function offeredInRow(
  label: string,
  user: User = userEvent,
  inside?: HTMLElement,
): Promise<string[]> {
  if (within(inside ?? document.body).queryByRole("button", { name: `More for ${label}` }) === null)
    return [];
  const menu = await rowMenu(label, user, inside);
  const offered = [
    ...within(menu).queryAllByRole("menuitem"),
    ...within(menu).queryAllByRole("menuitemradio"),
  ]
    .sort((a, b) => (a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1))
    .map((one) =>
      one.getAttribute("aria-disabled") === "true" ? `${wordsOf(one)} (off)` : wordsOf(one),
    );
  await user.keyboard("{Escape}");
  await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  return offered;
}

/** Opens the page's rarely used group `label` (R61), folded until then: its trigger reads
 *  "‹label› · ‹count›". */
export async function unfold(label: string, user: User = userEvent): Promise<void> {
  const fold = screen.getByRole("button", { name: new RegExp(`^${label} · \\d+$`) });
  if (fold.getAttribute("aria-expanded") !== "true") await user.click(fold);
}
