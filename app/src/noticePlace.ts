// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { OUTSIDE, type Said } from "./actions";
import type { NoticeKind } from "./ui/titlebar/TitleBarWorkspaces";

/**
 * **Where a notice is drawn** (spec #3647, its one test seam): a notice about a whole workspace
 * hangs from that workspace's chip in the title bar, as a card with an arrow; a project's or the
 * app's result pops from the status bar's Notices item; a chat's stays on its pane band. Every one
 * is also listed in the Notices centre, wherever else it is drawn: the cards are views onto the
 * one notice store, never a store of their own (the one-place ruling of 2026-10-10).
 *
 * One pure function, {@link placeOf}, so the rules are held in one table
 * (`noticePlace.test.ts`) and the window only draws what it answers.
 */

/** What a notice is about, which decides where it is drawn. */
export type About =
  /** One whole workspace: a workspace made, its clones, its stale repos, its layout. */
  | { workspace: string }
  /** A project's or the app's action, about no one workspace (a refused pin). */
  | { project: true }
  /** A chat, said on its pane's band. */
  | { pane: number };

/** **A chip of the title bar**, as the placement reads it: the workspace it is, and how it is
 *  drawn. `unpinned` is the dashed `name +` chip of the current workspace while it is not
 *  pinned; `dormant`, a pin whose folder is gone; `root`, the plane root's house after the
 *  project's name (`OUTSIDE`, #3654), always there. */
export type Chip = { workspace: string; look: "pinned" | "unpinned" | "dormant" | "root" };

/** Where a notice is drawn, besides the Notices centre, which lists every one. */
export type Place =
  /** The card that hangs from `workspace`'s chip. */
  | { at: "chip"; workspace: string }
  /** The card that pops from the status bar's Notices item. */
  | { at: "status" }
  /** The band of the pane that shows the chat. */
  | { at: "pane"; pane: number }
  /** The Notices centre alone, with a way to open `open`: no chip to hang from. */
  | { at: "centre"; open: string };

/**
 * **Where a notice about `about` is drawn, given the title bar's `chips`** (spec #3647):
 * - about a workspace that has a chip, pinned, dashed or dormant: that chip's card; about the
 *   plane root, its house chip's (the dispatcher's ruling on #3648);
 * - about a workspace with no chip: the Notices centre alone, which offers to open it, so
 *   nothing floats over the wrong chip;
 * - a project's or the app's result: the status bar's card;
 * - a chat's: its pane band, as it always was.
 */
export function placeOf(about: About, chips: readonly Chip[]): Place {
  if ("pane" in about) return { at: "pane", pane: about.pane };
  if ("project" in about) return { at: "status" };
  return chips.some((chip) => chip.workspace === about.workspace)
    ? { at: "chip", workspace: about.workspace }
    : { at: "centre", open: about.workspace };
}

/** **What a result is about**: the workspace it names (a workspace made names it), else the
 *  project it was said in. */
export function aboutOf(said: Said): About {
  return said.workspace === undefined ? { project: true } : { workspace: said.workspace };
}

/** **What kind of notice a result is** (#3649), which its chip's dot shows when it is the most
 *  urgent there: a refusal or a failure is a warning (▲), anything else news (●). */
export function kindOf(said: Said): NoticeKind {
  return said.refused ? "warning" : "info";
}

/**
 * **The title bar's chips** (`TitleBarSwitch.tsx`, R101–R105, #3654): the plane root's house,
 * always, then a chip per pin, in the pins' order, then a dormant chip per pin whose workspace is
 * gone (#3651), then the dashed one of the workspace in front while it is not pinned. The plane
 * root in front, or nothing focused yet, adds no dashed one.
 *
 * The dormant chips come after the live pins, not in their place among them, so ⌘1–⌘9 and a
 * swipe, which go to live pins alone, go to the chips in the order they are drawn.
 */
export function chipsOf({
  pins,
  dormant = [],
  focused,
}: {
  pins: readonly string[];
  /** Pins whose workspace is gone, kept dormant (V91c as amended, NO-1). */
  dormant?: readonly string[];
  focused: string | undefined;
}): Chip[] {
  const chips: Chip[] = [
    { workspace: OUTSIDE, look: "root" },
    ...pins.map((workspace): Chip => ({ workspace, look: "pinned" })),
    ...dormant
      .filter((workspace) => !pins.includes(workspace))
      .map((workspace): Chip => ({ workspace, look: "dormant" })),
  ];
  // A workspace in front that has gone is drawn once, dormant: never also as a dashed chip,
  // whose press would pin a workspace the project does not have (#3651's review).
  if (
    focused !== undefined &&
    focused !== OUTSIDE &&
    !pins.includes(focused) &&
    !dormant.includes(focused)
  )
    chips.push({ workspace: focused, look: "unpinned" });
  return chips;
}
