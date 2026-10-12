// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect } from "@wdio/globals";

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
const FRAME = ".region-frame[data-cycle]";
const MAC = process.platform === "darwin";

/** Whether the Panel takes room: a Panel put away is collapsed to no height. */
async function panelShown(): Promise<boolean> {
  return browser.execute(
    (panel: string) => (document.querySelector(panel)?.getBoundingClientRect().height ?? 0) > 1,
    PANEL,
  );
}

/** Takes the keyboard off whatever has it, so no chat keeps the next key. */
async function keyboardToThePage(): Promise<void> {
  await browser.execute(() => {
    const on = document.activeElement;
    if (on instanceof HTMLElement) on.blur();
  });
}

/** Dispatches `init` as a `keydown` from the page to the element that has the keyboard. */
async function keyFromThePage(init: KeyboardEventInit): Promise<void> {
  await browser.execute((given: KeyboardEventInit) => {
    (document.activeElement ?? document.body).dispatchEvent(
      new KeyboardEvent("keydown", { ...given, bubbles: true, cancelable: true }),
    );
  }, init);
}

/**
 * Presses a key as the window hears it. A chord is dispatched from the page: the driver does not
 * hold a modifier across `browser.keys` (`palette.e2e.ts`), so its Shift+F6 would arrive as a
 * bare F6 and step the wrong way. A bare key goes through the driver first and, where `landed`
 * has not come true within a few seconds, from the page.
 */
async function keyWithFallback(
  keys: string[] | "chord",
  init: KeyboardEventInit,
  landed: () => Promise<boolean>,
): Promise<void> {
  if (keys === "chord") return keyFromThePage(init);
  await browser.keys(keys);
  const came = await browser
    .waitUntil(landed, { timeout: 3_000, interval: 150 })
    .then(() => true)
    .catch(() => false);
  if (!came) await keyFromThePage(init);
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

/**
 * Where each region F6 stops in is drawn, by the stop's name in the frame's cycle: the frame's
 * own `REGION_OF` (`RegionFrame.tsx`), read here and never imported, since a spec runs outside
 * the app's bundle.
 */
const REGION_OF: Record<string, string> = {
  left: ".slot-left .region-view:not([hidden])",
  centre: ".region-centre",
  right: ".slot-right .region-view:not([hidden])",
  "panel-left": '.panel-half[data-side="left"]:not([hidden])',
  "panel-right": '.panel-half[data-side="right"]:not([hidden])',
};

/** The stop the keyboard is in, by its name in the cycle; `""` where it is in none. */
async function regionWithTheKeyboard(): Promise<string> {
  return browser.execute(
    (frame: string, regions: Record<string, string>) => {
      const root = document.querySelector(frame);
      const on = document.activeElement;
      if (root === null || on === null) return "";
      return (
        Object.entries(regions).find(([, selector]) =>
          [...root.querySelectorAll(selector)].some((region) => region.contains(on)),
        )?.[0] ?? ""
      );
    },
    FRAME,
    REGION_OF,
  );
}

/** F6, or Shift+F6 with `back`, until the keyboard is in a region other than `from`. */
async function f6From(from: string, back = false): Promise<string> {
  await keyWithFallback(
    back ? "chord" : ["F6"],
    { key: "F6", code: "F6", shiftKey: back },
    async () => {
      const now = await regionWithTheKeyboard();
      return now !== "" && now !== from;
    },
  );
  let now = "";
  await browser.waitUntil(
    async () => {
      now = await regionWithTheKeyboard();
      return now !== "" && now !== from;
    },
    {
      timeout: 10_000,
      timeoutMsg: `${back ? "Shift+F6" : "F6"} did not move the keyboard out of ${from || "the page"}`,
    },
  );
  return now;
}

/** How many stops the frame's cycle has: the most F6 presses that walk it once. */
async function stopsInTheCycle(): Promise<number> {
  return browser.execute(
    (frame: string) =>
      (document.querySelector(frame)?.getAttribute("data-cycle") ?? "").split(" ").filter(Boolean)
        .length,
    FRAME,
  );
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
