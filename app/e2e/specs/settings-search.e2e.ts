// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import { closesTheTabsItOpens, runFromThePalette } from "../reach.js";
import { SETTINGS_GROUPS, openProjectSettings, settingsAt } from "../settingsTab.js";

/**
 * **One settings search across every level, in the real WebView** (#2920, N27; #2814·4): a
 * search walked as the operator makes it.
 *
 * - From Settings at the Project level, typed words find settings at You and at the Project
 *   level on one page, the nav's first, each under its level and page.
 * - A match at another level is a way there: pressed, it lands on its field at You, with the
 *   keyboard in it and the search done.
 * - Set here narrows to what the Project level's files hold: the fixture project's
 *   `charter.toml` sets `[memory] share`, and every row left carries the set-here mark.
 * - From the palette, a setting's own row lands on its field.
 *
 * What jsdom cannot say: that the WebView draws it, and that the keyboard lands in a real field.
 * `SettingsSearch.test.tsx` holds the behaviour. **Nothing is written**; it closes the Settings
 * tabs it opened.
 */

const BOX = '[aria-label="Search settings"]';

/** What the page of every match draws: its title, its sections' headings, and its rows, each
 *  by its setting and whether it carries the set-here mark. */
async function found() {
  return browser.execute(() => {
    const page = document.querySelector(".ui-settings-found");
    return {
      title: page?.querySelector("h2")?.textContent ?? "",
      sections: [...(page?.querySelectorAll<HTMLElement>(".ui-settings-found-section") ?? [])]
        .filter((one) => !one.hidden)
        .map((one) => one.querySelector("h3")?.textContent ?? ""),
      rows: [...(page?.querySelectorAll<HTMLElement>("[data-setting]") ?? [])].map((row) => ({
        setting: row.dataset.setting ?? "",
        setHere: row.querySelector('[role="img"][aria-label="Set at this level"]') !== null,
      })),
      first: (() => {
        const nav = document.querySelector('nav[aria-label="Groups"]');
        const one = nav?.querySelector("button");
        return {
          words: one?.textContent ?? "",
          current: one?.getAttribute("aria-current") ?? "",
        };
      })(),
    };
  });
}

/** The setting whose row holds the keyboard, if one does. */
async function focusedSetting(): Promise<string> {
  return browser.execute(
    () => document.activeElement?.closest<HTMLElement>("[data-setting]")?.dataset.setting ?? "",
  );
}

/** Presses the Set here toggle, asked for by its role and name, and says what it is now. */
async function pressSetHere(): Promise<string> {
  await browser.execute(() => {
    const toggle = [...document.querySelectorAll<HTMLElement>('[role="button"], button')].find(
      (one) => one.textContent?.trim() === "Set here",
    );
    toggle?.click();
  });
  return browser.execute(
    () =>
      [...document.querySelectorAll<HTMLElement>('[role="button"], button')]
        .find((one) => one.textContent?.trim() === "Set here")
        ?.getAttribute("aria-pressed") ?? "",
  );
}

/** Types `words` into the search box, from empty. */
async function search(words: string): Promise<void> {
  const box = await $(BOX);
  await box.waitForDisplayed({ timeout: 20_000 });
  await box.clearValue();
  await box.addValue(words);
}

describe("one settings search across every level, in the real WebView", () => {
  closesTheTabsItOpens();

  it("finds what the words name at every level, on the nav's first page", async () => {
    await openProjectSettings();
    await search("theme");

    await browser.waitUntil(
      async () => {
        const { sections } = await found();
        return sections.includes("You › Theme") && sections.includes("Project › Appearance");
      },
      { timeout: 20_000, timeoutMsg: "the search never found You's and the project's themes" },
    );
    const { title, first } = await found();
    expect(title).toMatch(/^\d+ settings found$/);
    expect(first.words).toMatch(/^All results/);
    expect(first.current).toBe("page");
  });

  it("lands on a field at another level, with the keyboard in it", async () => {
    await search("window text size");
    const go = await $('[aria-label="Go to Text: Window text size"]');
    await go.waitForDisplayed({ timeout: 20_000 });
    await go.click();

    await browser.waitUntil(async () => (await focusedSetting()) === "you.text.window", {
      timeout: 20_000,
      timeoutMsg: "the keyboard never landed on You's window text size",
    });
    expect(await $(BOX).getValue()).toBe("");
  });

  it("narrows to what this level's files set, with Set here", async () => {
    await settingsAt("Project", "Saving");
    await $(SETTINGS_GROUPS).waitForDisplayed({ timeout: 20_000 });
    expect(await pressSetHere()).toBe("true");

    await browser.waitUntil(
      async () => (await found()).rows.some((one) => one.setting === "project.saving.memory.share"),
      { timeout: 20_000, timeoutMsg: "Set here never listed [memory] share" },
    );
    const { rows, sections } = await found();
    expect(sections.every((one) => one.startsWith("Project › "))).toBe(true);
    expect(rows.every((one) => one.setHere)).toBe(true);

    expect(await pressSetHere()).toBe("false");
    await browser.waitUntil(async () => (await found()).title === "", {
      timeout: 10_000,
      timeoutMsg: "Set here, pressed again, left the matches on screen",
    });
  });

  it("lands on a field from the palette's row for it", async () => {
    await runFromThePalette(
      "Saving › Target branch",
      "settings.setting:project.saving.plane.branch",
    );

    await browser.waitUntil(
      async () => (await focusedSetting()) === "project.saving.plane.branch",
      { timeout: 20_000, timeoutMsg: "the palette's row never landed on Target branch" },
    );
  });
});
