// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { layoutPref } from "./layoutPref";
import type { PaneNoticesShown } from "./paneNotices";

/**
 * **Minimised or expanded, kept** (#4754, `paneNotices.ts`): one choice for every pane,
 * minimised by default, in the layout file under `notices` (`layout.json`, `regions.ts`), beside
 * the window's other choices on this machine. Only `expanded` is written: minimised is what the
 * file leaves out. The pane (`LayoutPanes.tsx`) reads it and hands its band what it says.
 */

/** The choice as a layout document holds it, and what had to be put right. */
export function loadPaneNotices(raw: unknown): { value: PaneNoticesShown; said: string[] } {
  const held =
    raw !== null && typeof raw === "object" && !Array.isArray(raw)
      ? (raw as { notices?: unknown }).notices
      : undefined;
  if (held === undefined || held === "minimised") return { value: "minimised", said: [] };
  if (held === "expanded") return { value: "expanded", said: [] };
  return {
    value: "minimised",
    said: [
      `"notices" ${JSON.stringify(held)} is not "minimised" or "expanded", so a pane's notices are minimised`,
    ],
  };
}

/** The store (`layoutPref.ts`), under `notices`. */
export const PANE_NOTICES = layoutPref<PaneNoticesShown, "notices">({
  key: "notices",
  fallback: "minimised",
  load: loadPaneNotices,
  same: (one, other) => one === other,
  written: (shown) => (shown === "expanded" ? "expanded" : undefined),
  remedy: (where) =>
    `fix ${where}, or minimise or expand a pane's notices from its bell, which rewrites it`,
});

/** Whether every pane's notices are minimised now, for a component that redraws when it changes. */
export const useNoticesMinimised = (): boolean => PANE_NOTICES.use() === "minimised";

/** Minimises or expands every pane's notices, and writes the choice to the layout file. */
export const showNotices = (shown: PaneNoticesShown): void => PANE_NOTICES.set(shown);
