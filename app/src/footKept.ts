// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ChatUsage, HarnessGlance } from "./bindings";
import type { ChatHead } from "./paneHead";

/**
 * **What a pane foot last knew of a chat, kept for when it has ended** (#5205, spec #5191 stories 23
 * and 24). The core lets go of a chat closed on purpose, and a pane is drawn anew each time its tab
 * comes back or its side unfolds (story 26), so what the foot says of what has ended lives here and
 * not in the pane. Only for this launch, as the ended band that says the chat ended is. One small
 * entry per chat a pane has shown, so it is bounded by the chats the window has held.
 */
export type FootFacts = { head?: ChatHead; harness?: HarnessGlance; usage?: ChatUsage };

const kept = new Map<string, FootFacts>();

const keyOf = (plane: string, session: number) => `${plane}\u0000${session}`;

/** What is known of chat `session` of `plane` now, each fact over the one last known. */
export function keepFoot(plane: string, session: number, now: FootFacts): void {
  const was = kept.get(keyOf(plane, session));
  kept.set(keyOf(plane, session), {
    head: now.head ?? was?.head,
    harness: now.harness ?? was?.harness,
    usage: now.usage ?? was?.usage,
  });
}

/** What was last known of chat `session` of `plane`: nothing for one no pane has shown. */
export function footKept(plane: string, session: number): FootFacts {
  return kept.get(keyOf(plane, session)) ?? {};
}

/** Test-only: forgets what was kept, as a new launch has nothing kept. */
export function forgetFootKept(): void {
  kept.clear();
}
