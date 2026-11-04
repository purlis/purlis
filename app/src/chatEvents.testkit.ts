// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ChatEvent, ChatEvents } from "./bindings";
import { CHAT_EVENTS, type ChatEventKind, type ToldOf } from "./chatEvents";

/** The four events a chat's telling was sent as before `chat-events` (#4746), by its kind. */
const KINDS = {
  "chat-moved": "moved",
  "chat-doing": "doing",
  "chat-touching": "touching",
  "chat-skill-run": "skill-run",
} as const satisfies Record<string, ChatEventKind>;

/** A batch carrying only `told`, as the core sends a telling that came on its own. */
export function aBatchOf<K extends ChatEventKind>(
  kind: K,
  told: ToldOf<K> & { plane: string },
): ChatEvents {
  return { plane: told.plane, events: [{ kind, told } as ChatEvent] };
}

/**
 * **What the window is sent for one telling**, by the name of the event it was sent as on its
 * own before (#4746): `chat-events`, with a batch of that one. Any other event goes as it is.
 * For a test whose wire sends `(event, payload)`.
 */
export function onTheWire(event: string, payload: unknown): [string, unknown] {
  const kind = KINDS[event as keyof typeof KINDS] as ChatEventKind | undefined;
  if (kind === undefined) return [event, payload];
  return [CHAT_EVENTS, aBatchOf(kind, payload as ToldOf<typeof kind> & { plane: string })];
}
