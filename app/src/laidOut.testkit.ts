// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { LaidOut, LaidTab } from "./bindings";
import { chatsOf } from "./laidOut";

/**
 * **What the core takes from a layout the window told it** (#4751), for a window test that
 * stands in for the core: `Layout::on_screen` and `Layout::lead` in
 * `crates/purlis-core/src/reopen/layout.rs`, whose own tests pin the real ones. The window
 * itself derives neither: what is on screen is the core's to say.
 */

function frontTabs(laid: LaidOut): LaidTab[] {
  return laid.tabs.filter((tab) => tab.front && (tab.window !== 0 || tab.workspace === laid.strip));
}

/** Every chat in a pane of the front tab of the strip drawn, and of each tab window's front
 *  tab, in the order of their numbers. */
export function onScreenOf(laid: LaidOut): number[] {
  return [...new Set(frontTabs(laid).flatMap((tab) => chatsOf(tab.tree)))].sort((a, b) => a - b);
}

/** The first chat in the project window's front tab on the strip drawn. */
export function leadOf(laid: LaidOut): number | undefined {
  const tab = frontTabs(laid).find((one) => one.window === 0);
  return tab === undefined ? undefined : chatsOf(tab.tree)[0];
}

/**
 * **A stored layout giving each chat a tab of its own** (#4758), for a window test whose
 * stand-in core opens chats: since the switch a chat gets a tab only from the stored layout,
 * so a test about something else that wants its chats on the strip says so with this. Each chat
 * is on `workspaceOf` its session's strip (`null` outside every workspace), in the order given;
 * the one `in_front` is its strip's front tab and its strip is drawn, and every other strip's
 * front tab is its first.
 */
export function aTabForEachChat(
  chats: readonly { session: number; in_front: boolean }[],
  workspaceOf: (session: number) => string | null = () => null,
): LaidOut {
  const front = chats.find((chat) => chat.in_front);
  const strip = front === undefined ? null : workspaceOf(front.session);
  const fronted = new Set<string | null>(front === undefined ? [] : [strip]);
  const tabs = chats.map((chat): LaidTab => {
    const workspace = workspaceOf(chat.session);
    let isFront = chat === front;
    if (!fronted.has(workspace)) {
      fronted.add(workspace);
      isFront = true;
    }
    return {
      workspace,
      pinned: false,
      window: 0,
      front: isFront,
      focused: "",
      tree: { kind: "chat", chat: chat.session },
    };
  });
  return { strip, tabs };
}
