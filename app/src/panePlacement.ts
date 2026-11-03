// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { recorded } from "./layoutUndo";
import {
  type DropContext,
  dropOn,
  heldPane,
  judgeDrop,
  type Place,
  type Size,
  type Source,
  type Target,
} from "./paneDrop";
import { contentsOf, type Tabs } from "./tabs";

/**
 * **Placement mode: the keyboard and single-press route for every drag** (#4764, WCAG 2.5.7), as
 * the prototype's verdict ruled (`prototype/pane-drag` variant A, resolved on #4583). Something
 * picked up (a pane by its grip or ⌘⇧M, a sidebar chat, a tab) shows five targets on every
 * other pane of the tab in front and a New tab target on the strip; arrows move inside a pane's
 * targets, Tab goes to the next pane, Enter drops and Esc puts it back.
 *
 * **There is one set of rules**: a target is a {@link Target} of `paneDrop.ts`, judged and
 * dropped by the same `judgeDrop` and `dropOn` the pointer's drag is, and recorded for the same
 * Undo. This module only says which targets there are, how the keys walk them, and what is said.
 */

/** A pane's five targets, in the order they are drawn and read: a cross, top to bottom. */
export const PLACES: readonly Place[] = ["top", "left", "middle", "right", "bottom"];

/** Where targets are drawn: on a pane of the tab in front, or on the strip. */
export type Group = { kind: "pane"; tab: number; pane: number } | { kind: "strip"; at: number };

/** The keys that walk the targets. `ShiftTab` is Tab with Shift held. */
export type Step = "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" | "Tab" | "ShiftTab";

/**
 * **Where targets are drawn while `source` is held**: every pane of the tab in front, in the
 * tree's reading order, but the pane that would move (`heldPane`); then the strip's New tab, at
 * its end. A tab brought forward while it is held is the tab whose panes are offered, as a
 * pointer held over a tab brings it forward.
 */
export function groupsOf(tabs: Tabs, source: Source): Group[] {
  const front = tabs.inFront;
  const held = heldPane(tabs, source);
  const panes: Group[] =
    front === undefined
      ? []
      : contentsOf(tabs, front)
          .filter(({ pane }) => !(held !== undefined && held.tab === front && held.pane === pane))
          .map(({ pane }) => ({ kind: "pane", tab: front, pane }));
  return [...panes, { kind: "strip", at: tabs.order.length }];
}

/** The targets a group draws: a pane's five, or the strip's one. */
export function targetsOf(group: Group): Target[] {
  if (group.kind === "strip") return [group];
  return PLACES.map((place) => ({ kind: "pane", tab: group.tab, pane: group.pane, place }));
}

/** The target the keyboard starts on: the middle of the first pane offered, else the strip. */
export function firstTarget(groups: readonly Group[]): Target {
  return entryOf(groups[0] ?? { kind: "strip", at: 0 });
}

/** Where a group is entered: a pane's middle, or the strip's one target. */
function entryOf(group: Group): Target {
  return group.kind === "strip" ? group : { ...group, place: "middle" };
}

/** Each place on the cross, as a column and a row from its middle. */
const ON_THE_CROSS: Record<Place, readonly [number, number]> = {
  top: [0, -1],
  left: [-1, 0],
  middle: [0, 0],
  right: [1, 0],
  bottom: [0, 1],
};
const ARROWS: Record<Exclude<Step, "Tab" | "ShiftTab">, readonly [number, number]> = {
  ArrowUp: [0, -1],
  ArrowDown: [0, 1],
  ArrowLeft: [-1, 0],
  ArrowRight: [1, 0],
};

/**
 * **The target a key moves to from `at`.** An arrow moves on the pane's cross: from the middle to
 * the side it points at; from a side back to the middle with the opposite arrow, round to the
 * side it points at with another, and nowhere with the side's own. Tab goes to the next group's
 * entry and Shift+Tab to the one before, round from the last to the first. A target that has
 * gone (its pane closed) starts again on the first.
 */
export function stepFrom(groups: readonly Group[], at: Target, step: Step): Target {
  const index = groups.findIndex((group) => holds(group, at));
  if (index < 0 || groups.length === 0) return firstTarget(groups);
  if (step === "Tab" || step === "ShiftTab") {
    const next = (index + (step === "Tab" ? 1 : -1) + groups.length) % groups.length;
    return entryOf(groups[next]);
  }
  if (at.kind === "strip") return at;
  const [dx, dy] = ARROWS[step];
  const [x, y] = ON_THE_CROSS[at.place];
  const place: Place =
    at.place === "middle"
      ? placeOn(dx, dy)
      : x + dx === 0 && y + dy === 0
        ? "middle"
        : x === dx && y === dy
          ? at.place
          : placeOn(dx, dy);
  return { ...at, place };
}

function placeOn(dx: number, dy: number): Place {
  return (Object.keys(ON_THE_CROSS) as Place[]).find(
    (place) => ON_THE_CROSS[place][0] === dx && ON_THE_CROSS[place][1] === dy,
  ) as Place;
}

function holds(group: Group, at: Target): boolean {
  return group.kind === "strip"
    ? at.kind === "strip"
    : at.kind === "pane" && at.tab === group.tab && at.pane === group.pane;
}

/** Whether two targets are the same place. */
export function sameTarget(a: Target, b: Target): boolean {
  return (
    holds(a.kind === "strip" ? a : { kind: "pane", tab: a.tab, pane: a.pane }, b) &&
    (a.kind === "strip" || (b.kind === "pane" && a.place === b.place))
  );
}

/** **What is said as something is picked up** (#4583's ruling, politely). */
export function pickedUpSaid(name: string): string {
  return `Picked up ${name}. Arrow keys choose a side, Tab the next pane, Enter drops, Escape cancels.`;
}

/** What is said when Esc puts it back. */
export const PUT_BACK = "Put back. Nothing moved.";

/**
 * **Dropping `source` on `to` with the keys**: the tabs after, recorded for the window's one
 * Undo, and what is said, "Dropped. <what it did>"; or the tabs unchanged and "Not dropped:
 * <why>", from the same verdict the pointer is given.
 */
export function placingDrop(
  tabs: Tabs,
  source: Source,
  to: Target,
  ctx: DropContext,
  size?: Size,
): { tabs: Tabs; said: string; dropped: boolean } {
  const verdict = judgeDrop(tabs, source, to, ctx, size);
  if (!verdict.ok) return { tabs, said: `Not dropped: ${verdict.why}`, dropped: false };
  return {
    tabs: recorded(tabs, dropOn(tabs, source, to, ctx, size), verdict.did),
    said: `Dropped. ${verdict.did}`,
    dropped: true,
  };
}
