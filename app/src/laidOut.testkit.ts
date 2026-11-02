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
