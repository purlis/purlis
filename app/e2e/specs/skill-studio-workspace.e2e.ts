// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { projectRoot } from "../acting.js";
import {
  closeTheViewTabsOpenedSince,
  focusWorkspace,
  runFromThePalette,
  shownButton,
  tabNames,
} from "../reach.js";
import { studioSkill } from "../studio.js";

/**
 * **A workspace's skills in the Skill studio** (SK-4, #2767), in the built app against the real
 * core, with the `skill-studio` flag on for the run (`wdio.conf.ts`).
 *
 * The spec writes three skills as a person would: a project skill granted to `alpha` in its
 * `<name>.toml`, a project skill granted to no workspace, and a skill of `alpha`'s own. It opens
 * `alpha`'s Settings at its Skills group, whose summary counts what `alpha`'s chats get, and
 * follows Open Skill studio: the studio's Library opens on `alpha`'s chats (#3681), with the
 * granted skill and the workspace's own on, and the other marked left out, with why beside it.
 *
 * **It leaves the window and the project as it found them.**
 */

const GRANTED = "e2e-granted-probe";
const KEPT = "e2e-root-only-probe";
const OWN = "e2e-alpha-probe";

function writeSkill(dir: string, name: string): string {
  const folder = join(dir, name);
  mkdirSync(folder, { recursive: true });
  writeFileSync(
    join(folder, "SKILL.md"),
    `---\nname: ${name}\ndescription: Probes ${name}.\n---\n\nSay hello.\n`,
  );
  return folder;
}

describe("a workspace's skills in the Skill studio", function () {
  this.timeout(120_000);

  const written: string[] = [];
  let wereOpen: string[] = [];

  before(async () => {
    const root = await projectRoot();
    const project = join(root, "skills");
    written.push(writeSkill(project, GRANTED), writeSkill(project, KEPT));
    const grant = join(project, `${GRANTED}.toml`);
    writeFileSync(grant, 'grant = ["alpha"]\n');
    written.push(grant);
    written.push(writeSkill(join(root, "workspaces", "alpha", "skills"), OWN));
    await focusWorkspace("alpha");
    wereOpen = await tabNames();
  });

  after(async () => {
    await closeTheViewTabsOpenedSince(wereOpen);
    for (const path of written) rmSync(path, { recursive: true, force: true });
  });

  it("counts them on the workspace's Settings and opens the studio filtered to it", async () => {
    await runFromThePalette("Workspace settings: Skills", "settings.group:workspace.skills:alpha");

    const summary = await $('[data-testid="workspace-skills-summary"]');
    await summary.waitForExist({ timeout: 20_000 });
    await browser.waitUntil(async () => /left out/.test(await summary.getText()), {
      timeout: 20_000,
      timeoutMsg: "the Skills summary never counted the skill left out",
    });

    const open = await shownButton("Open Skill studio");
    await open.waitForClickable({ timeout: 10_000 });
    await open.click();

    const granted = await $(`[data-testid="skill-${GRANTED}"]`);
    await granted.waitForExist({ timeout: 20_000 });
    await expect(granted).toHaveText(/project/);
    await expect(granted).not.toHaveText(/Left out/);
    const grantedItem = await studioSkill(GRANTED);
    await expect(grantedItem).toHaveText(/alpha/);
    await expect(grantedItem.$('[data-testid="skill-in-a-chat"]')).toHaveText(/^On/);
    const own = await $(`[data-testid="skill-${OWN}"]`);
    await expect(own).toHaveText(/workspace/);
    await expect(own).not.toHaveText(/Left out/);
    const kept = await $(`[data-testid="skill-${KEPT}"]`);
    await expect(kept).toHaveText(/Left out/);
    const keptItem = await studioSkill(KEPT);
    await expect(keptItem).toHaveText(/Left out: not granted to this workspace/);
    expect(await tabNames()).toContain("Skill studio");
  });
});
