// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect } from "@wdio/globals";
import {
  FRAME,
  f6From,
  keyWithFallback,
  keyboardToThePage,
  stopsInTheCycle,
} from "../regionKeys.js";

/**
 * **⌘J shows and hides the Panel, and F6 moves the keyboard between the regions drawn** (#1737
 * P-6, #1741 P-13; the trains 73-74 ⌘J fix), in the built app, where the window lays out.
 *
 * - ⌘J (Ctrl+J off a Mac) puts the Panel away, which leaves it no height, and brings it back.
 * - F6 takes the keyboard to the next region the frame's cycle names, Shift+F6 back to the one
 *   before, and a Panel put away is a region F6 never lands in.
 *
 * `sideKeys.test.ts`, `regionCycle.test.ts` and `FourRegions.test.tsx` hold which key is which
 * and where each lands against jsdom; only a window that lays out can say the Panel is gone.
 *
 * **Keys as the window hears them** (train 78's e2e lessons): a bare key through the driver
 * first, and where nothing came of it within a few seconds, the same key dispatched from the page
 * to the element that has the keyboard, which the window's capture-phase listener hears; a chord
 * from the page alone, since the driver does not hold its modifier. The keyboard is taken off any
 * chat first: off a Mac, Ctrl+J is a terminal's newline and is left to a chat.
 *
 * **It leaves the window as it found it**: the Panel is shown or hidden as it was, whatever an
 * assertion did, and the keyboard is given back to the page.
 */

const PANEL = '[data-panel][id="region-panel"]';
const MAC = process.platform === "darwin";

/** Whether the Panel takes room: a Panel put away is collapsed to no height. */
async function panelShown(): Promise<boolean> {
  return browser.execute(
    (panel: string) => (document.querySelector(panel)?.getBoundingClientRect().height ?? 0) > 1,
    PANEL,
  );
}

/** ⌘J, or Ctrl+J off a Mac, until the Panel is `want`. */
async function togglePanelTo(want: boolean): Promise<void> {
  await keyboardToThePage();
  await keyWithFallback(
    "chord",
    { key: "j", code: "KeyJ", metaKey: MAC, ctrlKey: !MAC },
    async () => (await panelShown()) === want,
  );
  await browser.waitUntil(async () => (await panelShown()) === want, {
    timeout: 10_000,
    timeoutMsg: `${MAC ? "⌘J" : "Ctrl+J"} did not ${want ? "bring the Panel back" : "put the Panel away"}`,
  });
}

describe("the Panel's key and the region keys", function () {
  this.timeout(180_000);

  /** Whether the Panel was shown when the spec began, which is how it is left. */
  let wasShown = true;

  before(async () => {
    await browser.$(FRAME).waitForExist({ timeout: 30_000 });
    wasShown = await panelShown();
  });

  after(async () => {
    if ((await panelShown()) !== wasShown) await togglePanelTo(wasShown);
    await keyboardToThePage();
  });

  it("puts the Panel away and brings it back", async () => {
    await togglePanelTo(!wasShown);
    await togglePanelTo(wasShown);
    expect(await panelShown()).toBe(wasShown);
  });

  it("moves the keyboard to the next region with F6 and back with Shift+F6", async () => {
    await keyboardToThePage();
    const first = await f6From("");
    const second = await f6From(first);
    expect(second).not.toBe(first);
    const back = await f6From(second, true);
    expect(back).toBe(first);
  });

  it("never lands in a Panel that is put away", async () => {
    if (wasShown) await togglePanelTo(false);
    await keyboardToThePage();
    const visited: string[] = [];
    let at = "";
    // Twice round the cycle, so every region it names has had its turn.
    const presses = 2 * Math.max(await stopsInTheCycle(), 2);
    for (let press = 0; press < presses; press++) {
      at = await f6From(at);
      visited.push(at);
    }
    expect(visited.filter((one) => one.startsWith("panel-"))).toEqual([]);
    expect(new Set(visited).size).toBeGreaterThan(1);
    // And the cycle the frame names has no half of the Panel either.
    expect(
      await browser.execute(
        (frame: string) => document.querySelector(frame)?.getAttribute("data-cycle") ?? "",
        FRAME,
      ),
    ).not.toContain("panel-");
  });
});
