// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * **The Dock badge: the ✋'s total** (#5999, ADR 0121 decision 3): every open project's Blocking
 * asks, on the app's icon in the Dock on macOS and on the launcher's icon where a Linux desktop
 * shows one, and nothing at all at 0. Tauri's own badge, as its window API sets it.
 *
 * `count` is undefined while the asks have not been read yet, and the badge is left as it is.
 * Each project window sets the same total, since each counts the others' asks as well.
 */
export function useDockBadge(count: number | undefined): void {
  useEffect(() => {
    if (count === undefined) return;
    let here;
    try {
      here = getCurrentWindow();
    } catch {
      // Outside a Tauri window (a page in a plain browser) there is no icon to badge.
      return;
    }
    // A platform with no badge (Windows, a Linux desktop without one) answers an error: the ✋
    // still counts, so nothing is lost.
    void here.setBadgeCount(count > 0 ? count : undefined).catch(() => undefined);
  }, [count]);
}
