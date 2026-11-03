// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Layout } from "./tabs";

/**
 * **Where each pane is in its tab** (#4767, ruled on #4699), worked out from the tab's tree and
 * its splits' shares alone, so it is one plain answer wherever the window asks it:
 *
 * - {@link paneToward}: the pane ⌘⌥ and an arrow moves the keyboard to (Alt and an arrow off a
 *   Mac). There is no wrap, as in iTerm2: at the edge nothing moves.
 * - {@link paneNamesOf}: what a screen reader calls each pane ("left pane", "top right pane"),
 *   so a line about a pane it did not follow says where that pane is.
 * - {@link paneTakingTheSpace}: the pane the keyboard goes to when a pane closes, which is the
 *   one that grows into its space.
 */

/** The four ways ⌘⌥ and an arrow move. */
export type Toward = "left" | "right" | "up" | "down";

/** A pane's place in its tab, each edge a fraction of the tab's width or height. */
type Place = { pane: number; left: number; top: number; right: number; bottom: number };

/** What a split holds as its first child's share, in tenths of a percent (`laidOut.shareOf`). */
const EVEN = 500;
const WHOLE = 1000;

/** Edges are sums of products of shares, so two that meet may differ in the last bits. */
const NEAR = 1e-9;
const same = (a: number, b: number) => Math.abs(a - b) < NEAR;

/** Every pane's place, in the tree's order. */
function placesOf(layout: Layout): Place[] {
  const out: Place[] = [];
  const walk = (node: Layout, left: number, top: number, right: number, bottom: number) => {
    if (node.kind === "pane") {
      out.push({ pane: node.pane, left, top, right, bottom });
      return;
    }
    const share = (node.share ?? EVEN) / WHOLE;
    if (node.direction === "row") {
      const at = left + (right - left) * share;
      walk(node.children[0], left, top, at, bottom);
      walk(node.children[1], at, top, right, bottom);
    } else {
      const at = top + (bottom - top) * share;
      walk(node.children[0], left, top, right, at);
      walk(node.children[1], left, at, right, bottom);
    }
  };
  walk(layout, 0, 0, 1, 1);
  return out;
}

/** How much of two spans lie over each other. */
const overlap = (a0: number, a1: number, b0: number, b1: number) =>
  Math.max(0, Math.min(a1, b1) - Math.max(a0, b0));

/**
 * The pane next to pane `from` toward `to`: of the panes whose edge meets `from`'s on that
 * side, the one alongside most of it, and the first (leftmost or topmost) where two are
 * alongside as much. Nothing at the tab's edge, or for a pane not in it.
 */
export function paneToward(layout: Layout, from: number, to: Toward): number | undefined {
  const places = placesOf(layout);
  const at = places.find((one) => one.pane === from);
  if (at === undefined) return undefined;
  const across = to === "left" || to === "right";
  let best: { pane: number; along: number; start: number } | undefined;
  for (const one of places) {
    if (one.pane === from) continue;
    const meets =
      (to === "right" && same(one.left, at.right)) ||
      (to === "left" && same(one.right, at.left)) ||
      (to === "down" && same(one.top, at.bottom)) ||
      (to === "up" && same(one.bottom, at.top));
    if (!meets) continue;
    const along = across
      ? overlap(one.top, one.bottom, at.top, at.bottom)
      : overlap(one.left, one.right, at.left, at.right);
    if (along < NEAR) continue;
    const start = across ? one.top : one.left;
    if (
      best === undefined ||
      along > best.along + NEAR ||
      (same(along, best.along) && start < best.start)
    )
      best = { pane: one.pane, along, start };
  }
  return best?.pane;
}

/**
 * **What a reader calls each pane**, by where it is: a side ("left", "right", "top", "bottom")
 * or "middle" for each way the tab is divided across it, and nothing for a way it spans whole,
 * so a pane alone is "pane" and a full-width pane over two is "top pane". Panes that would
 * share a name are numbered in reading order ("middle pane 1"). In the tree's order.
 */
export function paneNamesOf(layout: Layout): Map<number, string> {
  const places = placesOf(layout);
  const side = (start: number, end: number, first: string, last: string) => {
    const atStart = same(start, 0);
    const atEnd = same(end, 1);
    if (atStart && atEnd) return undefined;
    if (atStart) return first;
    if (atEnd) return last;
    return "middle";
  };
  const named = places.map((one) => {
    const vertical = side(one.top, one.bottom, "top", "bottom");
    const horizontal = side(one.left, one.right, "left", "right");
    const words =
      vertical === "middle" && horizontal === "middle" ? ["middle"] : [vertical, horizontal];
    return { ...one, name: [...words.filter((word) => word !== undefined), "pane"].join(" ") };
  });
  const reading = [...named].sort((a, b) => (same(a.top, b.top) ? a.left - b.left : a.top - b.top));
  const counted = new Map<string, number>();
  for (const one of named) counted.set(one.name, (counted.get(one.name) ?? 0) + 1);
  const numbered = new Map<number, string>();
  const seen = new Map<string, number>();
  for (const one of reading) {
    if ((counted.get(one.name) ?? 0) < 2) continue;
    const n = (seen.get(one.name) ?? 0) + 1;
    seen.set(one.name, n);
    numbered.set(one.pane, `${one.name} ${n}`);
  }
  return new Map(named.map((one) => [one.pane, numbered.get(one.pane) ?? one.name]));
}

/**
 * **The pane that grows into pane `closed`'s space** when it closes (ruled on #4699: the
 * keyboard goes there): the pane beside it across the split it leaves, the one alongside most
 * of it. Nothing for a tab's only pane, or a pane not in the tab.
 */
export function paneTakingTheSpace(layout: Layout, closed: number): number | undefined {
  const found = splitHolding(layout, closed);
  if (found === undefined) return undefined;
  const { split, first } = found;
  const to: Toward = split.direction === "row" ? (first ? "right" : "left") : first ? "down" : "up";
  return paneToward(layout, closed, to);
}

/** The split pane `pane` is a direct child of, and whether it is the first child. */
function splitHolding(
  layout: Layout,
  pane: number,
): { split: Extract<Layout, { kind: "split" }>; first: boolean } | undefined {
  if (layout.kind === "pane") return undefined;
  const [a, b] = layout.children;
  if (a.kind === "pane" && a.pane === pane) return { split: layout, first: true };
  if (b.kind === "pane" && b.pane === pane) return { split: layout, first: false };
  return splitHolding(a, pane) ?? splitHolding(b, pane);
}
