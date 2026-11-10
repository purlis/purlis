// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $ } from "@wdio/globals";

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
