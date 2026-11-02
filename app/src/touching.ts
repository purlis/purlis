// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * Which files the chats are touching right now (FM-6, #1109; V86 F6).
 *
 * A chat's file tool — a read, an edit, a write — names its path to the app, which confines it
 * to the chat's own folder, rates it, and sends `chat-touching`. This keeps each one for a few
 * seconds and lets it fade once the chat has been quiet on it for {@link FADE_MS}, so the tree
 * shows the present and not a history. **In memory only**: nothing here is saved, and the core
 * wrote it nowhere either (D-86a).
 *
 * Bounded twice, so a chat that touches files as fast as it can costs the window a fixed amount:
 * at most {@link AT_MOST_PER_CHAT} paths per chat (its oldest goes first), and at most
 * {@link AT_MOST} in all. The core already caps each chat's rate.
 */
import { useEffect, useState, useSyncExternalStore } from "react";
import { useSyncExternalStoreWithSelector } from "use-sync-external-store/with-selector";
import { listen } from "./here";
import { useProjectShown } from "./projectShown";
import type { ChatTouching, OpenChat, PlaneId } from "./bindings";

/** How long a touched file stays marked after its chat last touched it. */
export const FADE_MS = 4000;

/** The most paths one chat has marked at once. */
export const AT_MOST_PER_CHAT = 8;

/** The most paths marked at once, across every chat. */
export const AT_MOST = 256;

/** One file a chat touched, and when this window last heard so, in ms. */
export type Touch = { readonly session: number; readonly path: string; readonly at: number };

/** Every file being touched, oldest first. */
export type Touches = readonly Touch[];

/** `touches` with chat `session`'s touch of `path` at `now` as its newest, within the bounds. */
export function touched(touches: Touches, session: number, path: string, now: number): Touches {
  const others = touches.filter((one) => one.session !== session || one.path !== path);
  let next = [...others, { session, path, at: now }];
  const mine = next.filter((one) => one.session === session);
  if (mine.length > AT_MOST_PER_CHAT) {
    const dropped = new Set(mine.slice(0, mine.length - AT_MOST_PER_CHAT));
    next = next.filter((one) => !dropped.has(one));
  }
  return next.length > AT_MOST ? next.slice(next.length - AT_MOST) : next;
}

/** `touches` without what has faded by `now`: the same list when nothing has. */
export function faded(touches: Touches, now: number): Touches {
  const kept = touches.filter((one) => now - one.at < FADE_MS);
  return kept.length === touches.length ? touches : kept;
}

/** When the next touch fades, or nothing when none is held. */
export function nextFade(touches: Touches): number | undefined {
  if (touches.length === 0) return undefined;
  return Math.min(...touches.map((one) => one.at)) + FADE_MS;
}

/** What a {@link TouchStore} holds: the touches, with the project they are of. */
type Held = { readonly plane?: PlaneId; readonly touches: Touches };

/**
 * **The files a project's chats are touching, held outside React** (#4730), as `chatDoing.ts`
 * holds what they are doing: a touch tells the rows that read it, and each draws again only when
 * what it shows changed ({@link useTouchedBy}), so an agent touching files as fast as it can
 * redraws its file's row and the folders above it, not the tree.
 */
export type TouchStore = {
  subscribe: (listener: () => void) => () => void;
  /** What is held now. Read through {@link touchesOf}, which answers for one project. */
  held: () => Held;
};

type OwnStore = TouchStore & {
  /** Holds `touches` of `plane`, and sets the timer for the next of them to fade. */
  hold: (plane: PlaneId | undefined, touches: Touches) => void;
};

function touchStore(): OwnStore {
  let held: Held = { touches: NONE };
  let timer: ReturnType<typeof setTimeout> | undefined;
  const listeners = new Set<() => void>();
  const store: OwnStore = {
    subscribe: (listener) => {
      listeners.add(listener);
      return () => void listeners.delete(listener);
    },
    held: () => held,
    hold: (plane, touches) => {
      if (plane === held.plane && touches === held.touches) return;
      held = { plane, touches };
      // One timer, for the next touch to fade; set again whenever the list changes.
      if (timer !== undefined) clearTimeout(timer);
      timer = undefined;
      const due = nextFade(touches);
      if (due !== undefined)
        timer = setTimeout(
          () => store.hold(plane, faded(store.held().touches, Date.now())),
          Math.max(0, due - Date.now()),
        );
      for (const listener of [...listeners]) listener();
    },
  };
  return store;
}

/** What `held` says of `plane`: nothing at all of any other project. */
export function touchesOf(held: Held, plane: PlaneId | undefined): Touches {
  return held.plane === plane ? held.touches : NONE;
}

/**
 * Keeps the files `plane`'s chats are touching, fading as they go quiet. It answers the store
 * and not the touches, so the component holding it is not drawn again by one.
 *
 * **Only while the project is in front** (`useProjectShown`): a project nobody can see draws no
 * mark, so it does no work for its agents' touches and listens for none. A mark lasts a few
 * seconds, so one that came while it was away would mostly have faded by its return anyway.
 */
export function useTouches(plane: PlaneId | undefined): TouchStore {
  const [store] = useState(touchStore);
  const shown = useProjectShown();
  useEffect(() => {
    if (plane === undefined || !shown) return;
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<ChatTouching>("chat-touching", (event) => {
          if (gone || event.payload.plane !== plane) return;
          const { session, path } = event.payload;
          const now = Date.now();
          const from = touchesOf(store.held(), plane);
          store.hold(plane, touched(faded(from, now), session, path, now));
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in: a unit test, or a webview being torn down.
      }
    })();
    return () => {
      gone = true;
      stop?.();
      // What it held goes with it, and its timer: the project left the front or the window
      // moved to another, and either way nothing marks them now.
      store.hold(plane, NONE);
    };
  }, [store, plane, shown]);
  return store;
}

/** The files `plane`'s chats are touching, as a value: whatever reads it is drawn again on each
 *  touch. For a part that needs the list itself; a row reads {@link useTouchedBy}. */
export function useTouching(plane: PlaneId | undefined): Touches {
  return useHeldTouches(useTouches(plane), plane);
}

/** What `store` holds of `plane`, as a value: whatever reads it is drawn again on each touch. */
export function useHeldTouches(store: TouchStore, plane: PlaneId | undefined): Touches {
  const touches = () => touchesOf(store.held(), plane);
  return useSyncExternalStore(store.subscribe, touches, touches);
}

/**
 * What a tree's rows read their marks out of: the store, and what its touches mark in each
 * branch by {@link branchKey}-style key, worked out once per touch and branch however many rows
 * ask (`marksOf`).
 */
export type TouchingOf = {
  readonly store: TouchStore;
  /** What `touches` mark in the branch `key` names, or nothing. Called with the same touches
   *  by every row; the answer is remembered for as long as they are the ones held. */
  readonly marksOf: (touches: Touches, key: string) => Touching | undefined;
  readonly plane: PlaneId | undefined;
};

/** A {@link TouchingOf} whose `marks` answers per branch, remembered per list of touches. */
export function touchingOf(
  store: TouchStore,
  plane: PlaneId | undefined,
  marks: (touches: Touches, key: string) => Touching | undefined,
): TouchingOf {
  let of: Touches | undefined;
  let known = new Map<string, Touching | undefined>();
  return {
    store,
    plane,
    marksOf: (touches, key) => {
      if (touches !== of) {
        of = touches;
        known = new Map();
      }
      if (!known.has(key)) known.set(key, marks(touches, key));
      return known.get(key);
    },
  };
}

/**
 * Which chats are touching `path` of the branch `key` names, and a redraw only when that
 * changes: a touch elsewhere, and a touch again of a file already marked by the same chats,
 * redraw nothing here.
 */
export function useTouchedBy(
  touching: TouchingOf | undefined,
  key: string,
  path: string,
): readonly string[] | undefined {
  const store = touching?.store ?? QUIET;
  const held = () => store.held();
  return useSyncExternalStoreWithSelector(
    store.subscribe,
    held,
    held,
    (now) =>
      touching === undefined
        ? undefined
        : touching.marksOf(touchesOf(now, touching.plane), key)?.get(path),
    sameNames,
  );
}

function sameNames(one: readonly string[] | undefined, other: readonly string[] | undefined) {
  return (
    one === other ||
    (one !== undefined &&
      other !== undefined &&
      one.length === other.length &&
      one.every((name, at) => name === other[at]))
  );
}

/** A store that never holds a touch, for a tree drawn without any. */
const QUIET: TouchStore = { subscribe: () => () => {}, held: () => ({ touches: NONE }) };

/** No files touched. */
const NONE: Touches = [];

/** The chats touching each path of one branch, by the path inside it. */
export type Touching = ReadonlyMap<string, readonly string[]>;

/** A path with `/` between its parts, whichever the platform wrote, and no `/` at its end. */
function slashed(path: string): string {
  return path.replace(/\\/g, "/").replace(/\/+$/, "");
}

/**
 * Which paths of the branch whose folder is `folder` are being touched, and by which chats, by
 * name: each file touched, each folder above it, and `""` for the branch's own folder.
 *
 * A touch's path is inside its chat's folder (the core confined it), so it is placed under the
 * branch by that folder: a chat working in a branch, or in a folder holding it, marks the files
 * of it it touches, and no other.
 */
export function touchingIn(
  touches: Touches,
  chats: readonly OpenChat[],
  folder: string | undefined,
): Touching {
  const out = new Map<string, string[]>();
  if (folder === undefined || touches.length === 0) return out;
  const top = slashed(folder);
  const bySession = new Map(chats.map((chat) => [chat.session, chat]));
  for (const touch of touches) {
    const chat = bySession.get(touch.session);
    if (chat?.cwd == null) continue;
    const whole = `${slashed(chat.cwd)}/${touch.path}`;
    if (!whole.startsWith(`${top}/`)) continue;
    const inside = whole.slice(top.length + 1);
    const parts = inside.split("/");
    const places = parts.map((_, at) => parts.slice(0, at + 1).join("/"));
    for (const place of ["", ...places]) {
      const names = out.get(place) ?? [];
      if (!names.includes(chat.name)) names.push(chat.name);
      out.set(place, names);
    }
  }
  return out;
}

/** What a marker says on hover and to a screen reader. */
export function touchSaid(names: readonly string[]): string {
  const who =
    names.length === 1
      ? names[0]
      : `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
  return `${who} ${names.length === 1 ? "is" : "are"} working here now`;
}
