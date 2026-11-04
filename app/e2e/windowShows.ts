// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";

/**
 * **What the window showed when a test failed** (#4758's merge-queue run 41813865770): the tab
 * strip in front, what its overflow hid, and the panes drawn. A failure that waited for a pane
 * or counted tabs said only that it never came, and on the macOS runner, where the scenario
 * parts run only in the merge queue, the record is all there is to read. So every failure's
 * record says which tabs were drawn, which was selected, how many the strip hid, and which panes
 * and views were drawn, for its annotation (`failures.ts`).
 */
export async function whatTheWindowShows(): Promise<string[]> {
  return browser
    .execute(() => {
      const words = (el: Element | null | undefined) =>
        (el?.textContent ?? "").trim().replace(/\s+/g, " ").slice(0, 60);
      const strip = document.querySelector('[data-strip="Tabs"]');
      const tabs = [...(strip?.querySelectorAll('[role="tab"]') ?? [])].map(
        (tab) =>
          `${words(tab)}${tab.getAttribute("aria-selected") === "true" ? " (selected)" : ""}`,
      );
      const more = [...document.querySelectorAll("button[aria-haspopup='menu']")].find((one) =>
        /^Show \d+ tabs? the strip is not showing/.test(one.getAttribute("aria-label") ?? ""),
      );
      const panes = [...document.querySelectorAll(".panes [data-testid]")]
        .map((one) => one.getAttribute("data-testid") ?? "")
        .filter((id) => /^(view-pane-|empty-pane|pane-)/.test(id));
      const views = [...document.querySelectorAll('[data-testid^="view-pane-"]')].map(
        (one) => one.getAttribute("data-testid") ?? "",
      );
      const workspaces = [...document.querySelectorAll('[role="tablist"] [role="tab"]')]
        .filter((tab) => strip === null || !strip.contains(tab))
        .filter((tab) => tab.getAttribute("aria-selected") === "true")
        .map((tab) => words(tab));
      return [
        `the strip drew ${tabs.length} tabs: ${tabs.join(" | ") || "none"}`,
        `its overflow: ${more?.getAttribute("aria-label") ?? "none"}`,
        `the panes drew: ${[...new Set(panes)].join(", ") || "none"}`,
        `views drawn anywhere: ${[...new Set(views)].join(", ") || "none"}`,
        `tabs selected on the other tab lists: ${workspaces.join(" | ") || "none"}`,
        `the window is ${window.innerWidth}×${window.innerHeight}`,
      ];
    })
    .catch((trouble: unknown) => [`the window did not answer: ${String(trouble)}`]);
}
