// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, readdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { press, projectRoot, stateOf } from "../acting.js";
import {
  closeTheViewTabsOpenedSince,
  focusWorkspace,
  runFromThePalette,
  shownButton,
  tabNames,
} from "../reach.js";
import { studioPage, studioMore, studioSkill, studioTab } from "../studio.js";

/**
 * **Making, renaming, deleting and restoring a skill in the Skill studio** (SK-7, #2770), in the
 * built app against the real core, with the `skill-studio` flag on for the run (`wdio.conf.ts`).
 *
 * The spec makes a skill with New skill at the project's root, where the studio was opened, and
 * finds it as a project skill with its folder on disk. It renames it and finds the folder under
 * its new name, deletes it after Delete has said who uses it, and restores it from the studio's
 * Deleted page. Each change is made on the skill chosen in the Library, beside the list (#3681).
 * The run's project names no save mode, so each change stays on disk for the next save. That each
 * one is a single commit is the core's seam test (`skills/verbs/tests.rs`).
 *
 * **It leaves the window and the project as it found them.**
 */

const MADE = "e2e-made-probe";
const RENAMED = "e2e-renamed-probe";

/** Types `text` into the box labelled `label`. */
async function typeInto(label: string, text: string) {
  const box = await $(`aria/${label}`);
  await box.waitForDisplayed({ timeout: 10_000 });
  await box.clearValue();
  await box.addValue(text);
}

describe("making and changing a skill in the Skill studio", function () {
  this.timeout(180_000);

  let root = "";
  let keptBefore: string[] = [];
  let wereOpen: string[] = [];
  const kept = () => {
    const dir = join(stateOf(root), "skills-deleted");
    return existsSync(dir) ? readdirSync(dir) : [];
  };

  before(async () => {
    root = await projectRoot();
    keptBefore = kept();
    await focusWorkspace("alpha");
    wereOpen = await tabNames();
  });

  after(async () => {
    await closeTheViewTabsOpenedSince(wereOpen);
    for (const name of [MADE, RENAMED])
      rmSync(join(root, "skills", name), { recursive: true, force: true });
    for (const one of kept())
      if (!keptBefore.includes(one))
        rmSync(join(stateOf(root), "skills-deleted", one), { recursive: true, force: true });
  });

  it("makes a skill at the project's root, renames it, deletes it and restores it", async () => {
    await runFromThePalette("Open Skill studio", "skills.studio");

    // New skill… asks how first; Write it yourself lands where the person came from (#3684).
    const make = await shownButton("New skill…");
    await make.waitForClickable({ timeout: 20_000 });
    await make.click();
    await press('[data-testid="skill-new"]', "Write it yourself");
    await typeInto("Name", MADE);
    await typeInto("What it does, and when a chat should use it", "Probes making a skill.");
    await expect($('[data-testid="skill-new-write"]')).toHaveText(
      new RegExp(`${MADE} is made in the project skills`),
    );
    await press('[data-testid="skill-new-write"]', "Make it and open its page");

    const made = await $(`[data-testid="skill-${MADE}"]`);
    await made.waitForExist({ timeout: 20_000 });
    await expect(made).toHaveText(/project/);
    expect(existsSync(join(root, "skills", MADE, "SKILL.md"))).toBe(true);
    // Its page opens with its text in the editor; Overview says who gets it.
    await $('[data-testid="skill-editor"]').waitForExist({ timeout: 20_000 });
    const item = await studioSkill(MADE);
    await studioTab("Overview");
    await expect(item.$('[data-testid="skill-in-a-chat"]')).toHaveText(/^On/);

    await studioMore(MADE, "Rename");
    await typeInto("New name", RENAMED);
    // Rename is a dialog that says what changes with it (#3685).
    await press('[data-testid="skill-rename"]', "Rename");
    const renamed = await $(`[data-testid="skill-${RENAMED}"]`);
    await renamed.waitForExist({ timeout: 20_000 });
    expect(existsSync(join(root, "skills", RENAMED, "SKILL.md"))).toBe(true);
    expect(existsSync(join(root, "skills", MADE))).toBe(false);

    await studioSkill(RENAMED);
    await studioMore(RENAMED, "Delete");
    const users = await $('[data-testid="skill-users"]');
    await users.waitForExist({ timeout: 20_000 });
    await expect(users).toHaveText(/Chats at the project's root/);
    await press('[data-testid="skill-delete"]', "Delete");
    await renamed.waitForExist({ reverse: true, timeout: 20_000 });
    expect(existsSync(join(root, "skills", RENAMED))).toBe(false);

    await studioPage("Deleted");
    // A deleted skill's page, beside the list, holds Bring back (S9, #3691).
    const row = await $(`[data-testid="deleted-${RENAMED}"]`);
    await row.waitForClickable({ timeout: 20_000 });
    await row.click();
    const restore = await $(`aria/Bring back ${RENAMED}`);
    await restore.waitForClickable({ timeout: 20_000 });
    await restore.click();
    await studioPage("Library");
    await browser.waitUntil(
      async () => (await $(`[data-testid="skill-${RENAMED}"]`)).isExisting(),
      { timeout: 20_000, timeoutMsg: "the restored skill never came back to the studio" },
    );
    expect(existsSync(join(root, "skills", RENAMED, "SKILL.md"))).toBe(true);
  });
});
