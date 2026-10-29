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
import { studioMore, studioSkill } from "../studio.js";

/**
 * **A skill's page: its state, history and uses** (SK-19, #2782; its page in the studio, #3682),
 * in the built app against the real core, with the `skill-studio` flag on for the run
 * (`wdio.conf.ts`).
 *
 * The spec lays a project skill on disk whose text names a skill there is none of, opens its page
 * in the Skill studio, and finds the broken reference marked on Overview and its History drawn.
 * It archives the skill from the page's "…" menu, finds the banner saying it never loads and the
 * state written beside its folder, and unarchives it from the banner, which takes the state away
 * again. Versions, diffs and
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
    const item = await studioSkill(PROBE);

    const uses = item.$('[data-testid="skill-uses"]');
    await uses.waitForExist({ timeout: 20_000 });
    await expect(uses).toHaveText(/e2e-gone-probe — no skill has this name/);
    await (await item.$('[role="tab"]*=History')).click();
    await item.$('[data-testid="skill-history"]').waitForExist({ timeout: 20_000 });
    await expect(item.$('[data-cause^="skill-banner:"]')).not.toBeExisting();

    await studioMore(PROBE, "Archive");
    const dialog = $('[role="dialog"]');
    await dialog.waitForExist({ timeout: 20_000 });
    await (await dialog.$("button=Archive")).click();
    const banner = $('[data-testid="skill-banner-archived"]');
    await banner.waitForExist({ timeout: 20_000, timeoutMsg: "no banner said it is archived" });
    await expect(banner).toHaveText(/never loads/);
    expect(readFileSync(beside(), "utf8")).toContain('lifecycle = "archived"');

    await (await $("button=Unarchive")).click();
    await banner.waitForExist({
      reverse: true,
      timeout: 20_000,
      timeoutMsg: "the unarchived skill's banner never went",
    });
    expect(existsSync(beside())).toBe(false);
  });
});
