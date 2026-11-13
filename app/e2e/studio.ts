// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser } from "@wdio/globals";
import { pressMenuPath } from "./menus.js";

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

/** **Waits until the studio's navigation says it is on the page called `name`**, pressing
 *  nothing: for a press elsewhere that goes there (Browse sources). */
export async function untilStudioPage(name: string) {
  await browser.waitUntil(
    async () =>
      browser.execute(
        (nav: string, said: string) =>
          [...document.querySelectorAll<HTMLElement>(`${nav} button`)].some(
            (one) =>
              one.querySelector('[data-part="label"]')?.textContent === said &&
              one.getAttribute("aria-current") === "page",
          ),
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

/**
 * **Chooses `said` from the "…" menu of skill `name`'s page** (#3682): Rename…, Move to another
 * level…, Delete…, each the start of what it changes.
 */
export async function studioMore(name: string, said: string) {
  const more = `button[aria-label="More for ${name}"]`;
  const menu = `[role="menu"][aria-label="More for ${name}"]`;
  await opened(more, menu, `${name}'s menu never opened`);
  await browser.waitUntil(
    async () =>
      browser.execute(
        (css: string, words: string) => {
          const row = [...document.querySelectorAll<HTMLElement>(`${css} [role="menuitem"]`)].find(
            (one) => (one.textContent ?? "").startsWith(words),
          );
          row?.click();
          return row !== undefined;
        },
        menu,
        said,
      ),
    { timeout: 20_000, interval: 250, timeoutMsg: `${name}'s menu has no ${said}` },
  );
}

const WHOSE = '[data-testid="studio-whose"]';
const WHOSE_MENU = '[role="menu"][aria-label="Whose chats and where they start"]';

/**
 * **Picks the row named `said` from the studio's Whose chats picker** (#2935): a chat in a
 * workspace, "A chat as steward", "Dispatched into alpha"… It opens the menu, presses the row by
 * its name, and waits until the picker's button `reads` the scope picked.
 */
export async function studioWhose(said: string, reads: string) {
  await opened(WHOSE, WHOSE_MENU, "the Whose chats menu never opened");
  await pressMenuPath(said);
  await browser.waitUntil(async () => (await $(WHOSE).getText()).trim() === reads, {
    timeout: 20_000,
    interval: 250,
    timeoutMsg: `the Whose chats picker never read ${JSON.stringify(reads)}`,
  });
}

/** **Presses `button` until `menu` is up**: a press that lands while the page redraws opens
 *  none, and a button that is not usable yet is not pressed. */
async function opened(button: string, menu: string, timeoutMsg: string) {
  await $(button).waitForExist({ timeout: 20_000 });
  await browser.waitUntil(
    async () =>
      browser.execute(
        (pressed: string, up: string) => {
          if (document.querySelector(up) !== null) return true;
          const one = document.querySelector<HTMLElement>(pressed);
          if (one !== null && one.getAttribute("aria-disabled") !== "true") one.click();
          return false;
        },
        button,
        menu,
      ),
    { timeout: 20_000, interval: 500, timeoutMsg },
  );
}

/** **Writes a skill folder `name` under `dir`**, as a person would: its `SKILL.md` alone, saying
 *  it probes `name`. Answers the folder, for the spec to remove. */
export function writeSkill(dir: string, name: string): string {
  const folder = join(dir, name);
  mkdirSync(folder, { recursive: true });
  writeFileSync(
    join(folder, "SKILL.md"),
    `---\nname: ${name}\ndescription: Probes ${name}.\n---\n\nSay hello.\n`,
  );
  return folder;
}
/** **Shows the tab `name` of the skill page beside the list** (#3682): Overview, Text, History,
 *  Grades or Runs, pressed again until it is the one shown. */
export async function studioTab(name: string) {
  await browser.waitUntil(
    async () =>
      browser.execute((said: string) => {
        const tab = [
          ...document.querySelectorAll<HTMLElement>(
            '[data-testid="skill-studio-skill"] [role="tab"]',
          ),
        ].find((one) => (one.textContent ?? "").trim().startsWith(said));
        if (tab === undefined) return false;
        if (tab.getAttribute("aria-selected") === "true") return true;
        tab.click();
        return false;
      }, name),
    { timeout: 20_000, interval: 250, timeoutMsg: `the skill's ${name} tab never showed` },
  );
}
