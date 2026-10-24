// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState, type KeyboardEvent } from "react";
import type { Referenced } from "./references";

/**
 * **Several file and folder rows picked in the explorer** (#1151), to hand a chat at once as one
 * paste (`referencesMany.ts`). A row is picked with Cmd or Ctrl and a click (picked again, it
 * is let go), every row from the last one clicked with Shift and a click, and from the keyboard
 * with Shift and Up or Down. Esc lets them all go. A plain click does what it always did, and
 * marks where a Shift-click counts from.
 *
 * **One branch at a time**: one paste is one branch's, so a row of another branch starts the
 * picks again. A row purlis will not hand over (refused, or a branch's own *Files* row) is never
 * picked. What a row is, is written on it ({@link pickable}) by the tree that draws it, so a
 * range is read in the order the rows are drawn.
 */

/** A row as it can be picked: what a drag of it carries. */
export type Pickable = Omit<Referenced, "lines">;

/** The attribute a pickable row carries its reference in. */
const PICK = "data-pick";

/** What a row that can be picked carries: its reference, written on it. Nothing for a row
 *  purlis will not hand over. */
export function pickable(r: Pickable | undefined): Record<string, string> {
  return r === undefined || r.path === "" ? {} : { [PICK]: JSON.stringify(r) };
}

/** The reference a row carries, read back. */
function readPick(el: Element): Pickable | undefined {
  const raw = el.getAttribute(PICK);
  if (raw === null) return undefined;
  try {
    return JSON.parse(raw) as Pickable;
  } catch {
    return undefined;
  }
}

const branchOf = (r: Pickable) => JSON.stringify([r.plane, r.workspace, r.repo, r.piece]);
const same = (a: Pickable, b: Pickable) =>
  branchOf(a) === branchOf(b) && a.path === b.path && a.folder === b.folder;

/** The keys held as a row is pressed, and the row. */
export interface Pressed {
  shiftKey: boolean;
  metaKey: boolean;
  ctrlKey: boolean;
  target: Element;
}

/** The pickable rows of the tree `el` is in, in the order they are drawn. */
function rowsAround(el: Element): { el: Element; r: Pickable }[] {
  const tree = el.closest('[role="treegrid"]');
  if (tree === null) return [];
  return [...tree.querySelectorAll(`[${PICK}]`)].flatMap((one) => {
    const r = readPick(one);
    return r === undefined ? [] : [{ el: one, r }];
  });
}

/** Every row of `to`'s branch from `from` to `to`, as drawn; `to` alone where `from` is not
 *  drawn there. */
function range(at: Element, from: Pickable | undefined, to: Pickable): Pickable[] {
  const rows = rowsAround(at)
    .map((one) => one.r)
    .filter((r) => branchOf(r) === branchOf(to));
  const end = rows.findIndex((r) => same(r, to));
  const start = from === undefined ? -1 : rows.findIndex((r) => same(r, from));
  if (end < 0) return [to];
  if (start < 0) return [rows[end]];
  return rows.slice(Math.min(start, end), Math.max(start, end) + 1);
}

/** The picks, and how the tree's rows and keys change them. `at` names what the rows are of
 *  now: the picks are let go when it changes. */
export function useFilePicks(at: string) {
  const [state, setState] = useState<{ at: string; picks: Pickable[]; anchor?: Pickable }>({
    at,
    picks: [],
  });
  // Derived while rendering, as React has state follow a prop: other rows, no picks.
  const now = state.at === at ? state : { at, picks: [] };
  if (now !== state) setState(now);
  const { picks, anchor } = now;

  /** Whether row `r` is picked. */
  const isPicked = (r: Pickable) => picks.some((one) => same(one, r));

  /**
   * A press on a pickable row: true where it picked (Cmd, Ctrl or Shift held), and the row's
   * own act is not done; false for a plain press, which only marks where a range counts from.
   */
  const press = (event: Pressed, r: Pickable | undefined): boolean => {
    if (r === undefined) return event.metaKey || event.ctrlKey || event.shiftKey;
    if (event.shiftKey) {
      const from = anchor !== undefined && branchOf(anchor) === branchOf(r) ? anchor : undefined;
      setState({ at, picks: range(event.target, from, r), anchor: from ?? r });
      return true;
    }
    if (event.metaKey || event.ctrlKey) {
      const mine = picks.length > 0 && branchOf(picks[0]) === branchOf(r) ? picks : [];
      const next = mine.some((one) => same(one, r))
        ? mine.filter((one) => !same(one, r))
        : [...mine, r];
      setState({ at, picks: next, anchor: r });
      return true;
    }
    setState({ at, picks: [], anchor: r });
    return false;
  };

  /**
   * A key on a row of the tree: Shift and Up or Down moves to the next row drawn and picks every
   * row from where it started; Esc lets the picks go. True where it was this one's.
   */
  const key = (event: KeyboardEvent<HTMLElement>): boolean => {
    if (event.key === "Escape" && picks.length > 0) {
      setState({ at, picks: [] });
      return true;
    }
    if (!event.shiftKey || event.altKey || event.ctrlKey || event.metaKey) return false;
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return false;
    const from = event.target as Element;
    const rows = rowsAround(from);
    const here = rows.findIndex((one) => one.el === from);
    const to = rows[here + (event.key === "ArrowDown" ? 1 : -1)];
    if (here < 0 || to === undefined) return here >= 0;
    const start =
      anchor !== undefined && branchOf(anchor) === branchOf(to.r) && picks.length > 0
        ? anchor
        : rows[here].r;
    setState({ at, picks: range(to.el, start, to.r), anchor: start });
    (to.el as HTMLElement).focus();
    return true;
  };

  /** The picks in the order the rows are drawn, from the tree `inside` holds. */
  const inOrder = (inside: Element | null): Pickable[] => {
    if (inside === null) return picks;
    const drawn = [...inside.querySelectorAll(`[${PICK}]`)].flatMap((el) => {
      const r = readPick(el);
      return r !== undefined && isPicked(r) ? [r] : [];
    });
    return drawn.length === picks.length ? drawn : picks;
  };

  const clear = () => setState({ at, picks: [] });

  return { picks, isPicked, press, key, inOrder, clear };
}
