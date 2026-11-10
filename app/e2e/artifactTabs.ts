// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";

/**
 * **An artifact's icon and its tab, as every artifact spec reaches them** (ADR 0122, #5952).
 *
 * One copy of the selectors, as `opening.ts` is for the picker, so a change to how the group or
 * the tab strip is drawn is one change here. Each asks for a control by where it stands (a
 * `toolbar`, the tab strip) and by its accessible name, never by its tag: the icon and the tab's
 * Close are buttons today, and a selector naming `button` would stop matching the day either is
 * drawn otherwise while the operator and a screen reader still get the same control.
 */

/** The window's strip of tabs. */
export const TABS = '[data-strip="Tabs"]';

/** A workspace's pinned artifacts: the toolbar their icons stand in. */
export const GROUP = '[role="toolbar"][aria-label="Artifacts"]';

/** The icon of the artifact named `name`, in its workspace's group. */
export const iconOf = (name: string): string => `${GROUP} [aria-label="${name}"]`;

/** The Close of the tab of the artifact named `name`. */
export const closeOf = (name: string): string => `${TABS} [aria-label="Close ${name}"]`;

/** Waits for the icon of the artifact named `name` to be drawn, and returns it. */
export async function iconFor(name: string, within = 30_000) {
  const icon = await $(iconOf(name));
  await icon.waitForExist({ timeout: within, timeoutMsg: `${name} has no icon in its group` });
  return icon;
}

/** Closes the tab of the artifact named `name`, if one is open. Says whether one was. */
export async function closeTabOf(name: string): Promise<boolean> {
  const close = await $(closeOf(name));
  if (!(await close.isExisting())) return false;
  await close.click();
  return true;
}

/**
 * **Whether an artifact's script runs where the app under test runs** (ADR 0122 §10, amended
 * 2026-11-10): on macOS alone. The scenario tests start the app on the machine they run on, so
 * this is the app's platform. Elsewhere a scripted page is shown without its script, and each
 * case that needs a page's script asserts that refusal ({@link expectScriptRefusedHere}) instead.
 */
export const SCRIPT_RUNS_HERE = process.platform === "darwin";

/** What purlis's bar says of a scripted page where script does not run. */
export const SCRIPT_REFUSED_HERE =
  "Shown without its script: scripted artifacts do not run on this system yet";

/**
 * Asserts that the tab in front is a scripted page refused on this platform: its bar says so in
 * its sentence, offers no approval, and its frame's edge says its script neither runs nor asks.
 */
export async function expectScriptRefusedHere(): Promise<void> {
  const bar = $('[data-testid="artifact-bar"]');
  await browser.waitUntil(async () => (await bar.getText()).includes(SCRIPT_REFUSED_HERE), {
    timeout: 20_000,
    timeoutMsg: "the bar did not say the page's script is refused on this platform",
  });
  expect(await bar.$("aria/Approve script").isExisting()).toBe(false);
  expect(await $('[data-testid="artifact-edge"]').getAttribute("data-standing")).toBe("plain");
}
