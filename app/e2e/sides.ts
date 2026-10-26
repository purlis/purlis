// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser } from "@wdio/globals";

/**
 * **Putting a side away and bringing it back, as a person does** since the status line's
 * toggles went (qw357). On either side a press of the open view on its bar puts it away, and a
 * press of any view there brings it back on that view (R121 as amended 2026-10-26, #3329); the
 * bar stands while the side is away, so nothing moves.
 */

/** A side's region, by the name its activity bar carries. */
export type SideRegion = "Navigation" | "Attention";

const bar = (region: SideRegion) => `[role="tablist"][aria-label="${region}"]`;
const openView = (region: SideRegion) => `${bar(region)} [role="tab"][aria-selected="true"]`;

/** Whether the side is put away: its bar has no open view. */
export async function sideIsAway(region: SideRegion): Promise<boolean> {
  return !(await $(openView(region)).isExisting());
}

/** Puts the side away by pressing its open view, and says which view that was, so
 *  {@link bringSideBack} can put the window back as it was found. */
export async function putSideAway(region: SideRegion): Promise<string> {
  const open = await $(openView(region));
  await open.waitForExist({ timeout: 20_000 });
  const view = (await open.getAttribute("aria-label")) ?? "";
  await open.click();
  await browser.waitUntil(() => sideIsAway(region), {
    timeout: 20_000,
    timeoutMsg: `the ${region} side was not put away from ${view}`,
  });
  return view;
}

/** Brings the side back on `view`, by pressing that view on its bar. */
export async function bringSideBack(region: SideRegion, view: string): Promise<void> {
  await $(`${bar(region)} [role="tab"][aria-label="${view}"]`).click();
  await $(openView(region)).waitForExist({ timeout: 20_000 });
}
