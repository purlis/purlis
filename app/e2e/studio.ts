// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser } from "@wdio/globals";

/**
 * **Getting about the Skill studio's tab** (#3681): its pages in the left navigation, and a skill
 * of its Library chosen, which opens beside the list. Each press is the element's own `click()`
 * from the page, as `reach.ts` presses everything.
 */

const NAV = 'nav[aria-label="Skill studio pages"]';

/** **Goes to the studio's page called `name`** (Library, Drafts, Deleted…), from its navigation,
 *  and waits until the navigation says it is there. */
export async function studioPage(name: string) {
  await $(NAV).waitForExist({ timeout: 20_000 });
  await browser.waitUntil(
    async () =>
      browser.execute(
        (nav: string, said: string) => {
          const page = [...document.querySelectorAll<HTMLElement>(`${nav} button`)].find(
            (one) => one.querySelector('[data-part="label"]')?.textContent === said,
          );
          if (page === undefined) return false;
          if (page.getAttribute("aria-current") === "page") return true;
          page.click();
          return false;
        },
        NAV,
        name,
      ),
    { timeout: 20_000, interval: 250, timeoutMsg: `the studio never went to ${name}` },
  );
}

/** **Chooses the Library's row of `name`** and answers the skill's item beside the list once it
 *  shows that skill. */
export async function studioSkill(name: string) {
  const row = `[data-testid="skill-${name}"]`;
  await $(row).waitForExist({ timeout: 20_000 });
  const id = await $(row).getAttribute("data-row");
  await browser.execute((css: string) => document.querySelector<HTMLElement>(css)?.click(), row);
  const item = $(`[data-testid="skill-studio-skill"][data-row="${id}"]`);
  await item.waitForExist({ timeout: 20_000, timeoutMsg: `${name} never opened beside the list` });
  return item;
}
