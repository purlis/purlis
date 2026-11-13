// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ChatLogRow, ChatLogSummary, StartProtection } from "./bindings";
import { LEVEL_SAID } from "./settings/yourLevel";
import type { ViewRef } from "./tabs";

/**
 * **A chat's Chat log in the window** (R-224, R-225, R-310, T1-27; ADR 0124 decision 11): the
 * one home for the chat's story, beside its Activity, which is unchanged. Its own simple
 * timeline, apart from the transcript, read from the core's `chat_log`, a query of the event
 * log. Phase 1 draws its first row, the start, and its last, the end and its result, and between
 * them its protection changes, its asks with their answers, its holds and its loop pauses
 * (T1-28); its blocks are T1-29's. Its honesty lines and its allowed calls head it.
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

/** Who made a protection change, by the kind of actor its trust event names. */
const ACTOR_SAID: Readonly<Record<string, string>> = {
  human: "set by you",
  host: "set by purlis",
  agent: "asked for by a chat",
};

/**
 * **A protection change's line, and who made it** (R-228): in phase 1, the sandbox lifted for a
 * run or put back on, through a shipped path.
 */
export function protectionChangeSaid(row: Extract<ChatLogRow, { kind: "protection_changed" }>) {
  const what = row.change === "on" ? "The sandbox is back on" : "No sandbox for this run";
  return `${what} · ${ACTOR_SAID[row.actor_kind] ?? "who made it is not known"}`;
}

/** What an ask of each group asked, as its row says it after "Asked you". */
const ASK_SAID: Readonly<Record<string, string>> = {
  sandbox: "about something the sandbox blocked",
  harness_prompt: "about its harness's permission prompt",
  consent: "about a step that needs your consent",
  hold: "about an action purlis held",
  loop_pause: "whether to keep going after a loop",
  budget: "whether to go past a budget",
  hook_give_up: "about a hook that gave up",
};

/** The groups whose Allow lets the chat go on, and whose refusal stops it. */
const GOES_ON = new Set(["loop_pause", "budget"]);

/** What an Allow of a group says where it is not the group's usual words (Q262). */
const ALLOWED_SAID: Readonly<Record<string, string>> = {
  budget: "Raised its budget",
};

/** Who answered, as an ask's row says it. */
const BY_SAID: Readonly<Record<string, string>> = {
  window: "by you in the window",
  inbox: "by you from purlis inbox",
};

/** **An ask, as one row draws it** (R-229): its raise, and its answer once there is one. */
export type AskRow = {
  kind: "ask";
  at: string | null;
  ask: string;
  answered: Extract<ChatLogRow, { kind: "ask_answered" }> | null;
};

/** A row as the timeline draws it: the core's, an ask's raise and answer joined as one. */
export type DrawnRow = Exclude<ChatLogRow, { kind: "ask_raised" | "ask_answered" }> | AskRow;

/**
 * **One row per ask** (R-229): each answer joins the oldest raise of its group still without
 * one, and the row stands where the ask was raised. An answer with no raise before it (a log
 * older than raises) is a row of its own.
 */
export function drawnRows(rows: readonly ChatLogRow[]): DrawnRow[] {
  const drawn: DrawnRow[] = [];
  for (const row of rows) {
    if (row.kind === "ask_raised") {
      drawn.push({ kind: "ask", at: row.at, ask: row.ask, answered: null });
    } else if (row.kind === "ask_answered") {
      const open = drawn.find(
        (one): one is AskRow => one.kind === "ask" && one.ask === row.ask && one.answered === null,
      );
      if (open === undefined) drawn.push({ kind: "ask", at: row.at, ask: row.ask, answered: row });
      else open.answered = row;
    } else {
      drawn.push(row);
    }
  }
  return drawn;
}

/** **An ask's line** (R-229): what it asked, its answer, and who answered. */
export function askSaid(row: AskRow): string {
  const asked = `Asked you ${ASK_SAID[row.ask] ?? "something"}`;
  const answer = row.answered;
  if (answer === null) return `${asked} · Not answered`;
  const goesOn = GOES_ON.has(row.ask);
  const said =
    answer.answer === "allowed"
      ? (ALLOWED_SAID[row.ask] ?? (goesOn ? "Let it go on" : "Allowed"))
      : goesOn
        ? "Stopped"
        : "Kept blocked";
  return `${asked} · ${said}, ${BY_SAID[answer.by] ?? "by you"}`;
}

/** The arm a held merge is held at (`purlis_core::mergehold::REASON`). */
const MERGE_HOLD = "merge-hold";

/** What a stop row under one of the chat's sub-agents ends with (Q261). */
const OF_A_HELPER = " · from one of its helpers";

/**
 * **A hold's line** (R-231): what purlis held for the person, and, where it was held under one
 * of the chat's sub-agents, that it was its helper's (Q261). Grants are phase 3's rows.
 */
export function holdSaid(row: Extract<ChatLogRow, { kind: "held" }>): string {
  const helper = row.sub_agent ? OF_A_HELPER : "";
  if (row.arm === MERGE_HOLD)
    return `Held a merge · purlis merges it only when you press Merge${helper}`;
  const call = row.tool === "" || row.tool === "unknown" ? "a call" : `a ${row.tool} call`;
  return `Held ${call} · purlis asked you first${helper}`;
}

/** **A loop pause's line** (R-232): which rule paused the chat, and at how many times. */
export function loopPauseSaid(row: Extract<ChatLogRow, { kind: "loop_paused" }>): string {
  switch (row.rule) {
    case "failing":
      return `Paused · the same command failed ${row.times} times in a row`;
    case "refused":
      return `Paused · ${row.times} of its asks were kept blocked in a row`;
    case "blocked":
      return `Paused · it was blocked ${row.times} times on the same thing`;
    default:
      return "Paused · it went round and round";
  }
}

/** A row's line. */
export function rowSaid(row: DrawnRow): string {
  switch (row.kind) {
    case "started":
      return startSaid(row);
    case "ended":
      return endSaid(row);
    case "protection_changed":
      return protectionChangeSaid(row);
    case "ask":
      return askSaid(row);
    case "held":
      return holdSaid(row);
    case "loop_paused":
      return loopPauseSaid(row);
  }
}

/**
 * **The honesty lines** (R-235): what heads the log of a chat whose reports purlis cannot
 * check, and of one that ran with No sandbox, in that order. None for a chat that is neither.
 */
export function honestyLines(summary: ChatLogSummary): string[] {
  const lines: string[] = [];
  if (summary.unverified)
    lines.push(
      "Unverified: this chat could reach what reports its calls to purlis, so purlis cannot check this log.",
    );
  if (summary.ran_as_you)
    lines.push(
      "Ran as you: this chat had No sandbox, so it could reach everything you can on this machine.",
    );
  return lines;
}

/**
 * **The allowed calls, in one line** (R-234): `212 allowed · 9 by your rules`, both counts
 * always, so none by the person's rules reads as 0 (Q261); none for a chat with no allowed call.
 */
export function allowedSaid(summary: ChatLogSummary): string | null {
  if (summary.allowed === 0) return null;
  return `${summary.allowed} allowed · ${summary.by_your_rules} by your rules`;
}

/** A row's time, as the timeline shows it: `2026-11-13 09:00 UTC`, or nothing. */
export function atSaid(at: string | null): string {
  const read = at === null ? Number.NaN : Date.parse(at);
  if (Number.isNaN(read)) return "";
  return `${new Date(read).toISOString().slice(0, 16).replace("T", " ")} UTC`;
}
