// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * Whether the keyboard in front of the operator is a Mac's.
 *
 * From the page and not asked of the binary, unlike the title bar's room (`bindings.ts`,
 * `titleBarRoom`): that one decides a layout and has to be exact, and this decides one key,
 * where the platform string WebKit reports on the machine it runs on is the answer. A module of
 * its own, so a page that may read nothing of the window's (the theme specimen) can ask it too.
 */
export function onAMac(): boolean {
  return navigator.platform.startsWith("Mac");
}
