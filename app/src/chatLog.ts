// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ChatLogRow, ChatLogSummary, StartProtection } from "./bindings";
import { HARNESSES } from "./settings/harnesses";
import { LEVEL_SAID } from "./settings/levels";
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
 * number as its Activity and its Network are. **It opens from five places** (R-226, T1-37): the
 * chat row, a Blocked log row, a block result (the turn's finish row in Updates), the session
 * record and the marker, each through `BlockedMarkers`' `open`, which presses that row.
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

/** What a way to a chat's Chat log says (Q217). */
export const OPEN_THE_CHAT_LOG = "Open the chat log";

/** A way to the Chat log of the chat called `name`, as a screen reader hears it: which chat's. */
export function openTheChatLogOf(name: string): string {
  return `${OPEN_THE_CHAT_LOG} of ${name}`;
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

/**
 * What an Allow of a group says where it is not the group's usual words (Q262). A budget's
 * Raise and Continue are both `allowed`, so its line is true of both (Q262 amended).
 */
const ALLOWED_SAID: Readonly<Record<string, string>> = {
  budget: "Let it go on past its budget",
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

/** How much a qualified block's report is worth, as its collapsed line groups it. */
export type Qualified = "unverified" | "cant_tell";

/**
 * **A chat's qualified blocks, as one line draws them** (R-105, T1-38): every block whose report
 * is unverified, or every one purlis can't tell more of, collapsed into one row with its count,
 * where the first of them stood.
 */
export type QualifiedRow = {
  kind: "qualified";
  at: string | null;
  qualifier: Qualified;
  rows: BlockRow[];
};

/** A row as the timeline draws it: the core's, an ask's raise and answer joined as one. */
export type DrawnRow =
  Exclude<ChatLogRow, { kind: "ask_raised" | "ask_answered" }> | AskRow | QualifiedRow;

/**
 * The qualifier a block's line is grouped under, or none for a plain one. A qualifier this
 * version does not know is unverified: verified is an allowlist (Q235).
 */
function qualifiedAs(row: BlockRow): Qualified | null {
  if (row.qualifier === null) return null;
  return row.qualifier === "cant_tell" ? "cant_tell" : "unverified";
}

/**
 * **One row per ask** (R-229): each answer joins the oldest raise of its group still without
 * one, and the row stands where the ask was raised. An answer with no raise before it (a log
 * older than raises) is a row of its own.
 *
 * **One row for a chat's unverified blocks, and one for those it can't tell more of** (R-105):
 * each stands where the first of its kind did, so however many a chat's side reports, they take
 * one line of its log.
 */
export function drawnRows(rows: readonly ChatLogRow[]): DrawnRow[] {
  const drawn: DrawnRow[] = [];
  const qualified = new Map<Qualified, QualifiedRow>();
  for (const row of rows) {
    const qualifier = row.kind === "blocked" ? qualifiedAs(row) : null;
    if (row.kind === "blocked" && qualifier !== null) {
      const group = qualified.get(qualifier);
      if (group !== undefined) group.rows.push(row);
      else {
        const first: QualifiedRow = { kind: "qualified", at: row.at, qualifier, rows: [row] };
        qualified.set(qualifier, first);
        drawn.push(first);
      }
    } else if (row.kind === "ask_raised") {
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
 *
 * **An unverified report's line is fixed words** (R-104, Q259): who blocked "a call", from the
 * closed word of its source and the harness its start named, and never its tool or target,
 * which the chat's side wrote. So is one purlis can't tell more of.
 */
export function blockSaid(row: BlockRow): string {
  if (qualifiedAs(row) !== null) return `${whoBlocked(row)} blocked a call`;
  return `${whoBlocked(row)} blocked ${whatBlocked(row)}`;
}

/** The most of a collapsed line's blocks its detail lists: the latest (R-105). */
export const QUALIFIED_LISTED = 10;

/** What each qualifier is called, after its count. */
const QUALIFIED_SAID: Readonly<Record<Qualified, string>> = {
  unverified: "unverified",
  cant_tell: "can't tell",
};

/** **A collapsed line** (R-105): how many, and of which kind: "12 unverified". */
export function qualifiedSaid(row: QualifiedRow): string {
  return `${row.rows.length} ${QUALIFIED_SAID[row.qualifier]}`;
}

/** Why a collapsed line's detail says so little, by its qualifier. */
const QUALIFIED_WHY: Readonly<Record<Qualified, string>> = {
  unverified:
    "each was reported from within the chat's reach, so purlis shows only who blocked a call, and when.",
  cant_tell: "purlis can't tell more of them, so it shows only who blocked a call, and when.",
};

/**
 * **A collapsed line's detail** (R-104, R-105): its count and why it says so little, the latest
 * of its blocks ([`QUALIFIED_LISTED`]), each in fixed words, and how many earlier ones are not
 * listed.
 */
export function qualifiedDetail(row: QualifiedRow): {
  said: string;
  listed: BlockRow[];
  earlier: string | null;
} {
  const listed = row.rows.slice(-QUALIFIED_LISTED);
  const left = row.rows.length - listed.length;
  return {
    said: `${qualifiedSaid(row)}: ${QUALIFIED_WHY[row.qualifier]}`,
    listed,
    earlier:
      left === 0
        ? null
        : left === 1
          ? "1 earlier one is not listed."
          : `${left} earlier ones are not listed.`,
  };
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
export function rowSaid(row: DrawnRow): string {
  switch (row.kind) {
    case "started":
      return startSaid(row);
    case "ended":
      return endSaid(row);
    case "blocked":
      return blockSaid(row);
    case "protection_changed":
      return protectionChangeSaid(row);
    case "ask":
      return askSaid(row);
    case "held":
      return holdSaid(row);
    case "loop_paused":
      return loopPauseSaid(row);
    case "qualified":
      return qualifiedSaid(row);
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
  return [...lines, ...cantTellLines(summary)];
}

/** The chat's harness by its name in the window, or "its harness" where none is known. */
function itsHarness(harness: string | null): string {
  return HARNESSES.find((one) => one.value === harness)?.label ?? "its harness";
}

/**
 * **What purlis can't tell of the chat's harness** (R-097, R-099, R-100; T1-38): from the core,
 * which reads it from the harness's adapter, never from the chat. A harness that gives no denial
 * signal (opencode, and Codex for now) can't tell its own check's blocks; one that reports no
 * mode purlis can check (Codex reports bypass for every chat, Q152) can't tell its mode, which
 * is then never said as a mode it runs in.
 */
export function cantTellLines(summary: ChatLogSummary): string[] {
  const harness = itsHarness(summary.harness);
  const lines: string[] = [];
  if (summary.cant_tell_its_checks_blocks)
    lines.push(
      `Can't tell what ${harness}'s own check blocks: it does not report them, so they are not in this log.`,
    );
  if (summary.cant_tell_its_mode)
    lines.push(
      `Can't tell which mode this chat runs in: ${harness} reports none purlis can check, so purlis goes by how it started the chat.`,
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
