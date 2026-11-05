// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { closesTheTabsItOpens } from "../reach.js";
import { SETTINGS_GROUPS, openProjectSettings } from "../settingsTab.js";
import { theProfilesWrapper } from "../harness.js";

/**
 * **Settings › Harness & profiles adds a profile, gives it a page, and removes it**, in the built
 * app (#1290's last line; ST-4, #1236).
 *
 * Opened the way the operator opens it, the palette's Project settings…, on the copy of the
 * fixture project this run was given. A profile is added through Harness & profiles' Add form,
 * `charter.local.toml` on disk is read to see the table the core wrote, the profile's own page is
 * opened from the nav, and its Remove takes the table out of the file again.
 *
 * What is proved here and not in jsdom: the profiles collection's commands exist, write the
 * project's real local settings file through `purlis_core::settings::harness_profiles`, and the
 * window reads back what they wrote, page and all. `ProfilesCollection.test.tsx` holds the rest.
 *
 * **It leaves the window and the project as it found them**: one app process serves the whole
 * run, so `charter.local.toml` is put back byte for byte whatever happened, and a Settings tab
 * this spec opened is closed.
 */

const NAME = "e2e-profile";
const TABLE = `[harness.${NAME}]`;

/** The project the app says it is acting on: the copy this run was given, never the repo's. */
async function planeRoot(): Promise<string> {
  // The status line says the project by its name; its directory is on the item (qw357).
  const said = await $('[data-testid="status-project"]');
  await said.waitForDisplayed({ timeout: 30_000 });
  return ((await said.getAttribute("data-directory")) ?? "").trim();
}

/** The local settings file as it is on disk now, or `null` when there is none. */
const onDisk = (file: string): string | null =>
  existsSync(file) ? readFileSync(file, "utf8") : null;

describe("Settings › Harness & profiles", () => {
  let root = "";
  let file = "";
  let original: string | null = null;

  before(async () => {
    root = await planeRoot();
    file = join(root, "charter.local.toml");
    original = onDisk(file);
  });

  after(async () => {
    // Whatever happened, the file is what it was before this spec: put back, or taken away
    // again when there was none. A `before` that failed read nothing, so touches nothing.
    if (file !== "") {
      const now = onDisk(file);
      if (original === null) {
        if (now !== null) rmSync(file);
      } else if (now !== original) writeFileSync(file, original);
    }
  });

  closesTheTabsItOpens();

  it("adds a profile that lands in charter.local.toml with a page of its own, and removes it", async () => {
    expect(original ?? "").not.toContain(TABLE);
    await openProjectSettings();
    await $(SETTINGS_GROUPS).$("button=Harness & profiles").click();

    await $("button=Add profile").click();
    const form = await $('form[aria-label="New profile"]');
    await form.waitForDisplayed({ timeout: 10_000 });
    await form.$("label=Name").click();
    await browser.keys(NAME.split(""));
    // The stand-in the run's own profiles start (`declareAProfile`): a program that is there.
    await form.$("label=Command").click();
    await browser.keys(theProfilesWrapper(root).split(""));
    await form.$("button=Add profile").click();

    await browser.waitUntil(async () => (onDisk(file) ?? "").includes(TABLE), {
      timeout: 20_000,
      timeoutMsg: `charter.local.toml never held ${TABLE}`,
    });
    const written = onDisk(file) ?? "";
    expect(written).toContain('kind = "claude"');
    expect(written).toContain(theProfilesWrapper(root));

    // Its own page, from the nav, which is where its Remove is.
    const page = await $(SETTINGS_GROUPS).$(`button=${NAME}`);
    await page.waitForDisplayed({
      timeout: 20_000,
      timeoutMsg: `the nav never listed ${NAME}'s page`,
    });
    await page.click();
    const remove = await $(`button[aria-label="Remove ${NAME}"]`);
    await remove.waitForDisplayed({ timeout: 20_000 });

    await remove.click();
    await browser.waitUntil(async () => !(onDisk(file) ?? "").includes(TABLE), {
      timeout: 20_000,
      timeoutMsg: `charter.local.toml still holds ${TABLE}`,
    });
    // The profiles the run started with are still there, and the page went with the profile.
    expect(onDisk(file) ?? "").toContain("[harness.scenario]");
    await $(SETTINGS_GROUPS)
      .$(`button=${NAME}`)
      .waitForExist({
        timeout: 20_000,
        reverse: true,
        timeoutMsg: `the nav still lists ${NAME}'s page after it was removed`,
      });
  });
});
