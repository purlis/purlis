// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { commands, type BrowsersListed, type ChatBrowser } from "./bindings";
import { listen } from "./here";

/**
 * **The browsers of a project's chats, as the window draws them** (#2024, spec #2008): under
 * each chat in the Chats list, live on its chat's tab, and in a browser view tab once opened.
 *
 * A chat's browser runs in the session host from the chat's first allowed browser call
 * (`chatbrowsers.rs`, #2013), and ends with its chat, unless the person pinned it (#2022): then
 * it outlives its chat, listed apart under Pinned browsers by what its chat was called, until
 * the person stops it, hands it to another chat, or Stop all or a quit ends it. The core says
 * when a project's chats' browsers come or go (`chat-browsers-listed`), and the window reads
 * them again.
 */

/** The event a project's chats' browsers are said to have come or gone on. */
export const LISTED = "chat-browsers-listed";

/** A project's chats' browsers, by chat, each chat's oldest first. */
export type ByChat = ReadonlyMap<number, readonly ChatBrowser[]>;

/** No chat has a browser. */
export const NO_BROWSERS: ByChat = new Map();

/** `listed`, by chat: those a chat uses. A pinned one whose chat closed is {@link keptOf}'s. */
export function byChat(listed: readonly ChatBrowser[]): ByChat {
  const by = new Map<number, ChatBrowser[]>();
  for (const one of listed)
    if (one.was === null) by.set(one.session, [...(by.get(one.session) ?? []), one]);
  return by;
}

/** The pinned browsers of `listed` whose chat has closed (#2022): the person's to stop. */
export function keptOf(listed: readonly ChatBrowser[]): readonly ChatBrowser[] {
  return listed.filter((one) => one.was !== null);
}

/** No pinned browser outlived its chat. */
export const NONE_KEPT: readonly ChatBrowser[] = [];

/**
 * Every program closing a tab must not end: those the chats of `browsers` use, which end with
 * their chats, and each of `kept`, pinned past its chat, which ends when the person stops it.
 * One that could not be ended yet is no chat's, and closing a tab that shows it may end it.
 */
/**
 * **The pool whose runner `program` runs on, as the chats' browsers list it** (#2050): `null` for
 * one on this machine, and `undefined` for one they do not list.
 */
export function poolOf(
  browsers: ByChat,
  kept: readonly ChatBrowser[],
  program: string,
): string | null | undefined {
  const listed = [...[...browsers.values()].flat(), ...kept].find((one) => one.program === program);
  return listed === undefined ? undefined : (listed.pool ?? null);
}

export function programsOf(
  browsers: ByChat,
  kept: readonly ChatBrowser[] = NONE_KEPT,
): Set<string> {
  return new Set([
    ...[...browsers.values()].flatMap((list) =>
      list.flatMap((one) => (one.unended ? [] : [one.program])),
    ),
    ...kept.map((one) => one.program),
  ]);
}

/**
 * **The browsers of project `plane`'s chats**, read as the project opens and again each time the
 * core says they came or went. `read` is false until the first reading is in, so what was
 * running before the window looked is told apart from what starts after.
 */
export function useChatBrowsers(plane: string): {
  browsers: ByChat;
  kept: readonly ChatBrowser[];
  read: boolean;
} {
  const [held, setHeld] = useState<{
    plane: string;
    browsers: ByChat;
    kept: readonly ChatBrowser[];
    read: boolean;
  }>({
    plane,
    browsers: NO_BROWSERS,
    kept: NONE_KEPT,
    read: false,
  });
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    // Only the latest reading is kept: one that answers after a later one is old.
    let asked = 0;
    const read = () => {
      asked += 1;
      const mine = asked;
      void commands
        .chatBrowsers(plane)
        .then((answer) => {
          if (gone || mine !== asked || answer.status === "error") return;
          const listed = Array.isArray(answer.data) ? answer.data : [];
          setHeld({ plane, browsers: byChat(listed), kept: keptOf(listed), read: true });
        })
        // A window whose core cannot list them draws none, and draws everything else.
        .catch(() => undefined);
    };
    read();
    void (async () => {
      try {
        const unlisten = await listen<BrowsersListed>(LISTED, (event) => {
          if (!gone && event.payload.plane === plane) read();
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane]);
  return held.plane === plane
    ? { browsers: held.browsers, kept: held.kept, read: held.read }
    : { browsers: NO_BROWSERS, kept: NONE_KEPT, read: false };
}

/**
 * **The person's own browsers a crash of the window left running** (#2025): the core takes them
 * back as the project's chats come back, and each is handed to `open` once, for a view tab of its
 * own, once `open` is given and each time the core says browsers came or went. A chat's browsers
 * come back with their chats, under them in the Chats list ({@link useChatBrowsers}).
 */
export function useBrowsersTakenBack(
  plane: string,
  open: ((program: string) => void) | undefined,
): void {
  // The latest `open`, so a new one does not listen again.
  const opening = useRef(open);
  useEffect(() => {
    opening.current = open;
  }, [open]);
  const ready = open !== undefined;
  useEffect(() => {
    // Nothing is taken until the window can open a tab that stays.
    if (!ready) return;
    let gone = false;
    let stop: (() => void) | undefined;
    const take = () => {
      void commands
        .browsersTakenBack(plane)
        // Opened even where this effect has been cleaned up since it asked: the core hands each
        // over once, and one dropped would run with no tab (a StrictMode remount asks twice).
        .then((answer) => {
          if (answer.status === "error") return;
          for (const program of answer.data) opening.current?.(program);
        })
        // A window whose core cannot say opens none.
        .catch(() => undefined);
    };
    take();
    void (async () => {
      try {
        const unlisten = await listen<BrowsersListed>(LISTED, (event) => {
          if (!gone && event.payload.plane === plane) take();
        });
        // Let go of quietly: a window going away may no longer have its events to let go of.
        const letGo = () => void Promise.resolve(unlisten()).catch(() => undefined);
        if (gone) letGo();
        else stop = letGo;
      } catch {
        // No window to listen in.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane, ready]);
}
