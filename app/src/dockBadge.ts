// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * **The Dock badge's number: every open project's Blocking asks** (#5999, #6341), in every
 * window, as `askedIn` counts each. The ✋ counts only the project in front (ADR 0121 amended
 * 2026-11-12, decision 1); the Dock is under no one project, so it keeps the total.
 */
export const dockTotal = (asked: ReadonlyMap<string, number>): number =>
  [...asked.values()].reduce((sum, count) => sum + count, 0);

/**
 * **The Dock badge** (#5999, ADR 0121 decision 3, amended 2026-11-12): every open project's
 * Blocking asks ({@link dockTotal}), on the app's icon in the Dock on macOS and on the launcher's
 * icon where a Linux desktop shows one, and nothing at all at 0. Tauri's own badge, as its
 * window API sets it.
 *
 * `asked` is undefined while the asks have not been read yet, and the badge is left as it is.
 * Each project window sets the same total, since each counts the others' asks as well.
 */
export function useDockBadge(asked: ReadonlyMap<string, number> | undefined): void {
  const count = asked === undefined ? undefined : dockTotal(asked);
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
