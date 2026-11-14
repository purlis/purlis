// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ChatLogRow } from "./bindings";
import { harnessSaid } from "./chatLog";

/**
 * **A block's ways out** (T1-33; R-189, R-191 to R-194, R-199; ADR 0124 decision 6.3): what a
 * person can do about a block from its Chat log row and its turn's finish row (R-300), never
 * from its marker. Each label is written here once, and `askLabels.guard.test.ts` holds every
 * file that draws one to it.
 *
 * - **A guard block:** Tell the chat… and Show this guard. No Allow (R-191): a guard's no is
 *   undone only by switching the guard, which is phase 2's (R-190).
 * - **A block by the harness's own check:** Tell the chat… and Show this check, and the
 *   sentence that only that check decides (R-193). No Allow and no Copy command (R-194): a
 *   command handed out to run by hand would take it past that check.
 * - **A Can't-tell block:** Go to chat (R-199): purlis can't tell who blocked it.
 * - A sandbox block's ways out are T1-34's (R-196), a policy's or a project's phase 2's: none
 *   yet.
 *
 * **None widens what a chat may do** (Q3, Q139): Tell the chat… types one line of purlis's own
 * that says the call stays blocked, and the two Show… open a detail that only reads.
 */

/** Asks first (docs/ui-copy.md): what it types, then the person's press types it. */
export const TELL_THE_CHAT = "Tell the chat…";
export const SHOW_THIS_GUARD = "Show this guard";
export const SHOW_THIS_CHECK = "Show this check";
export const GO_TO_CHAT = "Go to chat";

/** A way out, in the order a row offers them. */
export type Way = "tell" | "show-guard" | "show-check" | "go";

const ORDER: readonly Way[] = ["tell", "show-guard", "show-check", "go"];

/** What a way out is said as. */
export const WAY_SAID: Readonly<Record<Way, string>> = {
  tell: TELL_THE_CHAT,
  "show-guard": SHOW_THIS_GUARD,
  "show-check": SHOW_THIS_CHECK,
  go: GO_TO_CHAT,
};

/** What a block's ways out are read from: who blocked it, how much its report is worth, and at
 *  what. A finish row's blocks are the same fields of the core's count (`FinishedBlock`). */
export type BlockOfWays = Pick<
  Extract<ChatLogRow, { kind: "blocked" }>,
  "source" | "target_kind" | "target"
> & {
  qualifier?: string | null;
  harness?: string;
};

/** **The ways out of one block** (R-189, R-192, R-199), in their order. */
function waysOfOne(block: BlockOfWays): Way[] {
  if (block.qualifier === "cant_tell") return ["go"];
  if (block.source === "guard") return ["tell", "show-guard"];
  if (block.source === "harness_check") return ["tell", "show-check"];
  return [];
}

/** **The ways out of `blocks`**, each once, in their order: a finish row's turn may hold several. */
export function waysOut(blocks: readonly BlockOfWays[]): Way[] {
  const ways = new Set(blocks.flatMap(waysOfOne));
  return ORDER.filter((way) => ways.has(way));
}

/** **What a harness-check block says** (R-193): purlis can't allow what that check blocked. */
export const onlyItsCheckDecides = (harness: string | undefined) =>
  `purlis can't allow this; only ${harnessSaid(harness)}'s own check decides`;

/**
 * **Each guard's group** (ADR 0124 decision 2.2), by the arm a block's target carries: the
 * name the person's switches will carry (R-190, phase 2). Shown only behind Show this guard
 * (R-184): a row says what was blocked, never a group. An arm the decision puts in no group has
 * none here.
 */
const GROUP_OF: Readonly<Record<string, string>> = {
  A: "Secrets",
  A2: "Secrets",
  A3: "Project folder",
  A3b: "Project folder",
  A3c: "Project folder",
  A3d: "Project folder",
  A4: "Releases and merges",
  A5: "Issue and PR text",
  A6: "Issue and PR text",
  A7: "Starting other chats",
};

/** What the read-only detail of a guard says of its group, and of a guard no group holds. */
export function guardGroupSaid(block: BlockOfWays): string {
  const group =
    block.target_kind === "guard_arm" && block.target !== null ? GROUP_OF[block.target] : undefined;
  return group === undefined
    ? "One of purlis's guards, in no group of switches"
    : `One of purlis's ${group} guards`;
}

/** What the read-only detail of a guard says it can't do yet (R-009, R-190). */
export const GUARD_SWITCH_LATER =
  "Turning a guard off for one chat comes in a later version of purlis.";

/** What the read-only detail of a harness's own check says. */
export const checkSaid = (harness: string | undefined) =>
  `The rules of ${harnessSaid(harness)}'s own permission check are ${harnessSaid(harness)}'s, in its settings, not purlis's.`;

/** **What Tell the chat… asks first** (docs/ui-copy.md): what its press types, and when. */
export const TELL_ASKS =
  "purlis types one line of purlis's own into the chat: that the call stays blocked, and not to try it again. It types it only while the chat waits for you.";

/** The press that types it. */
export const TYPE_IT = "Type it in the chat";

/** What is said once it is typed. */
export const TOLD = "Typed in the chat.";
