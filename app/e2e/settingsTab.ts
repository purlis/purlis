// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser } from "@wdio/globals";

const PALETTE = '[role="dialog"][aria-label="Command palette"]';

/** The Settings tab's list of groups, drawn once it has opened. */
export const SETTINGS_GROUPS = 'nav[aria-label="Groups"]';

/**
 * Opens `file` from the Edit as TOML files at the nav's foot: the panel family's page navigation
 * named "Edit as TOML" (`ViewNav`, #2930), each file a button by its name.
 */
export async function openRawFile(file: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute((wanted: string) => {
        const files = document.querySelector('nav[aria-label="Edit as TOML"]');
        const link = [...(files?.querySelectorAll<HTMLElement>("button") ?? [])].find(
          (one) => one.textContent?.trim() === wanted,
        );
        if (link === undefined) return false;
        link.click();
        return true;
      }, file),
    { timeout: 20_000, interval: 250, timeoutMsg: `Edit as TOML offered no ${file}` },
  );
}

/** Opens Settings at the Project level through the palette, as the operator does. */
export async function openProjectSettings(): Promise<void> {
  await browser.keys(["F2"]);
  await $(PALETTE).waitForDisplayed({ timeout: 20_000 });
  const box = await $("#palette-query");
  await browser.waitUntil(async () => box.isFocused(), {
    timeout: 10_000,
    timeoutMsg: "the palette did not take the keyboard when it opened",
  });
  await box.addValue("Project settings");
  await browser.waitUntil(
    async () =>
      browser.execute(
        () =>
          document
            .querySelector('[role="option"][aria-selected="true"] .palette-title')
            ?.textContent?.startsWith("Project settings") ?? false,
      ),
    { timeout: 10_000, timeoutMsg: "the palette did not offer Project settings…" },
  );
  await browser.keys(["Enter"]);
  await $(SETTINGS_GROUPS).waitForDisplayed({ timeout: 20_000 });
}

/**
 * **Shows Settings' `group` at `level`** (You, Project, Workspace or Persona), as a person picks
 * them: the level on its switcher, then the group in its list, until the group says it is the
 * one shown. Settings is open already ({@link openProjectSettings}).
 *
 * Each control is asked for by its role (CLAUDE.md): said outright, or the native element whose
 * implicit role it is and which says no other. The group shown says so with `aria-current`, read
 * as any value but `false`: Settings' groups moved onto the panel family's view nav, which says
 * `page` where the old list said `true` (#2930).
 */
export async function settingsAt(level: string, group: string): Promise<void> {
  await browser.execute((called: string) => {
    const radio = [
      ...document.querySelectorAll<HTMLElement>(
        '[role="radiogroup"][aria-label="Level"] [role="radio"], [role="radiogroup"][aria-label="Level"] input[type="radio"]:not([role])',
      ),
    ].find((one) => (one.closest("label")?.textContent ?? one.textContent)?.trim() === called);
    if (
      radio?.getAttribute("aria-checked") !== "true" &&
      !(radio as HTMLInputElement | undefined)?.checked
    )
      radio?.click();
  }, level);
  await browser.waitUntil(
    async () =>
      browser.execute((called: string) => {
        const button = [
          ...document.querySelectorAll<HTMLElement>(
            'nav[aria-label="Groups"] [role="button"], nav[aria-label="Groups"] button:not([role])',
          ),
          // By the page's name alone: a page with things waiting says how many after it (R46).
        ].find(
          (one) => (one.querySelector('[data-part="label"]') ?? one).textContent?.trim() === called,
        );
        button?.click();
        const current = button?.getAttribute("aria-current");
        return current !== null && current !== undefined && current !== "false";
      }, group),
    { timeout: 20_000, interval: 250, timeoutMsg: `Settings never showed ${level} › ${group}` },
  );
}
