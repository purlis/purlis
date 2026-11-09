// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import { noDialogIsLeftUp } from "../opening.js";
import { closesTheTabsItOpens } from "../reach.js";
import { openProjectSettings } from "../settingsTab.js";

/**
 * **purlis's own Reduce motion, applied to the whole window** (#2929, #2925, R152; WCAG 2.2
 * 2.3.3), in the built app: Settings › You › Motion and contrast's switch, pressed as a person
 * presses it.
 *
 * `motion.e2e.ts` holds the tokens and one tab's transition whatever the runner's own setting is;
 * this turns purlis's setting on and reads every element the window draws: none transitions, and
 * none animates for longer than no time. Turned off again, the window moves as the theme says,
 * unless the runner's system asks for less motion itself, which purlis never overrides.
 */

const SWITCH = '[data-setting="you.motion.reduce"] [role="switch"]';

/** Every element in the window that still moves: its transition, or its animation's length. */
async function stillMoving(): Promise<string[]> {
  return browser.execute(() =>
    [...document.querySelectorAll<HTMLElement>("body *")]
      .map((el) => {
        const css = getComputedStyle(el);
        const durations = (text: string) => text.split(",").map((one) => Number.parseFloat(one));
        const transitions =
          css.transitionProperty !== "none" &&
          durations(css.transitionDuration).some((one) => one > 0);
        const animates =
          css.animationName !== "none" && durations(css.animationDuration).some((one) => one > 0);
        if (!transitions && !animates) return null;
        const what = `${el.tagName.toLowerCase()}.${[...el.classList].slice(0, 2).join(".")}`;
        return transitions
          ? `${what} transitions ${css.transitionProperty} over ${css.transitionDuration}`
          : `${what} animates ${css.animationName} over ${css.animationDuration}`;
      })
      .filter((one): one is string => one !== null)
      .slice(0, 20),
  );
}

/** Whether the switch says on. */
async function switchOn(): Promise<boolean> {
  return browser.execute((selector: string) => {
    const control = document.querySelector<HTMLElement>(selector);
    return (
      control?.getAttribute("aria-checked") === "true" ||
      (control instanceof HTMLInputElement && control.checked)
    );
  }, SWITCH);
}

/** Presses the switch until it says `want`. */
async function turn(want: boolean): Promise<void> {
  if ((await switchOn()) === want) return;
  await $(SWITCH).click();
  await browser.waitUntil(async () => (await switchOn()) === want, {
    timeout: 10_000,
    timeoutMsg: `Reduce motion never turned ${want ? "on" : "off"}`,
  });
}

describe("purlis's own Reduce motion, across the window", function () {
  this.timeout(120_000);

  let wasOn = false;

  after(async () => {
    if (await $(SWITCH).isExisting()) await turn(wasOn);
  });

  // Between the two: its `before` reads the strip before Settings opens, and its `after` closes
  // Settings once the switch above is put back (mocha runs hooks of a kind in their order).
  closesTheTabsItOpens();

  before(async () => {
    await noDialogIsLeftUp();
    await openProjectSettings();
    await browser.execute(() => {
      const radio = [
        ...document.querySelectorAll<HTMLInputElement>(
          '[role="radiogroup"][aria-label="Level"] input[type="radio"]',
        ),
      ].find((one) => one.closest("label")?.textContent?.trim() === "You");
      if (radio?.checked !== true) radio?.click();
    });
    await browser.waitUntil(
      async () =>
        browser.execute(() => {
          const button = [
            ...document.querySelectorAll<HTMLElement>('nav[aria-label="Groups"] button'),
          ].find((one) => one.textContent?.trim() === "Motion and contrast");
          button?.click();
          return button?.getAttribute("aria-current") === "true";
        }),
      { timeout: 20_000, interval: 250, timeoutMsg: "Settings never showed You › Motion" },
    );
    await $(SWITCH).waitForDisplayed({ timeout: 20_000 });
    wasOn = await switchOn();
  });

  it("stops every transition and animation in the window when it is turned on", async () => {
    await turn(true);
    await browser.waitUntil(
      async () =>
        (await browser.execute(() => document.documentElement.getAttribute("data-motion"))) ===
        "reduced",
      { timeout: 10_000, timeoutMsg: "the root never said motion was reduced" },
    );
    expect(await stillMoving()).toEqual([]);
  });

  it("gives the window its motion back when it is turned off, unless the system asks for less", async () => {
    await turn(false);
    const systemReduces = await browser.execute(
      () => matchMedia("(prefers-reduced-motion: reduce)").matches,
    );
    await browser.waitUntil(
      async () =>
        (await browser.execute(() => document.documentElement.getAttribute("data-motion"))) ===
        (systemReduces ? "reduced" : null),
      { timeout: 10_000, timeoutMsg: "the root's motion never followed the switch" },
    );
    if (systemReduces) return;
    // A project's tab on the rail moves again, as `motion.e2e.ts` reads it.
    const transition = await browser.execute(() => {
      const tab = document.querySelector('[data-strip="Projects"] [role="tab"]');
      return tab === null ? null : getComputedStyle(tab).transitionProperty;
    });
    expect(transition).not.toBeNull();
    expect(transition).not.toBe("none");
  });
});
