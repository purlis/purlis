// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import { endEveryChat, noDialogIsLeftUp, pressAndStart, showView } from "../opening.js";
import { f6From, keyFromThePage, keyboardToThePage, stopsInTheCycle } from "../regionKeys.js";

/**
 * **A keyboard-only journey across the window's regions** (#2929, spec #2897, N35; WCAG 2.2
 * 2.1.1, 2.4.3, 2.4.7 and 2.4.11), in the built app, with a chat open and the Inbox out.
 *
 * - **The tab sequence reads the window in its order**: the title bar, the project rail, the left
 *   side, the editor's tabs and panes, the right side (the Inbox), the Panel, and the right-hand
 *   activity bar at the window's edge, each one run of stops that never comes back once left.
 * - **Every stop F6 lands on shows it has the keyboard**: a ring the engine draws for
 *   `:focus-visible`, on a control that is on screen and not covered by another (2.4.11), both
 *   ways round the cycle, and the keyboard is never dropped on the page between stops.
 * - **A dialog shut with Escape gives the keyboard back** to where it was, not to the page.
 *
 * `keyboard-reach.e2e.ts` holds the stops each strip has and `panel-toggle.e2e.ts` which region
 * F6 lands in; this walks the whole window as one journey. As there, no scenario can press Tab:
 * this driver performs no key's default action (`docs/ui-primitives.md`), so the sequence is read
 * from the page as WebKit builds it, and F6 is the window's own key, which the window hears.
 */

/** The tab sequence's regions, in the window's reading order. */
const ORDER = ["title bar", "rail", "left", "editor", "right", "panel", "right bar", "status"];

/** What a stop shows of the keyboard: lost when it is on the page itself. */
type Shown = {
  lost: boolean;
  what: string;
  onScreen: boolean;
  uncovered: boolean;
  focusVisible: boolean;
  ring: boolean;
};

/** Reads where the keyboard is and whether a person can see it there. */
async function whatTheKeyboardShows(): Promise<Shown> {
  return browser.execute(() => {
    const on = document.activeElement;
    if (!(on instanceof HTMLElement) || on === document.body)
      return {
        lost: true,
        what: on?.tagName ?? "nothing",
        onScreen: false,
        uncovered: false,
        focusVisible: false,
        ring: false,
      };
    const label = on.getAttribute("aria-label") ?? on.textContent ?? "";
    const what = `${on.getAttribute("role") ?? on.tagName.toLowerCase()} ${JSON.stringify(label.trim().slice(0, 40))}`;
    const box = on.getBoundingClientRect();
    const left = Math.max(box.left, 0);
    const top = Math.max(box.top, 0);
    const right = Math.min(box.right, window.innerWidth);
    const bottom = Math.min(box.bottom, window.innerHeight);
    const onScreen = right - left >= 1 && bottom - top >= 1;
    // Not entirely hidden (2.4.11): some point of the part on screen is the control's own.
    const points = [
      [(left + right) / 2, (top + bottom) / 2],
      [left + 2, top + 2],
      [right - 2, top + 2],
      [left + 2, bottom - 2],
      [right - 2, bottom - 2],
    ];
    const uncovered =
      onScreen &&
      points.some(([x, y]) => {
        const hit = document.elementFromPoint(x, y);
        return hit !== null && (hit === on || on.contains(hit));
      });
    const css = getComputedStyle(on);
    const ring =
      (css.outlineStyle !== "none" && Number.parseFloat(css.outlineWidth) > 0) ||
      css.boxShadow !== "none";
    return {
      lost: false,
      what,
      onScreen,
      uncovered,
      focusVisible: on.matches(":focus-visible"),
      ring,
    };
  });
}

/** What a stop should show: on screen, not covered, and ringed as the keyboard's. */
function seen(what: string): Shown {
  return { lost: false, what, onScreen: true, uncovered: true, focusVisible: true, ring: true };
}

describe("a keyboard-only journey across the window's regions", function () {
  this.timeout(180_000);

  before(async () => {
    await noDialogIsLeftUp();
    await pressAndStart("New tab");
    await $('[data-testid="pane"] textarea').waitForExist({ timeout: 20_000 });
    // The Inbox out on the right, as the journey ends in it.
    await showView("Asks");
  });

  after(async () => {
    await noDialogIsLeftUp();
    await keyboardToThePage();
    await endEveryChat();
  });

  it("reads the tab sequence in the window's order: title bar, rail, sides, editor, Inbox, Panel", async () => {
    const regions = await browser.execute(() => {
      const regionOf = (el: Element): string => {
        if (el.closest(".title-bar")) return "title bar";
        if (el.closest(".window-below") && !el.closest(".window-work")) return "rail";
        const column = el.closest(".activity-column");
        if (column) return column.getAttribute("data-side") === "right" ? "right bar" : "left";
        if (el.closest(".slot-left")) return "left";
        if (el.closest(".region-centre")) return "editor";
        if (el.closest(".slot-right")) return "right";
        if (el.closest("#region-panel")) return "panel";
        if (el.closest('footer[data-scope="status-bar"]')) return "status";
        return `elsewhere (${el.tagName.toLowerCase()} ${(el.getAttribute("aria-label") ?? "").slice(0, 30)})`;
      };
      const stops = [
        ...document.querySelectorAll<HTMLElement>(
          'a[href], button, input, select, textarea, summary, [tabindex], [contenteditable="true"]',
        ),
      ].filter((el) => {
        if (el.tabIndex < 0) return false;
        if ((el as HTMLButtonElement).disabled) return false;
        if (el.closest('[inert], [hidden], [role="dialog"], [role="alertdialog"]')) return false;
        if (el.getClientRects().length === 0) return false;
        return getComputedStyle(el).visibility === "visible";
      });
      // WebKit's sequence: positive tabindex first, by value, then document order.
      const ordered = [
        ...stops.filter((el) => el.tabIndex > 0).sort((a, b) => a.tabIndex - b.tabIndex),
        ...stops.filter((el) => el.tabIndex === 0),
      ];
      const runs: string[] = [];
      for (const el of ordered) {
        const region = regionOf(el);
        if (runs[runs.length - 1] !== region) runs.push(region);
      }
      return runs;
    });

    // Each region one run, in reading order: a region that comes back is a stop out of place.
    expect(regions).toEqual(ORDER.filter((region) => regions.includes(region)));
    for (const needed of ["title bar", "rail", "left", "editor", "right"])
      expect(regions).toContain(needed);
  });

  it("shows the keyboard at every stop F6 makes, round the cycle and back", async () => {
    await keyboardToThePage();
    const stops = Math.max(await stopsInTheCycle(), 2);
    const walked: { region: string; shown: Shown }[] = [];
    let at = "";
    for (let press = 0; press < stops; press++) {
      at = await f6From(at);
      walked.push({ region: at, shown: await whatTheKeyboardShows() });
    }
    for (let press = 0; press < stops; press++) {
      at = await f6From(at, true);
      walked.push({ region: at, shown: await whatTheKeyboardShows() });
    }
    expect(walked.map((one) => one.region)).toContain("right");
    expect(walked).toEqual(walked.map((one) => ({ ...one, shown: seen(one.shown.what) })));
  });

  it("gives the keyboard back where it was when a dialog is shut with Escape", async () => {
    await keyboardToThePage();
    // On the Inbox, by F6: the last region the journey reached.
    let at = "";
    for (let press = 0; press < 5 && at !== "right"; press++) at = await f6From(at);
    expect(at).toBe("right");
    const before = (await whatTheKeyboardShows()).what;

    // Dispatched from the element that has the keyboard, so it is that element the palette
    // opens over and hands back to.
    await keyFromThePage({ key: "F2", code: "F2" });
    const palette = await $('[role="dialog"][aria-label="Command palette"]');
    await palette.waitForDisplayed({ timeout: 20_000 });
    await keyFromThePage({ key: "Escape", code: "Escape" });
    await palette.waitForDisplayed({ reverse: true, timeout: 10_000 });

    let back: Shown | undefined;
    await browser
      .waitUntil(
        async () => {
          back = await whatTheKeyboardShows();
          return !back.lost;
        },
        { timeout: 5_000, interval: 200 },
      )
      .catch(() => undefined);
    expect(back?.lost).toBe(false);
    expect(back?.what).toBe(before);
  });
});
