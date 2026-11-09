// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import { endEveryChat, noDialogIsLeftUp, pressAndStart, showView } from "../opening.js";
import { REGION_OF, f6From, keyFromThePage, keyboardToThePage } from "../regionKeys.js";

/**
 * **A keyboard-only journey across the window's regions** (#2929, spec #2897, N35; WCAG 2.2
 * 2.1.1, 2.4.3, 2.4.7 and 2.4.11), in the built app, with a chat open and the Inbox out.
 *
 * - **Every stop F6 lands on shows it has the keyboard**: a ring the engine draws for
 *   `:focus-visible`, on a control that is on screen and not covered by another (2.4.11), both
 *   ways round the cycle, and the keyboard is never dropped on the page between stops.
 * - **A dialog shut with Escape gives the keyboard back** to where it was, not to the page.
 *
 * `keyboard-reach.e2e.ts` holds the Tab sequence against the regions as they are laid out, and
 * `panel-toggle.e2e.ts` which region F6 lands in; this walks the whole cycle as one journey and
 * asks what a person sees at each stop. No scenario can press Tab: this driver performs no key's
 * default action (`docs/ui-primitives.md`). F6 is the window's own key, which its capture-phase
 * listener hears from the element that has the keyboard, so it is dispatched there.
 */

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
    // A ring is what the keyboard ADDS: the control's outline or shadow while it has the
    // keyboard, against the same control without it. A shadow it wears at rest is no ring.
    const drawn = () => {
      const css = getComputedStyle(on);
      const outline =
        css.outlineStyle !== "none" && Number.parseFloat(css.outlineWidth) > 0
          ? `${css.outlineStyle} ${css.outlineWidth} ${css.outlineColor}`
          : "none";
      return { outline, shadow: css.boxShadow };
    };
    const focusVisible = on.matches(":focus-visible");
    const held = drawn();
    on.blur();
    const rest = drawn();
    on.focus({ preventScroll: true });
    const ring =
      (held.outline !== "none" && held.outline !== rest.outline) ||
      (held.shadow !== "none" && held.shadow !== rest.shadow);
    return { lost: false, what, onScreen, uncovered, focusVisible, ring };
  });
}

/** What a stop should show: on screen, not covered, and ringed as the keyboard's. */
function seen(what: string): Shown {
  return { lost: false, what, onScreen: true, uncovered: true, focusVisible: true, ring: true };
}

/** The most F6 presses one way round: every region, a chip's card, and one more to be back. */
const MOST = 8;

/** Where one press of F6 left the keyboard, and what a person sees of it there. */
type Stopped = { stop: string; shown: Shown };

/**
 * **One press of F6, or Shift+F6 with `back`**, and where the keyboard went: the region F6 names
 * (`regionKeys.ts`), `card` for a title-bar chip's card (#3648, a stop of the cycle while one is
 * up), `stayed` when it did not move within five seconds, or what it landed on outside all of
 * them. Every outcome is a stop the test can name, so a walk that goes wrong says where.
 */
async function pressF6(back: boolean): Promise<Stopped> {
  await browser.execute(() => {
    (window as unknown as { __e2eWas: Element | null }).__e2eWas = document.activeElement;
  });
  await keyFromThePage({ key: "F6", code: "F6", shiftKey: back });
  const moved = await browser
    .waitUntil(
      () =>
        browser.execute(
          () =>
            document.activeElement !== (window as unknown as { __e2eWas: Element | null }).__e2eWas,
        ),
      { timeout: 5_000, interval: 100 },
    )
    .then(() => true)
    .catch(() => false);
  const where = await browser.execute((regions: Record<string, string>) => {
    const on = document.activeElement;
    if (on === null || on === document.body) return "the page";
    if (on.closest("[data-f6-card]")) return "card";
    const region = Object.entries(regions).find(([, selector]) =>
      [...document.querySelectorAll(selector)].some((one) => one.contains(on)),
    );
    if (region) return region[0];
    return `outside (${on.tagName.toLowerCase()} ${(on.getAttribute("aria-label") ?? "").slice(0, 30)})`;
  }, REGION_OF);
  return { stop: moved ? where : `stayed in ${where}`, shown: await whatTheKeyboardShows() };
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

  it("shows the keyboard at every stop F6 makes, round the cycle and back", async () => {
    await keyboardToThePage();
    const forth: Stopped[] = [];
    for (let press = 0; press < MOST; press++) {
      forth.push(await pressF6(false));
      if (forth.length > 1 && forth[forth.length - 1].stop === forth[0].stop) break;
    }
    const back: Stopped[] = [];
    for (let press = 0; press < MOST; press++) {
      back.push(await pressF6(true));
      if (back.length > 1 && back[back.length - 1].stop === back[0].stop) break;
    }
    const stops = (walk: Stopped[]) => walk.map((one) => one.stop);
    // Round once each way: back to where it began, through every region, the Inbox among them.
    expect(stops(forth)[stops(forth).length - 1]).toBe(stops(forth)[0]);
    expect(stops(forth)).toEqual(expect.arrayContaining(["left", "centre", "right"]));
    // Shift+F6 walks the same cycle the other way round.
    const once = (walk: Stopped[]) => stops(walk).slice(0, -1);
    expect([...once(back)].sort()).toEqual([...once(forth)].sort());
    // And at every stop a person can see where the keyboard is.
    const all = [...forth, ...back];
    expect(all).toEqual(all.map((one) => ({ ...one, shown: seen(one.shown.what) })));
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
