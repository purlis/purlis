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
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type RefObject,
} from "react";
import { commands, type InboxUpdate, type PlaneId, type Shown } from "./bindings";

/** How long a row is in sight in an open view before it has been seen: about a second. */
export const SEEN_AFTER_MS = 1000;

/** How much of a row is in sight for it to count as visible. */
const IN_SIGHT = 0.5;

/** What every ask's key starts with: the core lets go of those no longer standing. */
const ASK = "ask:";

/** An ask's key: its chat's, and its own, unique in its project. */
export const askSeenKey = (ask: Pick<Shown, "session" | "ask">) =>
  `${ASK}${ask.session}:${ask.ask}`;

/** A Notice's key: its cause, what its dismissal is keyed by too. */
export const noticeSeenKey = (cause: string) => `notice:${cause}`;

/** An update's key: its own, which the core keeps it by. */
export const updateSeenKey = (key: string) => `update:${key}`;

/** Whether `key` is New under `seen`: nothing is, until the record is read. */
export const isNewIn = (seen: ReadonlySet<string> | undefined, key: string) =>
  seen !== undefined && !seen.has(key);

/** The asks of `asks` that are New. */
export const newAsks = (
  asks: readonly Shown[] | undefined,
  seen: ReadonlySet<string> | undefined,
) => (asks ?? []).filter((ask) => isNewIn(seen, askSeenKey(ask))).length;

/** The updates of `updates` that are New: not seen, and not marked read. */
export const newUpdates = (
  updates: readonly InboxUpdate[] | undefined,
  seen: ReadonlySet<string> | undefined,
) => (updates ?? []).filter((one) => !one.read && isNewIn(seen, updateSeenKey(one.key))).length;

/** The Notices of `causes` that are New. */
export const newNotices = (causes: readonly string[], seen: ReadonlySet<string> | undefined) =>
  causes.filter((cause) => isNewIn(seen, noticeSeenKey(cause))).length;

/** What a row of the Inbox's views asks of the seen record: {@link useSeenRow}. */
export interface SeenRows {
  isNew: (key: string) => boolean;
  /** Watches `element` for `key`: seen once it has been in sight for {@link SEEN_AFTER_MS}.
   *  Answers the way to stop. */
  watch: (element: Element, key: string) => () => void;
}

/** The seen record of the project whose views are drawn below. None outside one: nothing New. */
export const SeenRowsOf = createContext<SeenRows | null>(null);

/**
 * **A row of an Inbox view**: whether it is New, and its element watched while it is, so it is
 * seen once it has been in sight for about a second. `drawn` is the row's own element.
 */
export function useSeenRow(key: string, drawn: RefObject<Element | null>): boolean {
  const rows = useContext(SeenRowsOf);
  const isNew = rows?.isNew(key) ?? false;
  useEffect(() => {
    const element = drawn.current;
    if (rows === null || !isNew || element === null) return undefined;
    return rows.watch(element, key);
  }, [rows, isNew, key, drawn]);
  return isNew;
}

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
    if (answer.status === "ok") setSeen(new Set(answer.data));
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
 */
export function useChatOnScreenSeen(
  see: (keys: readonly string[]) => void,
  session: number | undefined,
  shown: boolean,
  asks: readonly Shown[] | undefined,
  updates: readonly InboxUpdate[] | undefined,
): void {
  const keys = useMemo(
    () =>
      session === undefined
        ? []
        : [
            ...(asks ?? []).filter((ask) => ask.session === session).map(askSeenKey),
            ...(updates ?? [])
              .filter((one) => one.session === session)
              .map((one) => updateSeenKey(one.key)),
          ],
    [asks, updates, session],
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
