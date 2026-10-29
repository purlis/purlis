// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, expect } from "@wdio/globals";
import { projectRoot } from "../acting.js";
import {
  closeTheViewTabsOpenedSince,
  focusWorkspace,
  runFromThePalette,
  tabNames,
} from "../reach.js";
import { studioSkill } from "../studio.js";

/**
 * **A project skill in the Skill studio** (SK-2, #2765), in the built app against the real
 * core, with the `skill-studio` flag on for the run (`wdio.conf.ts`).
 *
 * The spec puts a skill folder in the project's `skills/`, as a person would, and opens the
 * studio the way a person does: `F2`, "Open Skill studio". The studio lists the skill as a
 * project skill a chat at the project's root gets, and the built-in skills beside it. What each
 * harness is handed is proved on the arguments and environment it is started with, in
 * purlis-core (`a_project_skill_reaches_a_chat_on_every_harness.rs`).
 *
 * **It leaves the window and the project as it found them.**
 */

const SKILL = "e2e-studio-probe";

describe("the Skill studio", function () {
  this.timeout(120_000);

  let folder = "";
  let wereOpen: string[] = [];

  before(async () => {
    folder = join(await projectRoot(), "skills", SKILL);
    mkdirSync(folder, { recursive: true });
    writeFileSync(
      join(folder, "SKILL.md"),
      `---\nname: ${SKILL}\ndescription: Probes the Skill studio.\n---\n\nSay hello.\n`,
    );
    await focusWorkspace("alpha");
    wereOpen = await tabNames();
  });

  after(async () => {
    await closeTheViewTabsOpenedSince(wereOpen);
    rmSync(folder, { recursive: true, force: true });
  });

  it("lists a project skill with its level, opened from the palette", async () => {
    await runFromThePalette("Open Skill studio", "skills.studio");

    const row = await $(`[data-testid="skill-${SKILL}"]`);
    await row.waitForExist({ timeout: 20_000 });
    await expect(row).toHaveText(/project/);
    await expect(row).toHaveText(/Probes the Skill studio\./);
    // Chosen, it opens beside the Library's list, saying a chat at the root gets it (#3681).
    const item = await studioSkill(SKILL);
    await expect(item.$('[data-testid="skill-in-a-chat"]')).toHaveText(/^On/);
    expect(await tabNames()).toContain("Skill studio");
  });
});
