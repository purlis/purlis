// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ChatLogRow, GrantLevel } from "./bindings";
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
 * - **A sandbox block** (T1-34; R-196, R-291): Allow next time and Show this rule. Allow next
 *   time is offered only where an Allow would unblock it (#1906, Q265): a host purlis's own proxy
 *   refused, not resolved since. It makes a grant with no ask open, at one of the shipped Allow
 *   labels, and the core judges it again. Show this rule opens a detail that only reads.
 * - A policy's or a project's block's ways out are phase 2's: none yet.
 *
 * **Only Allow next time widens what a chat may do** (Q3, Q139), and only by the person's press
 * after its row settled: Tell the chat… types one line of purlis's own that says the call stays
 * blocked, and the three Show… open a detail that only reads.
 */

/** Asks first (docs/ui-copy.md): what it types, then the person's press types it. */
export const TELL_THE_CHAT = "Tell the chat…";
export const SHOW_THIS_GUARD = "Show this guard";
export const SHOW_THIS_CHECK = "Show this check";
export const GO_TO_CHAT = "Go to chat";
/** A sandbox block's (T1-34, R-196): its press offers the shipped Allow labels first. */
export const ALLOW_NEXT_TIME = "Allow next time";
export const SHOW_THIS_RULE = "Show this rule";

/** A way out, in the order a row offers them. */
export type Way = "allow-next" | "tell" | "show-guard" | "show-check" | "show-rule" | "go";

const ORDER: readonly Way[] = ["allow-next", "tell", "show-guard", "show-check", "show-rule", "go"];

/** What a way out is said as. */
export const WAY_SAID: Readonly<Record<Way, string>> = {
  "allow-next": ALLOW_NEXT_TIME,
  tell: TELL_THE_CHAT,
  "show-guard": SHOW_THIS_GUARD,
  "show-check": SHOW_THIS_CHECK,
  "show-rule": SHOW_THIS_RULE,
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
  /** It has since been resolved (Q272): a Chat log row's, where a later call came back. */
  resolved?: boolean;
  /** **The labels its Allow next time may offer** (T1-34, Q278), as the core says policy keeps
   *  them: none, and it offers no Allow next time. */
  allow_levels?: readonly GrantLevel[];
};

/** **The ways out of one block** (R-189, R-192, R-199), in their order. Tell the chat… is
 *  offered only while the block stands (Q272): its line says what was just blocked. */
function waysOfOne(block: BlockOfWays): Way[] {
  if (block.qualifier === "cant_tell") return ["go"];
  if (block.source === "sandbox")
    return allowsNextTime(block) ? ["allow-next", "show-rule"] : ["show-rule"];
  const tell: Way[] = block.resolved === true ? [] : ["tell"];
  if (block.source === "guard") return [...tell, "show-guard"];
  if (block.source === "harness_check") return [...tell, "show-check"];
  return [];
}

/** **The ways out of `blocks`**, each once, in their order: a finish row's turn may hold several. */
export function waysOut(blocks: readonly BlockOfWays[]): Way[] {
  const ways = new Set(blocks.flatMap(waysOfOne));
  return ORDER.filter((way) => ways.has(way));
}

/**
 * **Whether a sandbox block offers Allow next time** (T1-34; #1906, Q265, Q278): only where an
 * Allow would unblock it. A host purlis's own proxy refused (no qualifier: not only the chat's
 * hook's word), named, not resolved since, and with a label the core says policy keeps. The core
 * checks the same before it grants anything.
 */
export function allowsNextTime(block: BlockOfWays): boolean {
  return (
    block.source === "sandbox" &&
    (block.qualifier ?? null) === null &&
    block.target_kind === "host" &&
    block.target !== null &&
    block.resolved !== true &&
    (block.allow_levels ?? []).length > 0
  );
}

/**
 * **What Allow next time offers** (Q188, Q197): the shipped Allow labels, in a sandbox host ask's
 * order (this project on this machine, this chat, everyone in the project), with Allow once,
 * the chat's next attempt alone, first. No label of its own.
 */
export const NEXT_TIME_LEVELS: readonly GrantLevel[] = ["once", "you", "chat", "project"];

/** **The labels a grouped row offers each chat by name** (Q278, Q273): a grant to that one chat. */
export const ONE_CHATS_LEVELS: readonly GrantLevel[] = ["once", "chat"];

/** **The labels a grouped row offers once for all its chats** (Q278): one grant, for this
 *  project on this machine or for everyone in it, sent once. */
export const EVERY_CHATS_LEVELS: readonly GrantLevel[] = ["you", "project"];

/** **The labels `blocks` offer for `target`**, in their order: the core's, as policy keeps them
 *  (Q278), read from the first of its blocks that offers Allow next time. */
export function nextTimeLevels(
  blocks: readonly BlockOfWays[],
  target: string,
): readonly GrantLevel[] {
  const kept = blocks.find((one) => one.target === target && allowsNextTime(one))?.allow_levels;
  return NEXT_TIME_LEVELS.filter((level) => (kept ?? []).includes(level));
}

/** What Allow next time says above its labels, of `target`, on one chat's row. */
export const nextTimeSaid = (target: string) =>
  `${target}, from this chat's next attempt at it. Allow once lets that one attempt through; the others stay until you remove them under Allowed.`;

/** "a", "a and b", "a, b and c". */
const listed = (names: readonly string[]) =>
  names.length < 2 ? names.join("") : `${names.slice(0, -1).join(", ")} and ${names.at(-1)}`;

/** What Allow next time says above its labels, of `target`, on a row of several chats, each
 *  named by `names` (Q278): it never says one chat's. */
export const nextTimeGroupedSaid = (target: string, names: readonly string[]) =>
  `${target}, blocked for ${listed(names)}. Allow once lets the named chat's next attempt through; the others stay until you remove them under Allowed.`;

/** What a grouped row says above the labels it offers once for all its chats (Q278). */
export const FOR_EVERY_CHAT = "For every chat here, one grant:";

/** What a press of one of its labels came to: the core's sentence, or why nothing was allowed. */
export type NextTimeSaid = { said?: string; why?: string };

/** What a label pressed on a row drawn or moved just now says (ADR 0097, `mayAct`). */
export const ROW_MOVED_JUST_NOW =
  "This row was drawn or moved just now, so nothing was allowed. Read it and press again.";

/** **What Show this rule says of a sandbox block** (R-196): the rule that refused it, read-only
 *  in phase 1. */
export function ruleSaid(block: BlockOfWays): string {
  const target = block.target ?? "";
  if (block.target_kind === "host" && target !== "")
    return `The sandbox lets a chat here reach only the Open hosts and the hosts you allow. ${target} is not one of them.`;
  if (block.target_kind === "folder" && target !== "")
    return `The sandbox lets a chat here write only in its project and the folders you allow. ${target} is not one of them.`;
  if (block.target_kind === "service" && target !== "")
    return `The sandbox lets a chat here reach only the services of this machine you allow. ${target} is not one of them.`;
  return "The sandbox keeps a chat here to its project and to what you allow.";
}

/** Where the sandbox's rules are read in full. */
export const RULE_WHERE =
  "Settings › Sandbox › Network lists the Open hosts, what was blocked and what you allowed.";

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

/** **What Tell the chat… asks first where it types into several chats** (Q272): each by name. */
export const tellAsksOf = (names: readonly string[]) =>
  names.length <= 1
    ? TELL_ASKS
    : `purlis types one line of purlis's own into each of ${names.join(", ")}: that the call stays blocked, and not to try it again. It types it into each only while that chat waits for you.`;

/** The press that types it. */
export const TYPE_IT = "Type it in the chat";

/** What is said once it is typed. */
export const TOLD = "Typed in the chat.";

/** What is said once it is typed into the chats `names` (Q272): each by name. */
export const toldIn = (names: readonly string[]) => `Typed in ${names.join(", ")}.`;

/** **What a press of Type it in the chat came to**: what was typed, and why nothing was where
 *  it wasn't. Either, both or neither may be said. */
export type Told = { typed?: string; why?: string };
