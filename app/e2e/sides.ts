// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser } from "@wdio/globals";

/**
 * **Putting a side away and bringing it back, as a person does** since the status line's
 * toggles went (qw357). The right side: a press of the open view on its bar puts it away, and a
 * press of any view there brings it back on that view; the bar stands while the side is away.
 * The left side panel's tabs are across its own top (#2917, N36, R121): a press of its open tab
 * does nothing, so its key, ⌘B (Ctrl+B off a Mac), puts it away and brings it back, and its tabs
 * go and come with it.
 */

/** A side's region, by the name its activity bar carries. */
export type SideRegion = "Navigation" | "Attention";

const bar = (region: SideRegion) => `[role="tablist"][aria-label="${region}"]`;
const openView = (region: SideRegion) => `${bar(region)} [role="tab"][aria-selected="true"]`;

/** The side's key, ⌘B or ⌥⌘B (Ctrl+B or Ctrl+Alt+B off a Mac), sent to the window as a person's
 *  chord is: the window reads it before anything else does (`sideKeys.ts`). */
async function sideKey(region: SideRegion): Promise<void> {
  await browser.execute((right: boolean) => {
    const mac = /Mac/.test(navigator.platform);
    document.body.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "b",
        code: "KeyB",
        metaKey: mac,
        ctrlKey: !mac,
        altKey: right,
        bubbles: true,
        cancelable: true,
      }),
    );
  }, region === "Attention");
}

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
  if (region === "Navigation") await sideKey(region);
  else await open.click();
  await browser.waitUntil(() => sideIsAway(region), {
    timeout: 20_000,
    timeoutMsg: `the ${region} side was not put away from ${view}`,
  });
  return view;
}

/** Brings the side back on `view`, by pressing that view on its bar. */
export async function bringSideBack(region: SideRegion, view: string): Promise<void> {
  if (region === "Navigation") {
    await sideKey(region);
    await $(openView(region)).waitForExist({ timeout: 20_000 });
    const tab = await $(`${bar(region)} [role="tab"][aria-label="${view}"]`);
    if ((await tab.getAttribute("aria-selected")) !== "true") await tab.click();
    return;
  }
  await $(`${bar(region)} [role="tab"][aria-label="${view}"]`).click();
  await $(openView(region)).waitForExist({ timeout: 20_000 });
}
