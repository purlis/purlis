// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ChatLogRow, StartProtection } from "./bindings";
import { HARNESSES } from "./settings/profiles";
import { LEVEL_SAID } from "./settings/yourLevel";
import type { ViewRef } from "./tabs";

/**
 * **A chat's Chat log in the window** (R-224, R-225, R-310, T1-27; ADR 0124 decision 11): the
 * one home for the chat's story, beside its Activity, which is unchanged. Its own simple
 * timeline, apart from the transcript, read from the core's `chat_log`, a query of the event
 * log. Phase 1 draws its first row, the start, and its last, the end and its result; the rows
 * between are T1-28's.
 *
 * It is a view tab, `{ from: null, view: "chat-log", key: "<session>" }`: one per chat, opened
 * from the chat's list and from the palette (`chat.log:<session>`), and keyed by the chat's
 * number as its Activity and its Network are.
 *
 * **Pull-only** (R-225): read once as the tab opens. Nothing the window hears reads it again or
 * marks it, and its tab carries no count and no New marker.
 */
export const CHAT_LOG_VIEW = "chat-log";

/** The Chat log tab of the chat in `session`. */
export function chatLogView(session: number): ViewRef {
  return { from: null, view: CHAT_LOG_VIEW, key: String(session) };
}

const TITLED = "Chat log · ";

/** What the Chat log tab of the chat called `name` is called. */
export function chatLogTitle(name: string): string {
  return `${TITLED}${name}`;
}

/** The chat's name in a Chat log tab's `title`. */
export function chatLogName(title: string): string {
  return title.startsWith(TITLED) ? title.slice(TITLED.length) : title;
}

/** Whether `view` is a chat's Chat log tab. */
export function isChatLog(view: ViewRef): boolean {
  return view.from === null && view.view === CHAT_LOG_VIEW;
}

/** The chat a Chat log tab is about; `undefined` for a key that is not a chat's number. */
export function chatLogSession(view: ViewRef): number | undefined {
  return /^[1-9]\d*$/.test(view.key) ? Number(view.key) : undefined;
}

/** Who set a layer, by the tier the start event recorded (T1-06). */
const TIER_SAID: Readonly<Record<string, string>> = {
  policy: "set by an administrator's policy",
  project: "set by the project",
  person: "set by you",
  unconfirmed: "set in the project's local file, not confirmed by you",
  chat: "set by you for this chat only",
  purlis: "purlis's default",
};

/** What a layer's line says where the start event could not tell who set it. */
const TIER_UNKNOWN = "who set it is not known";

/** Why a run began, as the start row says it. */
const CAUSE_SAID: Readonly<Record<string, string>> = {
  start: "Started",
  reopen: "Started again in its conversation",
  fresh: "Started again without its conversation",
};

function layerSaid(layer: StartProtection["layers"][number]): string {
  const level =
    LEVEL_SAID[layer.level as keyof typeof LEVEL_SAID] ?? `${layer.layer} ${layer.level}`;
  const by =
    layer.tier === "unknown"
      ? TIER_UNKNOWN
      : (TIER_SAID[layer.tier] ?? "set by a tier this version does not know");
  return `${level}, ${by}`;
}

function commitSaid(protection: StartProtection): string {
  if (protection.charter_commit === "none") return "purlis.toml in no commit";
  if (protection.charter_commit === "unknown") return "purlis.toml's commit not known";
  const at = `purlis.toml at ${protection.charter_commit.slice(0, 7)}`;
  return protection.charter_dirty ? `${at}, with changes not committed` : at;
}

function answersSaid(count: number): string {
  if (count === 0) return "no answers in force";
  return count === 1 ? "1 answer in force" : `${count} answers in force`;
}

/**
 * **The start row's one line** (R-227): the four fields of the start event, each from the
 * event and none from the chat: each layer's level and who set it, the `purlis.toml` commit,
 * and the answers in force.
 */
export function startSaid(row: Extract<ChatLogRow, { kind: "started" }>): string {
  const began = CAUSE_SAID[row.cause] ?? "Started again";
  const protection = row.protection;
  if (protection === null) return `${began} · what protected it was not recorded`;
  return [
    began,
    ...protection.layers.map(layerSaid),
    commitSaid(protection),
    answersSaid(protection.answers_in_force),
  ].join(" · ");
}

/** How a stop by the host is said, by its cause's word. */
const STOPPED_SAID: Readonly<Record<string, string>> = {
  closed: "stopped as you closed it",
  operator: "stopped by you",
  quit: "stopped as purlis quit",
  killed: "stopped by Stop all",
};

/**
 * **The end row's line** (R-233): that the chat ended, and its result. A run the project's
 * settings replaced at once ended `superseded` (ADR 0076 §2, Q247): its own line says so, and
 * the start row after it is the run that took its place.
 */
export function endSaid(row: Extract<ChatLogRow, { kind: "ended" }>): string {
  if (row.cause === "superseded")
    return "Ended · started again because the project's settings changed";
  if (row.state === "completed") return "Ended · done";
  if (row.state === "stopped") return `Ended · ${STOPPED_SAID[row.cause] ?? "stopped"}`;
  if (row.cause === "host-crash") return "Ended · failed, as purlis stopped while it ran";
  if (row.code !== null) return `Ended · failed, exit code ${row.code}`;
  return "Ended · failed";
}

/** A block row, as the core reads it (T1-29). */
export type BlockRow = Extract<ChatLogRow, { kind: "blocked" }>;

/**
 * **What each arm of purlis's gate blocked**, as an action a sentence names (R-183, Q196): never
 * the arm's id. An id this version does not know is said as the call it was.
 */
const ARM_SAID: Readonly<Record<string, string>> = {
  caps: "a command too big to read in time",
  A: "a command that could leak a secret",
  A2: "a command that reads a credential",
  A3: "a branch move in the project's main folder",
  A3b: "a reset that would destroy commits",
  A3c: "an update the sandbox would stop partway",
  A3d: "an update of a folder the sandbox would stop partway",
  A4: "an unattended publish",
  A5: "a live substitution in a forge's text",
  A6: "a live substitution in purlis's own text",
  A7: "a handoff it could not check",
  A7b: "a command that needs your consent, under another name",
  A7c: "a command spelt around one of your rules",
  A7d: "a dispatch from a sub-agent",
  A7e: "a dispatch beside a live substitution",
  A8: "a commit that skips the hooks",
  A9: "a git command that would stop on the project's lock",
  A9b: "a git add or commit in a branch folder",
  A10: "a dispatch, handoff or commit with a rider",
};

/** The harness, by its name in the window; "Its harness" for one this version does not know. */
function harnessSaid(harness: string): string {
  return HARNESSES.find((one) => one.value === harness)?.label ?? "Its harness";
}

/** Who blocked a call, as a sentence's subject. */
function whoBlocked(row: BlockRow): string {
  switch (row.source) {
    case "guard":
      return "purlis";
    case "harness_check":
      return `${harnessSaid(row.harness)}'s own check`;
    case "sandbox":
      return "The sandbox";
    case "policy":
      return "Your organisation's policy";
    case "project_setting":
      return "The project's settings";
    default:
      return "Something";
  }
}

/** What was blocked: the action its target names, or the call it was. */
function whatBlocked(row: BlockRow): string {
  const call = row.tool === "" || row.tool === "unknown" ? "a call" : `a ${row.tool} call`;
  if (row.target === null) return call;
  switch (row.target_kind) {
    case "guard_arm":
      return ARM_SAID[row.target] ?? call;
    case "host":
      return `a connection to ${row.target}`;
    case "service":
      return `a connection to service ${row.target}`;
    case "folder":
      return row.target === "." ? "a change in the project's folder" : `a change in ${row.target}`;
    default:
      return call;
  }
}

/**
 * **A block row's line** (R-183, R-170): an action sentence the window builds from the block,
 * who blocked what ("purlis blocked a reset that would destroy commits"), never a guard's or a
 * group's id. A target is the chat's side's word, and is drawn as text.
 */
export function blockSaid(row: BlockRow): string {
  return `${whoBlocked(row)} blocked ${whatBlocked(row)}`;
}

/** **A block's source, as its detail always names it** (R-187), for tagged and untagged rows. */
export function blockSourceSaid(row: BlockRow): string {
  switch (row.source) {
    case "guard":
      return "Blocked by purlis's guards";
    case "harness_check":
      return `Blocked by ${harnessSaid(row.harness)}'s own check`;
    case "sandbox":
      return "Blocked by the sandbox";
    case "policy":
      return "Blocked by your organisation's policy";
    case "project_setting":
      return "Blocked by the project's settings";
    default:
      return "Blocked, by a source purlis did not record";
  }
}

/** A tag a block row carries, by the kind of icon beside its word. */
export type BlockTagKind = "sandbox" | "harness_check" | "unverified" | "cant_tell";

/**
 * **The tags a block row carries** (R-185, R-186): the Sandbox, the harness's own check,
 * Unverified and Can't tell, in that order; purlis's own consents (its guards, verified) carry
 * none. The policy and project tags come in phase 2.
 */
export function blockTags(row: BlockRow): { kind: BlockTagKind; word: string }[] {
  const tags: { kind: BlockTagKind; word: string }[] = [];
  if (row.source === "sandbox") tags.push({ kind: "sandbox", word: "Sandbox" });
  if (row.source === "harness_check")
    tags.push({ kind: "harness_check", word: `${harnessSaid(row.harness)}'s own check` });
  if (row.qualifier === "unverified") tags.push({ kind: "unverified", word: "Unverified" });
  if (row.qualifier === "cant_tell") tags.push({ kind: "cant_tell", word: "Can't tell" });
  return tags;
}

/** A row's line. */
export function rowSaid(row: ChatLogRow): string {
  if (row.kind === "started") return startSaid(row);
  if (row.kind === "blocked") return blockSaid(row);
  return endSaid(row);
}

/** A row's time, as the timeline shows it: `2026-11-13 09:00 UTC`, or nothing. */
export function atSaid(at: string | null): string {
  const read = at === null ? Number.NaN : Date.parse(at);
  if (Number.isNaN(read)) return "";
  return `${new Date(read).toISOString().slice(0, 16).replace("T", " ")} UTC`;
}
