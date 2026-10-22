// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The key that sends a form's primary action** (N26, #2905): ⌘⏎ on a Mac, Ctrl+Enter
 * everywhere else, from anywhere in the form, a textarea included, where a plain Enter starts a
 * new line. The family's `Form` answers it; a surface whose primary action is not a form's submit
 * (Start a chat) asks `submits` itself.
 *
 * **It takes nothing from a chat** (`docs/ui-primitives.md`, "Which keys belong to the chat"): it
 * is answered only inside a form or a dialog's own fields, never in a terminal pane, and an Enter
 * an input method is still composing with is the input method's.
 */
export function submits(
  event: Pick<
    globalThis.KeyboardEvent,
    "key" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey" | "isComposing"
  >,
  mac: boolean = onAMac(),
): boolean {
  if (event.key !== "Enter" || event.isComposing || event.altKey || event.shiftKey) return false;
  return mac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}

/** How a hint spells the key, on this platform. */
export function submitKeySaid(mac: boolean = onAMac()): string {
  return mac ? "⌘↩" : "Ctrl+Enter";
}

function onAMac(): boolean {
  return typeof navigator !== "undefined" && navigator.platform.startsWith("Mac");
}
