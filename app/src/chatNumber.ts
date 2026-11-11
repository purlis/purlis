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
