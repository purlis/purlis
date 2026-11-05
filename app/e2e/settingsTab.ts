// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser } from "@wdio/globals";

const PALETTE = '[role="dialog"][aria-label="Command palette"]';

/** The Settings tab's list of groups, drawn once it has opened. */
export const SETTINGS_GROUPS = 'nav[aria-label="Groups"]';

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
