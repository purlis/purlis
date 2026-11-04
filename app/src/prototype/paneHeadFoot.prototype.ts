// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, NOT FOR MERGE (spec #5191, milestone M70; branch `prototype/pane-head-foot`).
 *
 * The question: what should a chat pane's head and foot look like, and at which widths does
 * each drop its parts? This is the model the spec asks for ("two rows, one model"), as a
 * throwaway: for a pane kind and a width it answers the head's parts and the foot's parts.
 * The real one grows out of `paneHead.ts`; this file is thrown away.
 */

export type PaneKind = "chat" | "task" | "ended" | "folded" | "view";

/** The head's parts, in order. */
export type HeadPart =
  | "persona"
  | "name"
  | "crumbs"
  | "markWord"
  | "mark"
  | "stateButton"
  | "buttonWord"
  | "more"
  | "bell"
  | "splitRight"
  | "splitDown"
  | "close";

/** The foot's parts, in order. `skillsShort` is the skills warning shrunk to its ⚠. */
export type FootPart =
  | "harness"
  | "workspace"
  | "from"
  | "skills"
  | "skillsShort"
  | "sandbox"
  | "ctx"
  | "cache";

/** The head's ruled order (#4585 less the workspace part, which moved to the foot). Unchanged
 *  widths: the rule's own. */
export const HEAD_DROPS: readonly { part: HeadPart; below: number }[] = [
  { part: "markWord", below: 470 },
  { part: "buttonWord", below: 420 },
  { part: "mark", below: 380 },
  { part: "splitDown", below: 300 },
];

/**
 * The foot's order, at the widths this prototype proposes. Measured on the default theme at
 * the comfortable density with the longest case the stories draw (harness, a workspace badge,
 * `↳ from steward 3`, "2 skills missing", the sandbox, ctx and cache): its whole line is about
 * 560 px wide, so `cache` goes first under that.
 */
export const FOOT_DROPS: readonly { part: "cache" | "skills" | "from" | "workspace"; below: number }[] = [
  { part: "cache", below: 560 },
  { part: "skills", below: 480 },
  { part: "from", below: 400 },
  { part: "workspace", below: 340 },
];

export type Facts = {
  /** Whether the chat has a skills warning. */
  skillsWarning: boolean;
  /** Whether it came from another chat (a handed-off chat, or a task). */
  from: boolean;
  /** Whether its state has a state button (Stop while working, Brief on a task). */
  stateButton: boolean;
};

export type Answer = { head: HeadPart[]; foot: FootPart[] };

/** What a pane of `kind`, `width` px wide, draws in its head and its foot. */
export function layoutOf(kind: PaneKind, width: number, facts: Facts): Answer {
  if (kind === "folded") return { head: ["persona", "bell"], foot: [] };
  const headGone = new Set(HEAD_DROPS.filter((d) => width < d.below).map((d) => d.part));
  const footGone = new Set<string>(FOOT_DROPS.filter((d) => width < d.below).map((d) => d.part));
  if (kind === "view")
    return {
      head: (["splitRight", "splitDown", "close"] as HeadPart[]).filter((p) => !headGone.has(p)),
      foot: [],
    };
  const head: HeadPart[] = ["persona", kind === "task" ? "crumbs" : "name"];
  // A task's breadcrumb says its state; a chat's head says it with its mark.
  if (kind !== "task") {
    if (!headGone.has("mark")) head.push("mark");
    if (!headGone.has("markWord") && !headGone.has("mark")) head.push("markWord");
  }
  if (facts.stateButton && kind !== "ended") {
    head.push("stateButton");
    if (!headGone.has("buttonWord")) head.push("buttonWord");
  }
  head.push("more", "bell", "splitRight");
  if (!headGone.has("splitDown")) head.push("splitDown");
  // A pane showing a task of its session draws no close (#1486).
  if (kind !== "task") head.push("close");

  const foot: FootPart[] = ["harness"];
  if (!footGone.has("workspace")) foot.push("workspace");
  if (facts.from && !footGone.has("from")) foot.push("from");
  if (facts.skillsWarning) foot.push(footGone.has("skills") ? "skillsShort" : "skills");
  foot.push("sandbox", "ctx");
  if (!footGone.has("cache")) foot.push("cache");
  return { head, foot };
}
