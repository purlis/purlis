// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A block shows as a marker on its chat: "2 new blocked"** (R-172, R-174, R-175, R-299,
 * R-301; ADR 0124 decision 9.1; T1-30). On the chat's sidebar row and its pane's band, it counts
 * the chat's verified blocks since the person last looked, and opens its Chat log filtered to
 * blocks. Unverified reports never count there: they show a muted dot, and their count is in
 * the Chat log.
 *
 * **The last look is kept where the shipped New markers keep theirs** (`seen.ts`: the Inbox's
 * seen record), as a key `blocks:<chat>:<seq>`, the chat by its logged id and the `seq` of the
 * latest block the person looked at (Q258). The core hands the key over with the counts, and
 * keeps one per chat: a new look replaces the earlier. No new store. A chat is looked at once
 * it has been on screen in the focused window, by the shipped rule (`useChatOnScreenSeen`), or
 * once its Chat log has been opened.
 *
 * **"Just so you know"** (R-176, R-177): drawn, never notified or announced, and it touches
 * nothing of Quiet or snooze. It only reads the core's counts (`chat_blocks`), as the window
 * comes back into focus and every few seconds while it is drawn.
 */
import { createContext, useContext, useEffect, useMemo, useRef, useState } from "react";
import { commands, type ChatBlocks, type PlaneId } from "./bindings";

/** What every look's key in the seen record starts with. */
export const BLOCKS = "blocks:";

/** How often the counts are read again while the window is drawn. */
export const BLOCKS_READ_EVERY_MS = 5000;

/**
 * **The last look of the chat look `key` names** (`blocks:<chat>:<seq>`, as the core hands it):
 * the latest of its blocks it was looked at, or `null` for none, or for a chat not counted yet.
 */
export function lastLook(seen: ReadonlySet<string>, key: string | null): string | null {
  if (key === null || !key.startsWith(BLOCKS)) return null;
  const prefix = key.slice(0, key.lastIndexOf(":") + 1);
  if (prefix.length <= BLOCKS.length) return null;
  let latest: string | null = null;
  for (const key of seen) {
    if (!key.startsWith(prefix)) continue;
    const at = key.slice(prefix.length);
    if (latest === null || later(at, latest)) latest = at;
  }
  return latest;
}

/** Whether `seq` is after `than`: decimal numbers, so a longer one is later (`10` after `9`). */
function later(seq: string, than: string): boolean {
  return seq.length === than.length ? seq > than : seq.length > than.length;
}

/** "2 new blocked": the marker's words, the word "new" in each. */
export function blockedSaid(count: number): string {
  return `${count} new blocked`;
}

/** What a reader hears of the muted dot. */
export const UNVERIFIED_SAID = "Unverified blocks, counted in its Chat log";

/** What a chat's marker shows: its count of verified blocks, or a muted dot. */
export type Marker = { kind: "count"; said: string; key: string } | { kind: "dot"; key: string };

/**
 * **What chat's marker shows**, from the core's count and the seen record: nothing once its
 * latest block has been looked at, so a look clears it before the core is asked again; the
 * count of verified blocks; or, where only unverified ones are new, a muted dot (R-299).
 */
export function markerOf(
  counted: ChatBlocks | undefined,
  seen: ReadonlySet<string>,
): Marker | undefined {
  if (counted === undefined || counted.key === null) return undefined;
  if (seen.has(counted.key)) return undefined;
  if (counted.blocks > 0)
    return { kind: "count", said: blockedSaid(counted.blocks), key: counted.key };
  if (counted.unverified > 0) return { kind: "dot", key: counted.key };
  return undefined;
}

/**
 * **Each listed chat's blocks since its last look**, as the core counts them: read as the list
 * or a look changes, as the window comes back into focus, and every few seconds while it is
 * drawn. A read that fails keeps what was last read, and so does one that answers the same:
 * the map stays the same object, so an unchanged poll redraws nothing. Each chat's look is
 * found by the key its last count named, so a chat's first read asks from no look.
 */
export function useBlockedMarkers(
  plane: PlaneId,
  sessions: readonly number[],
  seen: ReadonlySet<string> | undefined,
): ReadonlyMap<number, ChatBlocks> {
  const [counted, setCounted] = useState<ReadonlyMap<number, ChatBlocks>>(() => new Map());
  const listed = [...new Set(sessions)].sort((a, b) => a - b).join(",");
  const asked = useMemo(
    () =>
      seen === undefined || listed === ""
        ? []
        : listed.split(",").map((one) => {
            const session = Number(one);
            return { session, after: lastLook(seen, counted.get(session)?.key ?? null) };
          }),
    [listed, seen, counted],
  );
  const asking = JSON.stringify(asked);
  useEffect(() => {
    const these = JSON.parse(asking) as typeof asked;
    if (these.length === 0) return undefined;
    let live = true;
    const read = () => {
      if (document.visibilityState === "hidden") return;
      void commands
        .chatBlocks(plane, these)
        .then((answer) => {
          if (!live || answer.status !== "ok" || !Array.isArray(answer.data)) return;
          const next = new Map(answer.data.map((one) => [one.session, one]));
          setCounted((was) => (sameCounts(was, next) ? was : next));
        })
        .catch(() => undefined);
    };
    read();
    const every = setInterval(read, BLOCKS_READ_EVERY_MS);
    window.addEventListener("focus", read);
    return () => {
      live = false;
      clearInterval(every);
      window.removeEventListener("focus", read);
    };
  }, [plane, asking]);
  return counted;
}

/** Whether two reads of the counts answer the same of every chat. */
function sameCounts(
  was: ReadonlyMap<number, ChatBlocks>,
  now: ReadonlyMap<number, ChatBlocks>,
): boolean {
  if (was.size !== now.size) return false;
  for (const [session, one] of now) {
    const before = was.get(session);
    if (
      before === undefined ||
      before.blocks !== one.blocks ||
      before.unverified !== one.unverified ||
      before.key !== one.key
    )
      return false;
  }
  return true;
}

/** What the window's markers are drawn from: each chat's, and the way to its Chat log. */
export type BlockedMarkersHeld = {
  of: (session: number) => Marker | undefined;
  /**
   * **Opens chat `session`'s Chat log** (R-226), showing `filter`: blocks alone by default, as
   * the marker opens it (R-301). The one way to it from anywhere in the window: the chat row, a
   * Blocked log row, a block result, the session record and the marker all come here, and it
   * presses the chat's own Chat log row, so it opens nothing for a chat the window has no row of.
   */
  open: (session: number, filter?: ChatLogFilter) => void;
  /** Says chat `session`'s blocks were looked at, its Chat log opened: `false` while its
   *  blocks are not counted yet, so there is nothing to look at. */
  look?: (session: number) => boolean;
};

/** The project's markers, for every row and band that draws one, and the way to a chat's Chat
 *  log for every view that names one. None outside a project. */
export const BlockedMarkers = createContext<BlockedMarkersHeld>({
  of: () => undefined,
  open: () => undefined,
});

/** Chat `session`'s marker, and the way to its Chat log. */
export function useBlockedMarker(session: number): {
  marker: Marker | undefined;
  open: () => void;
} {
  const held = useContext(BlockedMarkers);
  return { marker: held.of(session), open: () => held.open(session, "blocks") };
}

/** **Chat `session`'s Chat log opened: its blocks are looked at** (R-175), once, as it opens
 *  (or as its blocks are first counted). A block that comes later marks the chat again. */
export function useBlocksLookedAt(session: number | undefined): void {
  const look = useContext(BlockedMarkers).look;
  const looked = useRef<number | undefined>(undefined);
  useEffect(() => {
    if (session === undefined || looked.current === session) return;
    if (look?.(session) === true) looked.current = session;
  }, [look, session]);
}

/** The Chat log's filters: every row, or the blocks alone. */
export type ChatLogFilter = "all" | "blocks";

/** Each Chat log's filter, by its project and its chat's number: two projects' chat 7 are two. */
const filters = new Map<string, ChatLogFilter>();
const told = new Set<() => void>();

/** The key of project `plane`'s chat `session` in {@link filters}. */
const filterOf = (plane: PlaneId, session: number) => JSON.stringify([plane, session]);

/** **Project `plane`'s chat `session`'s Chat log opens filtered to blocks** (R-301), from its
 *  marker. */
export function filterChatLog(plane: PlaneId, session: number, filter: ChatLogFilter): void {
  filters.set(filterOf(plane, session), filter);
  for (const tell of told) tell();
}

/** The filter project `plane`'s chat `session`'s Chat log shows, and the way to change it. */
export function useChatLogFilter(
  plane: PlaneId,
  session: number | undefined,
): [ChatLogFilter, (filter: ChatLogFilter) => void] {
  const [, redraw] = useState(0);
  useEffect(() => {
    const tell = () => redraw((was) => was + 1);
    told.add(tell);
    return () => {
      told.delete(tell);
    };
  }, []);
  const filter = session === undefined ? "all" : (filters.get(filterOf(plane, session)) ?? "all");
  return [
    filter,
    (to: ChatLogFilter) => {
      if (session !== undefined) filterChatLog(plane, session, to);
    },
  ];
}
