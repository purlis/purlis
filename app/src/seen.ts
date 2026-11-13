// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **New** (#6002, ADR 0121 decisions 10 and 11): an ask, Notice or update the person has not
 * seen yet. Seen, not received: a row is seen once it has been visible in an open view for about
 * a second ({@link SEEN_AFTER_MS}), or once its chat is on screen in a focused window. **An ask
 * from the chat on screen in a focused window is never New**: it is seen as it comes, and still
 * counted.
 *
 * **Kept per machine by the core** (`inbox_seen`, `see_inbox`: `<data>/inbox/<project
 * key>.seen.json`), so it survives a relaunch, by keys this module makes ({@link askSeenKey},
 * {@link noticeSeenKey}, {@link updateSeenKey}). An ask answered, or gone, is forgotten, so the
 * same ask asked again is New. An update the person marked read is not New either.
 *
 * Until the record is read nothing is New: a row drawn with its accent bar and then without it
 * would say something new came that did not.
 */
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { commands, type InboxUpdate, type PlaneId, type Shown } from "./bindings";
import { ASK, askSeenKey, isNewIn, SEEN_AFTER_MS, updateSeenKey, type SeenRows } from "./seenRows";
import { blocksSeenKey } from "./blockedMarkers";

export {
  askSeenKey,
  isNewIn,
  newAsks,
  newNotices,
  newUpdates,
  noticeSeenKey,
  SEEN_AFTER_MS,
  SeenRowsOf,
  updateSeenKey,
  useSeenRow,
  type SeenRows,
} from "./seenRows";

/** How much of a row is in sight for it to count as visible. */
const IN_SIGHT = 0.5;

/**
 * **A project's seen record, as its window draws it.** `asks` are the asks standing now, once
 * read: a seen ask not among them is forgotten. Answers the record (`undefined` until read),
 * the way to say keys were seen, and the {@link SeenRows} its views' rows read.
 */
export function useSeen(
  plane: PlaneId,
  asks: readonly Shown[] | undefined,
): {
  seen: ReadonlySet<string> | undefined;
  see: (keys: readonly string[]) => void;
  rows: SeenRows;
} {
  const [seen, setSeen] = useState<ReadonlySet<string>>();
  // What was last drawn, and said seen since: what `see` leaves out as told already.
  const latest = useRef(seen);
  useLayoutEffect(() => {
    latest.current = seen;
  }, [seen]);
  const told = useCallback((answer: { status: "ok"; data: string[] } | { status: "error" }) => {
    // Only a list is a record: anything else leaves it unread, so nothing is drawn New.
    if (answer.status === "ok" && Array.isArray(answer.data)) setSeen(new Set(answer.data));
  }, []);

  useEffect(() => {
    let live = true;
    void commands
      .inboxSeen(plane)
      .then((answer) => live && told(answer))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [plane, told]);

  const loaded = seen !== undefined;
  /** Says `keys` were seen: drawn so at once, and kept by the core. */
  const see = useCallback(
    (keys: readonly string[]) => {
      const was = latest.current;
      if (!loaded || was === undefined) return;
      const fresh = keys.filter((key) => !was.has(key));
      if (fresh.length === 0) return;
      const now = new Set(was);
      for (const key of fresh) now.add(key);
      latest.current = now;
      setSeen(now);
      void commands
        .seeInbox(plane, fresh, null)
        .then(told)
        .catch(() => undefined);
    },
    // Its identity changes once the record is read, so what waited on it is told again.
    [plane, told, loaded],
  );

  // **An ask answered, or gone, is forgotten** (decision 10): asked again, it is New.
  const standing = useMemo(() => asks?.map(askSeenKey), [asks]);
  useEffect(() => {
    if (seen === undefined || standing === undefined) return;
    const gone = [...seen].some((key) => key.startsWith(ASK) && !standing.includes(key));
    if (!gone) return;
    void commands
      .seeInbox(plane, [], standing)
      .then(told)
      .catch(() => undefined);
  }, [plane, seen, standing, told]);

  const watch = useWatch(see);
  const rows = useMemo<SeenRows>(
    () => ({ isNew: (key) => isNewIn(seen, key), watch }),
    [seen, watch],
  );
  return { seen, see, rows };
}

/** What a key of another project's is known by in {@link useSeenElsewhere}: its project first. */
const ELSEWHERE = "\u0001";

/**
 * **The seen records of other projects, for their asks listed in this one's Asks** (#1921):
 * Asks lists every open project's asks, and each is New until seen, under its own project's
 * record, as it is in that project's own Asks. Each record is read once its project is listed,
 * and a row seen here is told to its project's record. None is forgotten from here: an ask gone
 * is its own project's to forget, from the list it holds.
 */
export function useSeenElsewhere(planes: readonly string[]): (plane: string) => SeenRows {
  const [records, setRecords] = useState<Readonly<Record<string, ReadonlySet<string>>>>({});
  const latest = useRef(records);
  useLayoutEffect(() => {
    latest.current = records;
  }, [records]);
  const told = useCallback(
    (plane: string, answer: { status: "ok"; data: string[] } | { status: "error" }) => {
      if (answer.status !== "ok" || !Array.isArray(answer.data)) return;
      const record = new Set(answer.data);
      latest.current = { ...latest.current, [plane]: record };
      setRecords((was) => ({ ...was, [plane]: record }));
    },
    [],
  );
  const listed = [...new Set(planes)].sort().join("\n");
  useEffect(() => {
    let live = true;
    for (const plane of listed === "" ? [] : listed.split("\n"))
      void commands
        .inboxSeen(plane)
        .then((answer) => live && told(plane, answer))
        .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [listed, told]);
  const see = useCallback(
    (keys: readonly string[]) => {
      const byPlane = new Map<string, string[]>();
      for (const one of keys) {
        const at = one.indexOf(ELSEWHERE);
        if (at < 0) continue;
        const plane = one.slice(0, at);
        byPlane.set(plane, [...(byPlane.get(plane) ?? []), one.slice(at + 1)]);
      }
      for (const [plane, these] of byPlane) {
        const was = latest.current[plane];
        if (was === undefined) continue;
        const fresh = these.filter((key) => !was.has(key));
        if (fresh.length === 0) continue;
        const now = new Set(was);
        for (const key of fresh) now.add(key);
        latest.current = { ...latest.current, [plane]: now };
        setRecords((held) => ({ ...held, [plane]: now }));
        void commands
          .seeInbox(plane, fresh, null)
          .then((answer) => told(plane, answer))
          .catch(() => undefined);
      }
    },
    [told],
  );
  const watch = useWatch(see);
  // One record's rows for each project listed, made once for each reading of the records, so a
  // row's watch is the same from one drawing to the next.
  const made = useMemo(
    () =>
      new Map(
        (listed === "" ? [] : listed.split("\n")).map((plane): [string, SeenRows] => [
          plane,
          {
            isNew: (key) => isNewIn(records[plane], key),
            watch: (element, key) => watch(element, `${plane}${ELSEWHERE}${key}`),
          },
        ]),
      ),
    [listed, records, watch],
  );
  return useCallback((plane: string) => made.get(plane) ?? NOTHING_SEEN, [made]);
}

/** A project's rows whose record is not read here: nothing is New. */
const NOTHING_SEEN: SeenRows = { isNew: () => false, watch: () => () => undefined };

/**
 * **Seen once in sight for about a second** (decision 10): one observer for every row of the
 * project, each row's timer started as it comes into sight and stopped as it leaves. A webview
 * with no observer sees nothing this way: a row is then seen only by its chat.
 */
function useWatch(see: (keys: readonly string[]) => void) {
  const seeing = useRef(see);
  useLayoutEffect(() => {
    seeing.current = see;
  }, [see]);
  const state = useRef<{
    observer: IntersectionObserver | undefined;
    keys: Map<Element, string>;
    timers: Map<Element, ReturnType<typeof setTimeout>>;
  }>({ observer: undefined, keys: new Map(), timers: new Map() });

  useEffect(() => {
    const held = state.current;
    return () => {
      held.observer?.disconnect();
      held.observer = undefined;
      for (const timer of held.timers.values()) clearTimeout(timer);
      held.timers.clear();
    };
  }, []);

  return useCallback((element: Element, key: string) => {
    const { keys, timers } = state.current;
    const stop = (one: Element) => {
      const timer = timers.get(one);
      if (timer !== undefined) clearTimeout(timer);
      timers.delete(one);
    };
    if (state.current.observer === undefined && typeof IntersectionObserver !== "undefined") {
      state.current.observer = new IntersectionObserver(
        (entries) => {
          for (const entry of entries) {
            const one = entry.target;
            const seenKey = keys.get(one);
            if (seenKey === undefined) continue;
            if (!entry.isIntersecting || entry.intersectionRatio < IN_SIGHT) {
              stop(one);
              continue;
            }
            if (timers.has(one)) continue;
            timers.set(
              one,
              setTimeout(() => {
                timers.delete(one);
                // A window that is not drawn shows nothing: its rows wait to be seen.
                if (document.visibilityState === "hidden") return;
                seeing.current([seenKey]);
              }, SEEN_AFTER_MS),
            );
          }
        },
        { threshold: [0, IN_SIGHT, 1] },
      );
    }
    keys.set(element, key);
    state.current.observer?.observe(element);
    return () => {
      keys.delete(element);
      stop(element);
      state.current.observer?.unobserve(element);
    };
  }, []);
}

/**
 * **What the chat on screen has to say is seen** (decision 10), and **its asks are never New**
 * (decision 11): while the window has the keyboard and this project is in front, every ask and
 * update of chat `session` is seen as it comes, before it is painted, and again each time the
 * window comes back into focus.
 *
 * **And its blocks are looked at** (R-175, R-341; T1-30): `blocks` holds each chat's latest
 * block, and the one of the chat on screen is seen by this same rule, so its blocked marker
 * clears and a block that comes while it is on screen never marks it.
 */
export function useChatOnScreenSeen(
  see: (keys: readonly string[]) => void,
  session: number | undefined,
  shown: boolean,
  asks: readonly Shown[] | undefined,
  updates: readonly InboxUpdate[] | undefined,
  blocks?: ReadonlyMap<number, string>,
): void {
  const latest = session === undefined ? undefined : blocks?.get(session);
  const keys = useMemo(
    () =>
      session === undefined
        ? []
        : [
            ...(asks ?? []).filter((ask) => ask.session === session).map(askSeenKey),
            ...(updates ?? [])
              .filter((one) => one.session === session)
              .map((one) => updateSeenKey(one.key)),
            ...(latest === undefined ? [] : [blocksSeenKey(session, latest)]),
          ],
    [asks, updates, session, latest],
  );
  useLayoutEffect(() => {
    if (!shown || keys.length === 0) return undefined;
    const told = () => {
      if (document.hasFocus()) see(keys);
    };
    told();
    window.addEventListener("focus", told);
    return () => window.removeEventListener("focus", told);
  }, [see, keys, shown]);
}
