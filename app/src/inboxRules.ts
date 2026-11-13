// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Inbox's own rules** (#1692, spec #1688, I-6, I-12): how a project's asks are grouped and
 * ordered, what was answered here lately, and the bytes a reply is typed as. No commands and no
 * markup, so each rule is tested on its own; `Inbox.tsx` draws them.
 */
import { useCallback, useEffect, useRef, useSyncExternalStore } from "react";
import type { Shown } from "./bindings";

/** The asks of one chat, as the Inbox draws them under its chain. */
export type ChatGroup = {
  session: number;
  /** Who is asking, the session first and the chat that asked last (I-9): names, as data. */
  chain: readonly string[];
  asks: readonly Shown[];
};

/** The key an ask is known by across reads of its project's list. */
export const askKey = (ask: Shown) => `${ask.session}\u0000${ask.ask}`;

/**
 * **The key an ask is known by in Asks, which lists every open project's** (#1921): its own
 * {@link askKey} for the listing project's, and the same after its project for another's, so two
 * projects' asks never share one.
 */
export const heldKey = (ask: Shown, plane: string | undefined) =>
  plane === undefined ? askKey(ask) : `${plane}\u0001${askKey(ask)}`;

/** The time now, in ms: when this window first saw an ask. */
const seenNow = () => Date.now();

/**
 * **When this window first saw each ask**, by project: the time, and a sequence that keeps
 * apart asks seen in the same instant. Kept for the window's life, so a project put behind
 * another and brought back keeps its order.
 */
const seen = new Map<string, Map<string, { at: number; seq: number }>>();
let seq = 0;

/** Notes the asks of `plane` this window has not seen before, and forgets the ones gone. */
export function noteSeen(plane: string, asks: readonly Shown[]): void {
  const mine = seen.get(plane) ?? new Map<string, { at: number; seq: number }>();
  seen.set(plane, mine);
  const now = new Set(asks.map(askKey));
  for (const key of mine.keys()) if (!now.has(key)) mine.delete(key);
  const at = seenNow();
  for (const ask of asks) if (!mine.has(askKey(ask))) mine.set(askKey(ask), { at, seq: ++seq });
}

/** How old an ask is, as the Inbox orders them: the lower, the older. */
export type Age = readonly [when: number, seq: number];

/**
 * **How long `ask` of `plane` has waited** (#1700): since it began, where the registry knows
 * (`Shown.since`, seconds), else since this window first saw it; asks of the same instant in
 * the order they were first seen, which is the registry's order, its sources' order.
 */
export function ageOf(plane: string, ask: Shown): Age {
  const first = seen.get(plane)?.get(askKey(ask));
  const when =
    typeof ask.since === "number" ? ask.since * 1000 : (first?.at ?? Number.MAX_SAFE_INTEGER);
  return [when, first?.seq ?? Number.MAX_SAFE_INTEGER];
}

/**
 * **How long ago `when` was, as a row's meta says it**: `now` under a minute, then whole
 * minutes, hours and days (`5m`, `2h`, `3d`), the largest unit alone, as an editor's timeline
 * and a forge's list say an age. Nothing for a time that is not known, and a time ahead of the
 * clock is `now`: this machine's clock and the one that stamped the ask need not agree.
 */
export function agoSaid(when: number, now: number): string | undefined {
  if (!Number.isFinite(when) || when >= Number.MAX_SAFE_INTEGER) return undefined;
  const minutes = Math.floor((now - when) / 60_000);
  if (minutes < 1) return "now";
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h`;
  return `${Math.floor(hours / 24)}d`;
}

/**
 * What a group is headed: its chat, by its first ask. Session 0 is no chat's, and its asks can
 * come from different places (a change held from one chat, a sign-in), so where they differ the
 * head says purlis alone and each row says its own (#5240). The groups and their order are
 * `asksOrder.ts`'s (#6006).
 */
export function chainOfGroup(session: number, mine: readonly Shown[]): readonly string[] {
  const first = mine[0].chain;
  if (first.length === 0) return [`chat ${session}`];
  const differ = mine.some((ask) => ask.chain.join() !== first.join());
  return session === 0 && differ ? ["purlis"] : first;
}

/** The sources whose asks a pane draws a Notice for: a reply is answered in the pane itself. */
const ON_A_PANE: ReadonlySet<Shown["source"]> = new Set([
  "dispatch",
  "permission",
  "terminal",
  "sandbox-host",
  "sandbox-write",
  "sandbox-local",
  "workspace-removal",
  "proposed-change",
  "runner-write",
  "runner-dispatch",
  "browser",
  "browser-site",
  "browser-needs-you",
  "browser-login",
  "secret-fill",
  "vault",
  "pool-login",
  "pool-tool",
  "secret",
  "run-grant",
  // A chat paused at its budget or as looping (#6015).
  "budget",
  "looping",
]);

/**
 * **The asks of one chat its pane draws a Notice for** (#1695, spec #1688; #4754): the chat's
 * in-context copy of what the Inbox lists, every one of them, the longest waiting first
 * (`since`, else the registry's order). There is no cap: the band scrolls past its room.
 */
export function onItsPane(asks: readonly Shown[], session: number): Shown[] {
  return asks
    .map((ask, at) => ({ ask, at }))
    .filter(({ ask }) => ask.session === session && ON_A_PANE.has(ask.source))
    .sort(
      (one, other) =>
        (one.ask.since ?? Number.MAX_SAFE_INTEGER) - (other.ask.since ?? Number.MAX_SAFE_INTEGER) ||
        one.at - other.at,
    )
    .map(({ ask }) => ask);
}

/** What the Inbox keeps of an ask once it was answered from it (I-12): read-only. */
export type Answered = {
  /** When, in ms since the epoch. */
  at: number;
  /** The chat that asked, by which the chips list its sentence (ADR 0121, amended 2026-11-12,
   *  decision 9); 0 or left out where no chat asked (a change purlis holds, a sign-in). */
  session?: number;
  chain: readonly string[];
  says: string;
  /** The answer as its button read, or a reply's words. */
  answer: string;
  /** What the source said of the answer, where it said something the person should read: Keep
   *  blocked on a vault the chat's persona already holds (#5890). A Notice says it in the Inbox,
   *  whatever else waits, until the person dismisses it ([`dismissSaid`]). */
  said?: string;
  /** Whether `said` sends the person to Settings › Sandbox › Network › Allowed: its Notice links there
   *  (#5939). */
  allowedList?: true;
};

/** The most answered asks a project's Inbox lists. */
export const MOST_ANSWERED = 10;

/**
 * **What was answered from the Inbox, by project, newest first** (I-12). In memory: it is a
 * look back over what the person agreed to while the window was open, never a record, and the
 * sources keep their own (the network record, the grants).
 */
const answered = new Map<string, readonly Answered[]>();
const readers = new Set<() => void>();
const NONE: readonly Answered[] = [];

/** The newest [`MOST_ANSWERED`], and any older one whose sentence the person has not dismissed
 *  yet: only Dismiss puts that away (review of #5934). */
const kept = (all: readonly Answered[]): readonly Answered[] =>
  all.filter((one, at) => at < MOST_ANSWERED || one.said !== undefined);

export function noteAnswered(plane: string, one: Answered): void {
  answered.set(plane, kept([one, ...(answered.get(plane) ?? NONE)]));
  for (const changed of readers) changed();
}

/** **The person read what an answer said and dismissed it** (#5890): the answer stays listed as
 *  answered, and its sentence goes. `one` is the answer as [`useAnswered`] gave it. */
export function dismissSaid(plane: string, one: Answered): void {
  const now = answered.get(plane) ?? NONE;
  answered.set(
    plane,
    kept(
      now.map((each) => {
        if (each !== one) return each;
        const rest = { ...each };
        delete rest.said;
        delete rest.allowedList;
        return rest;
      }),
    ),
  );
  for (const changed of readers) changed();
}

/** What `plane`'s Inbox answered lately, newest first: the newest [`MOST_ANSWERED`], and any older
 *  one whose sentence still stands. */
export function useAnswered(plane: string): readonly Answered[] {
  return useSyncExternalStore(
    (changed) => {
      readers.add(changed);
      return () => readers.delete(changed);
    },
    () => answered.get(plane) ?? NONE,
  );
}

/**
 * **An answer given on a Notice, noted once its ask goes** (#1700, 2026-10-10 21:09 item 2):
 * `touch(ask, answer)` when a way out of the Notice drawn for `ask` is pressed, with its words;
 * the ask is listed as answered once `asks` (its project's registry) no longer holds it, since a
 * press is no answer until its source stops waiting. One store for every place that draws an
 * ask's Notice, the Inbox's and a pane's alike: what this window answered, in memory, never a
 * record (the sources keep their own, read after a relaunch by `allowedLately`).
 */
export function useAnsweredWhenGone(
  plane: string,
  asks: readonly Shown[] | undefined,
): (ask: Shown, answer: string) => void {
  const touched = useRef(new Map<string, { ask: Shown; answer: string }>());
  useEffect(() => {
    const listed = new Set((asks ?? []).map(askKey));
    for (const [key, { ask, answer }] of touched.current) {
      if (listed.has(key)) continue;
      touched.current.delete(key);
      noteAnswered(plane, {
        at: Date.now(),
        session: ask.session,
        chain: ask.chain,
        says: ask.says,
        answer,
      });
    }
  });
  return useCallback((ask: Shown, answer: string) => {
    touched.current.set(askKey(ask), { ask, answer });
  }, []);
}

/** Forgets every project's order and answers: for a test, which starts with none. */
export function forgetInbox(): void {
  seen.clear();
  answered.clear();
  seq = 0;
  for (const changed of readers) changed();
}

/** Whether `c` is a C0 or C1 control character, or DEL. */
const isControl = (c: string) => {
  const code = c.codePointAt(0) ?? 0;
  return code < 0x20 || (code >= 0x7f && code <= 0x9f);
};

/** What starts and ends a bracketed paste. */
const PASTE_BEGINS = "\u001b[200~";
const PASTE_ENDS = "\u001b[201~";

/**
 * **A reply, as the bytes a chat is sent** (#1692): one bracketed paste, then Enter, in one
 * write, as purlis types any line into a chat (`curation::bracketed`), so a harness reads it as
 * the person's message and a line break inside it does not send half of it. Every control
 * character but a line break is dropped first, so nothing typed can end the paste early or reach
 * the terminal as a key. Nothing at all for a reply with nothing in it.
 */
export function replyBytes(text: string): string | undefined {
  const plain = [...text.replace(/\r\n?/g, "\n")]
    .filter((c) => c === "\n" || !isControl(c))
    .join("");
  if (plain.trim() === "") return undefined;
  return `${PASTE_BEGINS}${plain}${PASTE_ENDS}\r`;
}
