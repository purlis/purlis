// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a project's chats told the window, as the core sends it: one batch about every
 * 100 ms** (#4746, part of #4724; `chatevents.rs`).
 *
 * A chat moving, saying what it is doing, touching a file or counting a skill call was once an
 * event of its own, and every event is a hop onto the window's main thread. The core now queues
 * them and sends `chat-events`, every one of a project's tellings in the order they came. Each
 * store listens for the kind it keeps and is handed those in that order, all in the one task
 * the batch arrived in, so the stores that tell their readers once a frame (`heldStore.ts`)
 * draw a whole batch in one commit.
 */
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { ChatEvent, ChatEvents } from "./bindings";
import { listen } from "./here";

/** The event the core sends: a {@link ChatEvents}. */
export const CHAT_EVENTS = "chat-events";

/** Each kind of telling, and what it carries. */
export type ChatEventKind = ChatEvent["kind"];
export type ToldOf<K extends ChatEventKind> = Extract<ChatEvent, { kind: K }>["told"];

/** Calls `heard` with every telling of kind `kind` in each batch, in the order they came. */
export function listenForChats<K extends ChatEventKind>(
  kind: K,
  heard: (told: ToldOf<K>) => void,
): Promise<UnlistenFn> {
  return listen<ChatEvents>(CHAT_EVENTS, (event) => {
    for (const one of event.payload.events) if (one.kind === kind) heard(one.told as ToldOf<K>);
  });
}
