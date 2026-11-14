// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The order of Asks, by facts** (ADR 0121 decision 5, #6006): never a priority someone sets.
 *
 * 1. An **Expiring ask** (`Shown.expires`, still ahead): the one that ends soonest first.
 * 2. A **timed-out ask** (`Shown.timed_out`, ADR 0121 decision 15, #6007), still answerable:
 *    the longest wait first.
 * 3. An ask whose chat other chats wait on (`Shown.blocks`): the more it blocks, the sooner.
 * 4. The rest, the longest wait first.
 *
 * One chat's asks stay together, where the first of them stands, its Expiring ones first. Every
 * key is a fact with a time or a count, never when the window saw the list: so a new ask takes
 * its place by its facts and never jumps to the top for being new (ADR 0119 decision 4), and the
 * same asks are listed in the same order however the registry listed them.
 *
 * Pure: no commands and no markup. The Inbox draws what it answers, and each row says its
 * standing in words ([`standingSaid`]).
 */
import type { Shown } from "./bindings";
import { chainOfGroup, type Age, type ChatGroup } from "./inboxRules";

/** **Why an ask stands where it does**, as its row's reason line says it. Times in ms. */
export type Standing =
  /** It ends by itself at `at`. */
  | { by: "expires"; at: number }
  /** Its wait ended at `at` with nobody's answer, and it is answerable still (#6007). */
  | { by: "timed-out"; at: number }
  /** It stands with its chat's ask that expires. */
  | { by: "its-chat" }
  /** `chats` open chats wait on its chat's report. */
  | { by: "blocks"; chats: number }
  /** It has waited since `since`. */
  | { by: "wait"; since: number };

/** One ask, and why it stands where it does. */
export type Placed = { ask: Shown; why: Standing };

/** One chat's asks, in their order, each with its standing. */
export type PlacedGroup = ChatGroup & { placed: readonly Placed[] };

/** When `ask` ends by itself, in ms, where that is still ahead of `now`. */
const endsAt = (ask: Shown, now: number): number | undefined =>
  typeof ask.expires === "number" && ask.expires * 1000 > now ? ask.expires * 1000 : undefined;

/** When `ask`'s wait ended unanswered, in ms, where it is answerable still and not Expiring. */
const timedOutAt = (ask: Shown, now: number): number | undefined =>
  endsAt(ask, now) === undefined && typeof ask.timed_out === "number"
    ? ask.timed_out * 1000
    : undefined;

/** How many chats wait on `ask`'s chat. */
const blocking = (ask: Shown): number => ask.blocks ?? 0;

/** Older first, then the registry's order. */
const byAge = (one: Age, other: Age) => one[0] - other[0] || one[1] - other[1];

/**
 * **The asks, grouped by chat and ordered by facts.** `age` is how long each has waited (the
 * Inbox's `ageOf`), and `now` the time an Expiring ask's end is read against, in ms.
 * They are one project's: another project's asks are listed in its own Asks (ADR 0121, amended
 * 2026-11-12).
 */
export function inOrder(
  asks: readonly Shown[],
  age: (ask: Shown) => Age,
  now: number,
): PlacedGroup[] {
  const groups = new Map<number, { ask: Shown; age: Age; ends?: number; lapsed?: number }[]>();
  for (const ask of asks) {
    const group = groups.get(ask.session) ?? [];
    group.push({ ask, age: age(ask), ends: endsAt(ask, now), lapsed: timedOutAt(ask, now) });
    groups.set(ask.session, group);
  }
  /** 0 for an Expiring ask, 1 for a timed-out one, 2 for the rest. */
  const tier = (one: { ends?: number; lapsed?: number }) =>
    one.ends !== undefined ? 0 : one.lapsed !== undefined ? 1 : 2;
  const ranked = [...groups.values()].map((mine) => {
    const { session } = mine[0].ask;
    // Inside a chat: its Expiring asks, soonest first, then its timed-out ones, then the rest,
    // each oldest first.
    mine.sort(
      (one, other) =>
        tier(one) - tier(other) ||
        (one.ends ?? Infinity) - (other.ends ?? Infinity) ||
        byAge(one.age, other.age),
    );
    const ends = mine[0].ends;
    const lapsed = mine.some((one) => one.lapsed !== undefined);
    const blocks = Math.max(...mine.map(({ ask }) => blocking(ask)));
    const oldest = mine.map(({ age }) => age).sort(byAge)[0];
    const placed: Placed[] = mine.map(({ ask, ends: own, lapsed: ended }) => ({
      ask,
      why:
        own !== undefined
          ? { by: "expires", at: own }
          : ended !== undefined
            ? { by: "timed-out", at: ended }
            : ends !== undefined
              ? { by: "its-chat" }
              : blocks > 0
                ? { by: "blocks", chats: blocks }
                : { by: "wait", since: age(ask)[0] },
    }));
    return { session, ends, lapsed, blocks, oldest, placed };
  });
  ranked.sort(
    (one, other) =>
      (one.ends ?? Infinity) - (other.ends ?? Infinity) ||
      Number(other.lapsed) - Number(one.lapsed) ||
      (one.lapsed ? 0 : other.blocks - one.blocks) ||
      byAge(one.oldest, other.oldest) ||
      one.session - other.session,
  );
  return ranked.map(({ session, placed }) => {
    const mine = placed.map(({ ask }) => ask);
    return {
      session,
      chain: chainOfGroup(session, mine),
      asks: mine,
      placed,
    };
  });
}

/** Whether any ask among `placed` is Expiring: its row's countdown needs a clock by the second. */
export const anyExpiring = (groups: readonly PlacedGroup[]): boolean =>
  groups.some((group) => group.placed.some(({ why }) => why.by === "expires"));

/** **What a timed-out ask's row says** (ADR 0121 decision 15, #6007): it is answerable, and the
 *  answer reaches its chat. */
export const TIMED_OUT_SAID = "Timed out. Answer, and the chat is told to try again.";

/**
 * **A row's reason line** (#6006): why it stands where it does, in a few words. An Expiring
 * ask's counts down by the second under a minute, then in whole minutes, rounded up.
 */
export function standingSaid(why: Standing, now: number): string {
  switch (why.by) {
    case "expires": {
      const seconds = Math.ceil((why.at - now) / 1000);
      if (seconds <= 0) return "Expiring now";
      if (seconds < 60) return `Expires in ${seconds} s`;
      return `Expires in ${Math.ceil(seconds / 60)} min`;
    }
    case "timed-out":
      return TIMED_OUT_SAID;
    case "its-chat":
      return "With its chat's ask that expires";
    case "blocks":
      return `Blocks ${why.chats} ${why.chats === 1 ? "chat" : "chats"}`;
    case "wait": {
      if (!Number.isFinite(why.since) || why.since >= Number.MAX_SAFE_INTEGER) return "Waiting";
      const minutes = Math.floor((now - why.since) / 60_000);
      if (minutes < 1) return "Waiting under a minute";
      if (minutes < 60) return `Waiting ${minutes} min`;
      const hours = Math.floor(minutes / 60);
      if (hours < 24) return `Waiting ${hours} h`;
      return `Waiting ${Math.floor(hours / 24)} d`;
    }
  }
}
