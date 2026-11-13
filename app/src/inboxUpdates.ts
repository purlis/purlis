// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Inbox's updates** (#1693, spec #1688, I-6, I-8, I-10): what happened that the person
 * may want to know and need not answer. Listed after the asks, newest first.
 *
 * **Each comes from a source that already says it**, derived here by a plain function with no
 * state, so each rule is tested on its own:
 *
 * - a task that finished or came to nothing: its finished row ({@link taskUpdates});
 * - a chat that came back, resumed or fresh: the chats a launch or a Resume put back
 *   ({@link resumeUpdates});
 * - a chat another chat started, announced and not looked at yet: a handoff, or a task that
 *   can't report asks (#4759) ({@link arrivedUpdates});
 * - a sandbox change that asks nothing: a block that is no host ask ({@link sandboxUpdates});
 * - a dispatch refused while nobody was there (#1507): the away list ({@link awayUpdates});
 * - a Smart close that stopped without its record (SI-8f) ({@link smartCloseUpdates});
 * - an Ask about a chat's browser that ended with the browser (#2015) ({@link browserAskUpdates});
 * - a report a chat drafted, filed by the person (#5694) ({@link reportSentUpdates}).
 *
 * **Kept a day on this machine, never in a project** (`note_inbox_updates`, I-8): the window
 * notes what it derived, by a key of its source's own, and the core keeps it in purlis's data
 * home, so the same update noted again is one update, a relaunch keeps it, and one the person
 * dismissed does not come back while its source still says it.
 *
 * **The last two are answered at their source**, so they are drawn only while it still lists
 * them ({@link LIVE}): a refused dispatch with its three answers, a stopped Smart close with
 * Go and Dismiss. Every other update stands for its day.
 *
 * **Dismiss all and Mark all read act on updates alone** (I-10): no ask is ever answered in
 * bulk ({@link useInboxUpdates}).
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { awaySaid } from "./AwayRefusals";
import { arrivedTitle, CANT_REPORT_WORDS } from "./actionResult";
import { cantReportAsks } from "./chatsTree";
import {
  commands,
  type AskEnded,
  type AwayRefusal,
  type ChatBlocked,
  type FinishedTask,
  type InboxUpdate,
  type OpenChat,
  type PlaneId,
  type UpdateKind,
  type UpdateNoted,
  type UpdateSettled,
} from "./bindings";
import { ROOT_UPDATE_REFUSED_SAID, type Noticed } from "./chatState";
import { qualifierOf, shownOf } from "./finished";
import { finishedBucketOf } from "./taskBuckets";
import type { SentDraft } from "./ReportDraftNotices";

/** The time now, in ms: when the window first saw an update that has no time of its own. */
export const seenAt = () => Date.now();

/** Seconds since 1970, as an update keeps its time. */
export const secondsOf = (ms: number) => Math.floor(ms / 1000);

/** How long an update is kept: a day (I-6), as the core keeps it. */
export const KEPT_FOR_MS = 24 * 60 * 60 * 1000;

/** The kinds drawn only while their source still lists them: each is answered there. */
export const LIVE: ReadonlySet<UpdateKind> = new Set([
  "refused-away",
  "smart-close",
  "report-undelivered",
  "commit-refused",
  "root-update-refused",
  // Drawn while its chat reads Stuck? (#6014): "It's fine" is answered there.
  "stuck",
]);

/** What a row says of what kind of thing happened. */
export const KIND_SAID: Record<UpdateKind, string> = {
  "task-done": "Task finished",
  "task-failed": "Task failed",
  doctor: "Doctor",
  resumed: "Came back",
  sandbox: "Sandbox",
  "refused-away": "Blocked while you were away",
  "smart-close": "Smart close stopped",
  "report-undelivered": "Report not delivered",
  "commit-refused": "Commit refused",
  "root-update-refused": "Asks you to update the project",
  "browser-ask-ended": "Browser ended",
  "question-closed": "Question closed",
  "question-failed": "Question not routed",
  arrived: "Arrived",
  "report-sent": "Report sent",
  stuck: "Stuck?",
};

/**
 * **Which of Updates' two lists an update is listed in** (#5646, ADR 0119): `mine`, From your
 * chats, what the person's own chats and tasks did; or `project`, From the project, what is the
 * project's and no chat's of theirs. The bell counts the unread Mine lines alone.
 */
export type UpdateFrom = "mine" | "project";

/**
 * **Each kind's list.** Every kind a chat or task of the person's makes is Mine. A doctor finding
 * is about the plane, not about a chat, so it is Project's, and never rings the bell (#5645 folds
 * it into one Notice line). No kind is about another person: what the core keeps is this
 * machine's chats' and this project's, so there is nothing here to leave out.
 */
const FROM: Record<UpdateKind, UpdateFrom> = {
  "task-done": "mine",
  "task-failed": "mine",
  doctor: "project",
  resumed: "mine",
  sandbox: "mine",
  "refused-away": "mine",
  "smart-close": "mine",
  "report-undelivered": "mine",
  "commit-refused": "mine",
  "root-update-refused": "mine",
  "browser-ask-ended": "mine",
  "question-closed": "mine",
  "question-failed": "mine",
  arrived: "mine",
  "report-sent": "mine",
  stuck: "mine",
};

/** The list an update of `kind` is listed in. */
export const fromOf = (kind: UpdateKind): UpdateFrom => FROM[kind];

/**
 * **The list `update` is listed in**: its kind's, except that a line the first-open baseline set
 * aside (#5648, ADR 0119) is Project's, whatever made it: it was already there the first time
 * the project opened on this machine.
 */
export const listedFrom = (update: Pick<InboxUpdate, "kind" | "baselined">): UpdateFrom =>
  update.baselined === true ? "project" : fromOf(update.kind);

/** **The bell's count** (#5646): the unread Mine lines. Project is never counted. */
export const unreadMine = (updates: readonly InboxUpdate[] | undefined): number =>
  (updates ?? []).filter((one) => !one.read && listedFrom(one) === "mine").length;

/** The key of the update that says draft `id` was sent. */
export const reportSentKey = (id: string) => `report-sent:${id}`;

/**
 * **A report a chat drafted, filed by the person** (#5694), or sent by Always send (#5695): one
 * update per draft filed in this window, on the chat it came from, saying where it went
 * (`ReportDestination`), or that it waits on this machine to send (#5693). Always send's says
 * "Sent a report" from the start of its hold, while the row offers Undo.
 */
export function reportSentUpdates(sent: readonly SentDraft[]): UpdateNoted[] {
  return sent.map((one) => ({
    key: reportSentKey(one.id),
    kind: "report-sent",
    at: secondsOf(one.at),
    session: one.session,
    chain: [one.chat],
    says:
      one.to === "waiting"
        ? `Waiting to send a report: ${one.title}`
        : `Sent a report${one.to === "github" ? " on GitHub" : ""}: ${one.title}`,
  }));
}

/**
 * **A task that ended, done or come to nothing** (#1693): one update per finished row that
 * says when it ended, in the count its row is in (`finishedBucketOf`), worded as the row
 * words it. A row with no time it ended is no update: nothing places it. Nor is one that ended
 * a day or more before `now`: it is past what is kept.
 */
export function taskUpdates(
  finished: Iterable<FinishedTask>,
  nameOf: (session: number) => string,
  now: number,
): UpdateNoted[] {
  const out: UpdateNoted[] = [];
  for (const task of finished) {
    const ended = task.ended === null ? Number.NaN : Date.parse(task.ended);
    if (Number.isNaN(ended) || now - ended >= KEPT_FOR_MS) continue;
    const word = shownOf(task)?.word ?? task.how;
    const more = qualifierOf(task);
    out.push({
      key: `task:${task.id}`,
      kind: finishedBucketOf(task.how) === "failed" ? "task-failed" : "task-done",
      at: secondsOf(ended),
      session: task.asker,
      chain: [nameOf(task.asker)],
      says: more === undefined ? `${task.name}: ${word}` : `${task.name}: ${word} · ${more}`,
    });
  }
  return out;
}

/**
 * **A chat that came back** (#1693): resumed on its conversation, or started fresh in its
 * place and why. Only the chats the core said either of; a chat that simply started is none.
 */
export function resumeUpdates(chats: Iterable<OpenChat>, now: number): UpdateNoted[] {
  const out: UpdateNoted[] = [];
  for (const chat of chats) {
    if (chat.resumed === null && chat.fresh === null) continue;
    out.push({
      key: `resumed:${chat.session}:${chat.resumed ?? `fresh:${chat.fresh}`}`,
      kind: "resumed",
      at: secondsOf(now),
      session: chat.session,
      chain: [chat.name],
      says:
        chat.resumed !== null
          ? "Resumed its conversation"
          : `Came back as a new chat: ${chat.fresh}`,
    });
  }
  return out;
}

/**
 * **A chat another chat started, announced** (#4759, spec #4750 story 26): a handoff, or a task
 * that can't report asks, which started in no pane. One update per chat the core
 * says the person has not looked at yet (`unseen`), keyed by the chat's id, so noting it again
 * is the same update, and it stands for its day once the chat has been looked at. **It asks
 * nothing**, so the ✋ never counts it: the ✋ counts the asks registry alone.
 */
export function arrivedUpdates(
  chats: Iterable<OpenChat>,
  nameOf: (session: number) => string,
  now: number,
): UpdateNoted[] {
  const out: UpdateNoted[] = [];
  for (const chat of chats) {
    if (chat.unseen !== true || chat.from === null) continue;
    const name = nameOf(chat.session);
    const title = arrivedTitle(name, chat.from.name);
    out.push({
      key: `arrived:${chat.id ?? chat.session}`,
      kind: "arrived",
      at: secondsOf(now),
      session: chat.session,
      chain: [name],
      says: cantReportAsks(chat.from, chat.cant_report_asks)
        ? `${title}. ${CANT_REPORT_WORDS}`
        : title,
    });
  }
  return out;
}

/**
 * **A sandbox change that asks nothing** (#1693): a block that is no host ask (#1690 counts
 * only a host it offers to allow). Its sentence is the block's own, the core's words.
 */
export function sandboxUpdates(
  blocks: Readonly<Record<number, readonly ChatBlocked[]>>,
  nameOf: (session: number) => string,
  now: number,
): UpdateNoted[] {
  return Object.entries(blocks).flatMap(([key, held]) => {
    const session = Number(key);
    return held
      .filter((block) => block.offer !== "host" || block.ours || block.target === null)
      .map((block) => ({
        key: `sandbox:${session}:${block.operation}:${block.kind}:${block.ours}:${block.target ?? ""}`,
        kind: "sandbox" as const,
        at: secondsOf(now),
        session,
        chain: [nameOf(session)],
        says: block.said,
      }));
  });
}

/** The key of a refused dispatch's update: its pair and workspace. */
export const awayUpdateKey = (one: Pick<AwayRefusal, "asking" | "target" | "workspace">) =>
  `away:${one.asking}#${one.target}#${one.workspace ?? ""}`;

/**
 * **A dispatch refused while nobody was at its chat** (#1507): one update a pair and
 * workspace, at its last refusal. Answered where it is listed, with its own three answers.
 */
export function awayUpdates(refused: readonly AwayRefusal[]): UpdateNoted[] {
  return refused.map((one) => ({
    key: awayUpdateKey(one),
    kind: "refused-away",
    at: one.latest,
    session: null,
    chain: [],
    says: awaySaid(one),
  }));
}

/**
 * **An Ask about a chat's browser that ended with the browser** (#2015): the chat asked the
 * person to take it over, and the browser ended first (closed, narrowed, its chat closed, Stop
 * all). One update an Ask, at its end, in the core's words.
 */
export function browserAskUpdates(
  ended: readonly AskEnded[],
  nameOf: (session: number) => string,
): UpdateNoted[] {
  return ended.map((one) => ({
    key: `browser-ask:${one.session}:${one.browser}:${one.at}`,
    kind: "browser-ask-ended",
    at: one.at,
    session: one.session,
    chain: [nameOf(one.session)],
    says: one.says,
  }));
}

/** The key of a stopped Smart close's update: its chat and why. */
export const smartCloseKey = (session: number, why: string) => `smartclose:${session}:${why}`;

/**
 * **A Smart close that stopped without its record** (SI-8f): the chat stays open, and the
 * update says why until the person goes to it or dismisses it.
 */
export function smartCloseUpdates(
  stopped: Readonly<Record<number, string>>,
  nameOf: (session: number) => string,
  now: number,
): UpdateNoted[] {
  return Object.entries(stopped).map(([key, why]) => {
    const session = Number(key);
    return {
      key: smartCloseKey(session, why),
      kind: "smart-close",
      at: secondsOf(now),
      session,
      chain: [nameOf(session)],
      says: why,
    };
  });
}

/**
 * **What the app found a chat needs the person for** (#1448, #1694, I-1): a report with nowhere
 * to go (`found`, as its needs-you item says it, without the tasks that came to nothing) and a
 * commit refused (`refused`, the masked line). Each is information, never a decision the chat
 * waits on, so each is an update, drawn while its chat still has it (`LIVE`), and the asks
 * registry does not list the chat for it.
 */
export function reasonUpdates(
  found: Readonly<Record<number, readonly string[]>>,
  refused: Readonly<Record<number, readonly string[]>>,
  nameOf: (session: number) => string,
  now: number,
): UpdateNoted[] {
  const each = (
    of: Readonly<Record<number, readonly string[]>>,
    as: "report-undelivered" | "commit-refused",
  ) =>
    Object.entries(of).flatMap(([key, lines]) => {
      const session = Number(key);
      return lines.map((says) => {
        // A need the app found is told apart by its own sentence (qw348): the needs ride the
        // window as the sentences their items say.
        const kind: UpdateKind =
          as === "report-undelivered" && says === ROOT_UPDATE_REFUSED_SAID
            ? "root-update-refused"
            : as;
        return {
          key: reasonKey(kind, session, says),
          kind,
          at: secondsOf(now),
          session,
          chain: [nameOf(session)],
          says,
        };
      });
    });
  return [...each(found, "report-undelivered"), ...each(refused, "commit-refused")];
}

/** The key of an update of what the app found a chat needs the person for. */
export const reasonKey = (kind: UpdateKind, session: number, says: string) =>
  `${kind}:${session}:${says}`;

/**
 * **The updates as the Inbox lists them**: what the core keeps, newest first, leaving out an
 * update of a {@link LIVE} kind its source no longer lists, and putting in one its source lists
 * that the core does not keep (`live`): one that happened more than a day ago is still answered
 * where its source lists it, and is put away only there. A doctor finding the core kept from
 * before #5645 is left out: every finding is the Notices view's one line now (`DoctorNotices`).
 */
export function listed(
  kept: readonly InboxUpdate[],
  live: readonly UpdateNoted[],
): readonly InboxUpdate[] {
  const listing = new Set(live.map((one) => one.key));
  const held = new Set(kept.map((one) => one.key));
  return [
    ...kept.filter((one) => one.kind !== "doctor" && (!LIVE.has(one.kind) || listing.has(one.key))),
    ...live.filter((one) => !held.has(one.key)).map((one) => ({ ...one, read: false })),
  ].sort((one, other) => other.at - one.at || one.key.localeCompare(other.key));
}

/** What a project's updates are, and what can be done with them. */
export type InboxUpdates = {
  /** Newest first; nothing before the first read. */
  updates: readonly InboxUpdate[] | undefined;
  /** Why this machine keeps none, where it does not: the list is then this window's alone. */
  trouble?: string;
  /** Mark read, or Dismiss: the ones named, or every one listed (`null`). */
  settle: (keys: readonly string[] | null, how: UpdateSettled) => void;
  /** The doctor's findings already there at a fresh first open, by key (#5648): read. */
  setAside: (keys: readonly string[]) => void;
};

/** What is said where the core answered with no list of updates. */
const KEEPS_NONE = "purlis did not say what updates it keeps";

/** An answer that is a list of updates, or nothing: a test's catch-all is no list. */
const listOf = (data: unknown) => (Array.isArray(data) ? (data as InboxUpdate[]) : undefined);

/**
 * **A project's updates, noted and kept** (#1693): reads what the core keeps, notes each update
 * `derived` holds that this window has not noted yet, and settles what the person marks read or
 * dismisses. Where this machine keeps none (no data home), the updates are drawn from `derived`
 * alone, held in this window and gone at its end, and `trouble` says why.
 */
export function useInboxUpdates(plane: PlaneId, derived: readonly UpdateNoted[]): InboxUpdates {
  /** What the core keeps, and for which project: another project's list is none of this one's. */
  const [keptFor, setKept] = useState<{ plane: PlaneId; list: readonly InboxUpdate[] }>();
  const kept = keptFor?.plane === plane ? keptFor.list : undefined;
  const [trouble, setTrouble] = useState<string>();
  /** What this window settled, for a machine that keeps nothing. */
  const [settled, setSettled] = useState<ReadonlyMap<string, UpdateSettled>>(new Map());
  /** The keys this window noted already, so a source said again writes nothing. */
  const noted = useRef(new Set<string>());

  /** Each ask's number, and the newest whose answer landed: an answer that comes back after a
   *  newer one is older news, and is dropped. */
  const asked = useRef(0);
  const newest = useRef(0);
  const ask = useCallback(
    (
      asking: PlaneId,
      sent: () => Promise<
        { status: "ok"; data: InboxUpdate[] } | { status: "error"; error: string }
      >,
    ) => {
      const mine = ++asked.current;
      void sent()
        .then((answer) => {
          if (mine < newest.current) return;
          newest.current = mine;
          if (answer.status === "error") {
            setTrouble(answer.error);
            return;
          }
          const list = listOf(answer.data);
          // A core that answers with no list (an older build) keeps none: this window does.
          if (list === undefined) {
            setTrouble(KEEPS_NONE);
            return;
          }
          setTrouble(undefined);
          setKept({ plane: asking, list });
        })
        .catch((err: unknown) => {
          if (mine >= newest.current) setTrouble(String(err));
        });
    },
    [],
  );

  useEffect(() => {
    noted.current = new Set();
    ask(plane, () => commands.inboxUpdates(plane));
  }, [plane, ask]);

  useEffect(() => {
    const fresh = derived.filter((one) => !noted.current.has(one.key));
    if (fresh.length === 0) return;
    for (const one of fresh) noted.current.add(one.key);
    ask(plane, () => commands.noteInboxUpdates(plane, fresh));
  }, [plane, derived, ask]);

  const settle = useCallback(
    (keys: readonly string[] | null, how: UpdateSettled) => {
      setSettled((was) => {
        const now = new Map(was);
        for (const key of keys ?? derived.map((one) => one.key))
          if (now.get(key) !== "dismissed") now.set(key, how);
        return now;
      });
      ask(plane, () => commands.settleInboxUpdates(plane, keys === null ? null : [...keys], how));
    },
    [plane, derived, ask],
  );

  const setAside = useCallback(
    (keys: readonly string[]) => ask(plane, () => commands.setAsideInboxFindings(plane, [...keys])),
    [plane, ask],
  );

  const live = useMemo(() => derived.filter((one) => LIVE.has(one.kind)), [derived]);
  const updates = useMemo(() => {
    // What this window dismissed is gone at once, before the core's answer lands (#5650): a
    // line whose Undo went never shows again for the moment the core takes to answer.
    if (trouble === undefined)
      return kept === undefined
        ? undefined
        : listed(kept, live).filter((one) => settled.get(one.key) !== "dismissed");
    // Nothing kept on this machine: what the sources say now, as this window settled it.
    const here = derived
      .filter((one) => settled.get(one.key) !== "dismissed")
      .map((one) => ({ ...one, read: settled.get(one.key) === "read" }));
    return listed(
      here,
      live.filter((one) => settled.get(one.key) !== "dismissed"),
    );
  }, [kept, live, trouble, derived, settled]);

  return { updates, trouble, settle, setAside };
}

/**
 * **A chat that reads Stuck?** (#6014, ADR 0121 decision 19): a Should-know update, one per
 * chat and reason, drawn while the core says it reads so ({@link LIVE}). Nothing is sent beyond
 * the line: whether it notifies is #6005's. Its row offers "It's fine", which clears it until the
 * chat changes state.
 */
export function stuckUpdates(
  noticed: Readonly<Record<number, Noticed>> | undefined,
  nameOf: (session: number) => string,
  now: number,
): UpdateNoted[] {
  return Object.entries(noticed ?? {}).flatMap(([key, one]) => {
    if (one.stuck === null) return [];
    const session = Number(key);
    return [
      {
        key: `stuck:${session}:${one.stuck}`,
        kind: "stuck" as const,
        at: secondsOf(now),
        session,
        chain: [nameOf(session)],
        says: `Stuck? ${one.stuck}`,
      },
    ];
  });
}
