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
import { studioPage, studioSkill } from "../studio.js";

/**
 * **A repo's own skills, and each program's, in the Skill studio** (SK-6, #2769), in the built
 * app against the real core, with the `skill-studio` flag on for the run (`wdio.conf.ts`).
 *
 * The spec puts a clone in `alpha` as git leaves one (a `.git` folder) holding a committed
 * `.claude/skills/<name>`, and a project skill of the same name granted to `alpha`. It opens
 * `alpha`'s Settings at Skills, and follows Open Skill studio, which lands on `alpha`'s Library
 * (#3681). There the project's copy says a chat gets both, and the Harness skills page lists
 * the repo's skill under Claude Code, owned by the repo; chosen, it opens beside the list with its
 * switch (#3692). Switching it off leaves the repo's skill out of `alpha`'s chats, in `alpha`'s own
 * settings, and switching it on takes that back out again.
 *
 * **It leaves the window and the project as it found them**: `workspace.json` is put back as it
 * was read.
 */

const NAME = "e2e-clash-probe";
const REPO = "e2e-repo-probe";

function writeSkill(dir: string, name: string, says: string): string {
  const folder = join(dir, name);
  mkdirSync(folder, { recursive: true });
  writeFileSync(
    join(folder, "SKILL.md"),
    `---\nname: ${name}\ndescription: ${says}\n---\n\nSay hello.\n`,
  );
  return folder;
}

describe("a repo's own skills in the Skill studio", function () {
  this.timeout(150_000);

  const written: string[] = [];
  let wereOpen: string[] = [];
  let manifest = "";
  let manifestWas: string | undefined;

  before(async () => {
    const root = await projectRoot();
    const project = join(root, "skills");
    written.push(writeSkill(project, NAME, "The project's copy."));
    const grant = join(project, `${NAME}.toml`);
    writeFileSync(grant, 'grant = ["alpha"]\n');
    written.push(grant);
    const clone = join(root, "workspaces", "alpha", REPO);
    mkdirSync(join(clone, ".git"), { recursive: true });
    writeSkill(join(clone, ".claude", "skills"), NAME, "The repo's copy.");
    written.push(clone);
    manifest = join(root, "workspaces", "alpha", "workspace.json");
    manifestWas = existsSync(manifest) ? readFileSync(manifest, "utf8") : undefined;
    await focusWorkspace("alpha");
    wereOpen = await tabNames();
  });

  after(async () => {
    await closeTheViewTabsOpenedSince(wereOpen);
    for (const path of written) rmSync(path, { recursive: true, force: true });
    if (manifestWas === undefined) rmSync(manifest, { force: true });
    else writeFileSync(manifest, manifestWas);
  });

  it("opens the studio from the workspace's Skills in Settings, where the repo's skill is listed", async () => {
    // #3467: Settings has no Plugins group any more; the Skills group is the way in.
    await runFromThePalette("Workspace settings: Skills", "settings.group:workspace.skills:alpha");

    const open = await $("button=Open Skill studio");
    await open.waitForClickable({ timeout: 10_000 });
    await open.click();

    await studioSkill(NAME);
    const clash = await $(`[data-testid="clash-${NAME}"]`);
    await expect(clash).toHaveText(
      new RegExp(`the repo ${REPO} has a skill of this name too, and a chat there gets both`),
    );
    expect(await tabNames()).toContain("Skill studio");

    await studioPage("Harness skills");
    const found = await $(`[data-testid="found-claude-${NAME}"]`);
    await found.waitForExist({ timeout: 20_000 });
    await expect(found).toHaveText(new RegExp(`Repo ${REPO}`));
    await expect(found).not.toHaveText(/Off here/);
    // The row chosen opens beside the list, with its switch (#3692).
    await found.click();
    const item = await $('[data-testid="harness-skill-item"]');
    await item.waitForExist({ timeout: 10_000 });
    await expect(item).toHaveText(new RegExp(NAME));
    await expect(item).toHaveText(/On for alpha's chats/);
  });

  it("turns the repo's skill off for the workspace's chats, and back on", async () => {
    const row = () => $(`[data-testid="found-claude-${NAME}"]`);
    const toggle = () =>
      $('[data-testid="harness-skill-item"] [data-scope="switch"][data-part="root"]');
    const named = () => readFileSync(manifest, "utf8").includes(NAME);
    // The item is drawn again after each switch, and again when the plane's change reaches the
    // window, so the switch is found afresh on every try rather than held. It is pressed only
    // while workspace.json is not yet as wanted, and at most once every few seconds: a press
    // that lands just as the item is redrawn could send the other way and undo the first
    // (#3139). The file is the signal the switch landed; the row is the signal the window caught
    // up.
    const press = async (until: (text: string) => boolean, keyed: boolean, what: string) => {
      let pressed = 0;
      try {
        await browser.waitUntil(
          async () => {
            if (named() === keyed) return until(await (await row()).getText());
            if (Date.now() - pressed < 5_000) return false;
            const button = await toggle();
            if (
              (await button.isExisting()) &&
              (await button.getAttribute("data-disabled")) === null
            ) {
              pressed = Date.now();
              await button.click().catch(() => undefined);
            }
            return false;
          },
          { timeout: 30_000, interval: 500 },
        );
      } catch {
        // Said with what the row, the item and the file read, so a red run says why.
        const said = await $('[data-testid="harness-skill-item"]')
          .getText()
          .catch(() => "");
        const file = `workspace.json ${named() ? "names it" : "does not name it"}`;
        const reads = await (await row()).getText();
        throw new Error(`${what}. ${file}. The row reads: ${reads}. ${said}`);
      }
    };

    await press(
      (text) => /Off here/.test(text),
      true,
      "turning the repo's skill off never showed in workspace.json and the row",
    );
    expect(readFileSync(manifest, "utf8")).toContain(NAME);
    await expect($('[data-testid="harness-skill-item"]')).toHaveText(/Left out:/);

    await press(
      (text) => !/Off here/.test(text),
      false,
      "turning the repo's skill back on never showed in workspace.json and the row",
    );
    expect(readFileSync(manifest, "utf8")).not.toContain(NAME);
  });
});
