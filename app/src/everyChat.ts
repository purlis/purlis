// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Shown as Ask } from "./bindings";
import { needsOf, type ChatStates } from "./chatState";
import type { ListedChat } from "./chatsTree";
import { standingOfRows } from "./sessionTasks";
import { CHAT_STATE_NAMES, CHAT_STATES, type ChatState, type ShownKind } from "./shownState";

/**
 * **All chats** (ADR 0121 decision 18, #6013; CONTEXT.md, **All chats**): the Chats sidebar's
 * mode that lists every chat in every open project, with its Chat state, how long it has been in
 * it, its project and workspace and its last line, Waiting on you first, then Stuck?, then the
 * rest.
 *
 * **Read the way the sidebar reads, and nothing more** (#6013's speed rule): each project makes
 * its own chats' entries from the rows its Chats list draws and the one pass that says each
 * row's state (`standingOfRows`), its clock (`stateClock`) and its asks registry, and tells the
 * window, which tells the other windows (`windows.ts`). Nothing here asks the core anything.
 *
 * Every entry is plain data, so a window can be told another window's.
 */
export type FleetChat = {
  /** The project it is in, by its plane. With `session`, what names it: a session number is
   *  only one project's. */
  plane: string;
  /** That project, as the strip names it. */
  project: string;
  session: number;
  name: string;
  persona: string | null;
  /** Where it works: a workspace's name, or the root's own word. */
  workspace: string;
  /** Its one Chat state (`shownState`). */
  state: ChatState;
  /** Its state's own word (`needs you`, `waiting on 2 tasks`): heard on its mark. */
  word: string;
  /** When it came into its state (ms), where the window saw it (`stateClock`). */
  since: number | null;
  /** **Its last line**: what it asks, why it needs you, or what its state's name leaves out
   *  (`waiting on 2 tasks`, `stopped by you`). Data, drawn as text. What a working chat is doing
   *  is read live where the chat is in this window (`ChatDoingLine`). */
  line: string | null;
  /** The keys of the asks it holds now: what Snooze puts away (`snoozeInboxAsks`). */
  asks: readonly string[];
  /** A task, which is ended by its own way (`endTask`) and not by Stop chat. */
  task: boolean;
  /** Its program still runs, so Stop has something to stop. */
  stoppable: boolean;
};

/** The kinds of a chat whose program has ended: nothing is left to stop. */
const ENDED: ReadonlySet<ShownKind> = new Set<ShownKind>([
  "done",
  "failed",
  "cancelled",
  "stopped-by-you",
  "closed-by-you",
  "stopped-at-limit",
  "stopped-with-above",
  "unreported",
  "last-message",
  "reported",
]);

/** What one project reads its entries from: what its Chats list already holds. */
export type FleetFrom = {
  plane: string;
  project: string;
  /** The rows of its Chats list, each chat before the tasks under it. */
  rows: readonly ListedChat[];
  states: ChatStates;
  /** When each chat came into its state (ms), where the clock saw it. */
  since: ReadonlyMap<number, number>;
  /** Its asks registry's asks, in its order. */
  asks: readonly Ask[];
};

/** **One project's chats, as All chats lists them**, in the order its Chats list has them. */
export function fleetOf({ plane, project, rows, states, since, asks }: FleetFrom): FleetChat[] {
  const stood = standingOfRows(states, rows);
  const asksOf = new Map<number, Ask[]>();
  for (const one of asks) asksOf.set(one.session, [...(asksOf.get(one.session) ?? []), one]);
  return rows.flatMap((row) => {
    const shown = stood.get(row.session)?.shown;
    if (shown === undefined) return [];
    const held = asksOf.get(row.session) ?? [];
    const first = held[0];
    const needs = needsOf(states, row.session);
    const said = shown.word.toLowerCase() === CHAT_STATE_NAMES[shown.mark].toLowerCase();
    return [
      {
        plane,
        project,
        session: row.session,
        name: row.name,
        persona: row.persona,
        workspace: row.workspace,
        state: shown.mark,
        word: shown.word,
        since: since.get(row.session) ?? null,
        line:
          first !== undefined
            ? (first.last_message ?? first.says)
            : needs.length > 0
              ? needs[needs.length - 1]
              : said
                ? null
                : shown.word,
        asks: held.map((one) => one.ask),
        task: row.mode === "task",
        stoppable: !ENDED.has(shown.kind),
      },
    ];
  });
}

/** What names an entry across projects. */
export const fleetKey = (chat: Pick<FleetChat, "plane" | "session">) =>
  `${chat.plane}#${chat.session}`;

/**
 * **The order All chats lists them in** (ADR 0121 decision 18): by Chat state in the one order
 * (`CHAT_STATES`: Waiting on you, Stuck?, then the rest), and in each state the longest in it
 * first. A chat whose time the window did not see comes after the timed ones of its state, and
 * then by name, so the order does not move for nothing.
 */
export function inFleetOrder(chats: readonly FleetChat[]): FleetChat[] {
  const rank = (chat: FleetChat) => CHAT_STATES.indexOf(chat.state);
  return [...chats].sort(
    (a, b) =>
      rank(a) - rank(b) ||
      (a.since ?? Infinity) - (b.since ?? Infinity) ||
      a.name.localeCompare(b.name) ||
      fleetKey(a).localeCompare(fleetKey(b)),
  );
}

/**
 * **How many chats are in each state** (#6013's seam for #6020): what the empty Asks says ("7
 * chats working · 1 stuck?"), each part opening All chats filtered to its state.
 */
export function countsByState(chats: readonly FleetChat[]): Record<ChatState, number> {
  const counts = Object.fromEntries(CHAT_STATES.map((state) => [state, 0])) as Record<
    ChatState,
    number
  >;
  for (const chat of chats) counts[chat.state] += 1;
  return counts;
}
