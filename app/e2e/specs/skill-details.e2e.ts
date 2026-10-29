// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { projectRoot } from "../acting.js";
import {
  closeTheViewTabsOpenedSince,
  focusWorkspace,
  runFromThePalette,
  tabNames,
} from "../reach.js";
import { studioSkill } from "../studio.js";

/**
 * **A skill's tab: its state, history and uses** (SK-19, #2782), in the built app against the
 * real core, with the `skill-studio` flag on for the run (`wdio.conf.ts`).
 *
 * The spec lays a project skill on disk whose text names a skill there is none of, opens its tab
 * from the Skill studio, and finds the broken reference marked, the History panel drawn, and the
 * state "In use". It archives the skill, finds the tab saying it never loads and the state written
 * beside its folder, and restores it, which takes the state away again. Versions, diffs and
 * reverts on a real git history are the core's seam tests (`skills/history/tests.rs`).
 *
 * **It leaves the window and the project as it found them.**
 */

const PROBE = "e2e-life-probe";

describe("a skill's tab", function () {
  this.timeout(180_000);

  let root = "";
  let wereOpen: string[] = [];
  const folder = () => join(root, "skills", PROBE);
  const beside = () => join(root, "skills", `${PROBE}.toml`);

  before(async () => {
    root = await projectRoot();
    mkdirSync(folder(), { recursive: true });
    writeFileSync(
      join(folder(), "SKILL.md"),
      `---\nname: ${PROBE}\ndescription: Probes a skill's tab in the scenario run.\n---\n\nThen skills:e2e-gone-probe.\n`,
    );
    await focusWorkspace("alpha");
    wereOpen = await tabNames();
  });

  after(async () => {
    await closeTheViewTabsOpenedSince(wereOpen);
    rmSync(folder(), { recursive: true, force: true });
    rmSync(beside(), { force: true });
  });

  it("shows the skill's state, history and broken use, and archives and restores it", async () => {
    await runFromThePalette("Open Skill studio", "skills.studio");
    // Chosen in the Library, its own tab is one press away until its page is the studio's
    // (#3681, #3682).
    const item = await studioSkill(PROBE);
    const open = await item.$("button=History and runs");
    await open.waitForClickable({ timeout: 20_000 });
    await open.click();

    const uses = await $('[data-testid="skill-uses"]');
    await uses.waitForExist({ timeout: 20_000 });
    await expect(uses).toHaveText(/e2e-gone-probe — no skill has this name/);
    await expect($('[data-testid="skill-history"]')).toBeExisting();
    await expect($('[data-testid="skill-lifecycle"]')).toHaveText("In use.");

    await (await $('[data-testid="skill-state"]').$("button=Archive")).click();
    await browser.waitUntil(
      async () => /never loads/.test(await (await $('[data-testid="skill-lifecycle"]')).getText()),
      { timeout: 20_000, timeoutMsg: "the tab never said the archived skill never loads" },
    );
    expect(readFileSync(beside(), "utf8")).toContain('lifecycle = "archived"');

    await (await $('[data-testid="skill-state"]').$("button=Restore")).click();
    await browser.waitUntil(
      async () => (await (await $('[data-testid="skill-lifecycle"]')).getText()) === "In use.",
      { timeout: 20_000, timeoutMsg: "the restored skill's tab never said it is in use" },
    );
    expect(existsSync(beside())).toBe(false);
  });
});
