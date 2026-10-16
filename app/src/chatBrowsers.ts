// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type BrowsersListed, type ChatBrowser } from "./bindings";
import { listen } from "./here";

/**
 * **The browsers of a project's chats, as the window draws them** (#2024, spec #2008): under
 * each chat in the Chats list, live on its chat's tab, and in a browser view tab once opened.
 *
 * A chat's browser runs in the session host from the chat's first allowed browser call
 * (`chatbrowsers.rs`, #2013), and ends with its chat. The core says when a project's chats'
 * browsers come or go (`chat-browsers-listed`), and the window reads them again.
 */

/** The event a project's chats' browsers are said to have come or gone on. */
export const LISTED = "chat-browsers-listed";

/** A project's chats' browsers, by chat, each chat's oldest first. */
export type ByChat = ReadonlyMap<number, readonly ChatBrowser[]>;

/** No chat has a browser. */
export const NO_BROWSERS: ByChat = new Map();

/** `listed`, by chat. */
export function byChat(listed: readonly ChatBrowser[]): ByChat {
  const by = new Map<number, ChatBrowser[]>();
  for (const one of listed) by.set(one.session, [...(by.get(one.session) ?? []), one]);
  return by;
}

/**
 * Every program the chats of `browsers` use: those end with their chats. One that could not be
 * ended yet is no chat's, and closing a tab that shows it may end it.
 */
export function programsOf(browsers: ByChat): Set<string> {
  return new Set(
    [...browsers.values()].flatMap((list) =>
      list.flatMap((one) => (one.unended ? [] : [one.program])),
    ),
  );
}

/**
 * **The browsers of project `plane`'s chats**, read as the project opens and again each time the
 * core says they came or went. `read` is false until the first reading is in, so what was
 * running before the window looked is told apart from what starts after.
 */
export function useChatBrowsers(plane: string): { browsers: ByChat; read: boolean } {
  const [held, setHeld] = useState<{ plane: string; browsers: ByChat; read: boolean }>({
    plane,
    browsers: NO_BROWSERS,
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
          setHeld({ plane, browsers: byChat(listed), read: true });
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
    ? { browsers: held.browsers, read: held.read }
    : { browsers: NO_BROWSERS, read: false };
}
