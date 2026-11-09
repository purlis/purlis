// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { expect } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";

/**
 * **A tree of the tree family, as a test reads it** (#2911, #3198): React Aria draws every tree
 * as a `treegrid` of `row`s (D-2911-T-1), so a test asks for the tree and its rows by those
 * roles, here, and never by the markup that draws them.
 */
const TREE = "treegrid";
const ROW = "row";

/** The tree named `name`. */
export function treeNamed(name: string | RegExp): HTMLElement {
  return screen.getByRole(TREE, { name });
}

/** The tree named `name`, once it is drawn. */
export function findTree(name: string | RegExp): Promise<HTMLElement> {
  return screen.findByRole(TREE, { name });
}

/** The tree named `name`, or none. */
export function queryTree(name: string | RegExp): HTMLElement | null {
  return screen.queryByRole(TREE, { name });
}

/**
 * Every row of `tree` drawn now, in their order, at every level: its items, which say their
 * level, and not a group's name, which React Aria draws as a row of its own (`TreeSection`).
 */
export function rowsOf(tree: HTMLElement): HTMLElement[] {
  return within(tree)
    .queryAllByRole(ROW)
    .filter((row) => row.hasAttribute("aria-level"));
}

/** The row of `tree` named `name` (its `textValue`, or its `aria-label`). */
export function rowNamed(tree: HTMLElement, name: string | RegExp): HTMLElement {
  return within(tree).getByRole(ROW, { name });
}

/** The row of `tree` named `name`, once it is drawn. */
export function findRow(tree: HTMLElement, name: string | RegExp): Promise<HTMLElement> {
  return within(tree).findByRole(ROW, { name });
}

/** The row of `tree` named `name`, or none. */
export function queryRow(tree: HTMLElement, name: string | RegExp): HTMLElement | null {
  return within(tree).queryByRole(ROW, { name });
}

/** The names of `tree`'s rows, in their order. */
export function rowNames(tree: HTMLElement): string[] {
  return rowsOf(tree).map((row) => row.getAttribute("aria-label") ?? row.textContent ?? "");
}

/** Waits until `tree` draws `count` rows. */
export async function waitForRows(tree: HTMLElement, count: number): Promise<void> {
  await waitFor(() => expect(rowsOf(tree)).toHaveLength(count));
}

/** The row an element is in, or none: a row's own button, mark or words. */
export function rowAround(element: Element | null | undefined): HTMLElement | null {
  return element?.closest<HTMLElement>(`[role="${ROW}"]`) ?? null;
}

/**
 * **What a reader hears of a row**: its name, then its description (`aria-description`, and the
 * words of what `aria-describedby` names), as one line.
 */
export function heard(row: HTMLElement): string {
  const named = row.getAttribute("aria-label") ?? row.textContent ?? "";
  const described = (row.getAttribute("aria-describedby") ?? "").split(/\s+/).flatMap((id) => {
    const by = id === "" ? null : document.getElementById(id);
    return by === null ? [] : [by.textContent ?? ""];
  });
  return [named, row.getAttribute("aria-description") ?? "", ...described]
    .map((one) => one.trim())
    .filter((one) => one !== "")
    .join(" ");
}

/**
 * **Presses `keys` with the keyboard on `row`**, as a person does: the row has the keyboard
 * first, since a tree hears a row's keys only on the row it has focused (`{ArrowLeft}` folds it).
 */
export async function keysOn(row: HTMLElement, keys: string): Promise<void> {
  act(() => row.focus());
  await userEvent.keyboard(keys);
}
