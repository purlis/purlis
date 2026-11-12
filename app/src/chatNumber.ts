// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A chat's number, as the window says it** (#5820): `#641`. It is the chat's current number,
 * the session the window shows it by, so a chat started again is said by its new one. Written
 * once, for its tab's hover and its pane's head (#6212) and the switcher's search (#6213).
 */
export function chatNumberSaid(number: number): string {
  return `#${number}`;
}

/** A chat by its number, then its name: `#641 · steward 4`, its tab's hover (#6212). */
export function numberedName(number: number, name: string): string {
  return `${chatNumberSaid(number)} · ${name}`;
}

/**
 * **The chat number a search asks for** (#6213): `641` or `#641`, spaces round it ignored, or
 * nothing for any other words. Only a whole number is one: `#64` asks for chat 64, never 641.
 */
export function chatNumberIn(query: string): number | undefined {
  const found = /^#?(\d+)$/.exec(query.trim());
  return found === null ? undefined : Number(found[1]);
}
