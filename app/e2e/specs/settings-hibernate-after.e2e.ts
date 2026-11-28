// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import { closesTheTabsItOpens } from "../reach.js";
import { openProjectSettings, settingsAt } from "../settingsTab.js";

/**
 * **Settings › Project › Chats › Hibernate after, in the real app** (SC-4h, #7637): the row reads
 * `[chats] hibernate_after` from the real core, a value under 10 minutes is refused in the core's
 * own sentence, and a time the core takes is what the row then shows, as the core answered it.
 *
 * What jsdom cannot say: that the window's command reaches the core's own check and its write.
 * `HibernateAfter.test.tsx` holds the row's own behaviour, `hibernateafter/tests.rs` what the
 * file says and keeps, and the gate's tests in `planes/tests.rs` that a change arms its deadline
 * anew.
 *
 * **It leaves the window as it found it**: one app process serves the whole run, so the setting
 * is put back to the 30 minutes it started at, and the Settings tab it opened is closed.
 */

/** The row's name, which names its box by `aria-labelledby`. */
const NAME = "Hibernate after";

const UNDER = "5m is under 10 minutes, the least it may be. Set 10m or more, or off.";

/** The row's box, once the core has said what is in force. */
async function theBox() {
  // By its accessible name, never its tag: the control the row's name labels.
  await browser.waitUntil(
    async () =>
      browser.execute((wanted: string) => {
        const name = [...document.querySelectorAll<HTMLElement>(".ui-setting-label")].find(
          (one) => one.textContent?.trim() === wanted && one.id !== "",
        );
        const box = name && document.querySelector(`[aria-labelledby~="${name.id}"]`);
        box?.setAttribute("data-e2e", "hibernate-after");
        return box != null;
      }, NAME),
    { timeout: 20_000, timeoutMsg: "Settings › Project › Chats drew no Hibernate after" },
  );
  const box = await $('[data-e2e="hibernate-after"]');
  await box.waitForDisplayed({ timeout: 20_000 });
  await browser.waitUntil(async () => box.isEnabled(), {
    timeout: 20_000,
    timeoutMsg: "the core never said what Hibernate after is",
  });
  return box;
}

/** Types `value` into the box in place of what it holds, and presses Enter. */
async function typeIn(value: string): Promise<void> {
  const box = await theBox();
  await box.click();
  await box.clearValue();
  await box.addValue(value);
  await browser.keys(["Enter"]);
}

/** Opens Settings at Project › Chats. */
async function chatsPage(): Promise<void> {
  await openProjectSettings();
  await settingsAt("Project", "Chats");
}

describe("Settings › Project › Chats › Hibernate after", () => {
  closesTheTabsItOpens();

  it("reads the core's 30 minutes, refuses under 10 minutes, and keeps a time it takes", async () => {
    await chatsPage();
    await expect(await theBox()).toHaveValue("30m");

    await typeIn("5m");
    await browser.waitUntil(
      async () =>
        browser.execute(
          (said: string) =>
            [...document.querySelectorAll(".ui-setting-row")].some((row) =>
              row.textContent?.includes(said),
            ),
          UNDER,
        ),
      { timeout: 20_000, timeoutMsg: "the core's sentence for 5m was never said on the row" },
    );

    await typeIn("45m");
    await browser.waitUntil(
      async () =>
        browser.execute(
          (said: string) =>
            ![...document.querySelectorAll(".ui-setting-row")].some((row) =>
              row.textContent?.includes(said),
            ),
          UNDER,
        ),
      { timeout: 20_000, timeoutMsg: "the refusal stayed once 45m was taken" },
    );

    await expect(await theBox()).toHaveValue("45m");

    await typeIn("30m");
    await browser.waitUntil(async () => (await (await theBox()).getValue()) === "30m", {
      timeout: 20_000,
      timeoutMsg: "the setting was not put back to 30m",
    });
  });
});
