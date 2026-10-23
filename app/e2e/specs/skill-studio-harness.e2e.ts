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

/**
 * **A repo's own skills, and each program's, in the Skill studio** (SK-6, #2769), in the built
 * app against the real core, with the `skill-studio` flag on for the run (`wdio.conf.ts`).
 *
 * The spec puts a clone in `alpha` as git leaves one (a `.git` folder) holding a committed
 * `.claude/skills/<name>`, and a project skill of the same name granted to `alpha`. It opens
 * `alpha`'s Settings at Plugins, which is one summary row once the studio is on, and follows
 * Open in Skill studio. There Claude Code's section lists the repo's skill, owned by the repo, and
 * the project's copy says a chat gets both. Turn off leaves the repo's skill out of `alpha`'s
 * chats, in `alpha`'s own settings, and Turn on takes that back out again.
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

  it("summarises plugins on Settings and opens the studio, where the repo's skill is listed", async () => {
    await runFromThePalette(
      "Workspace settings: Plugins",
      "settings.group:workspace.plugins:alpha",
    );
    const summary = await $('[data-testid="workspace.plugins-summary"]');
    await summary.waitForExist({ timeout: 20_000 });
    await expect(summary).toHaveText(/Claude Code:/);

    const open = await $("button=Open in Skill studio");
    await open.waitForClickable({ timeout: 10_000 });
    await open.click();

    const found = await $(`[data-testid="found-claude-${NAME}"]`);
    await found.waitForExist({ timeout: 20_000 });
    await expect(found).toHaveText(new RegExp(`Repo ${REPO}`));
    await expect(found).toHaveText(/On/);
    const clash = await $(`[data-testid="clash-${NAME}"]`);
    await expect(clash).toHaveText(
      new RegExp(`the repo ${REPO} has a skill of this name too, and a chat there gets both`),
    );
    expect(await tabNames()).toContain("Skill studio · alpha");
  });

  it("turns the repo's skill off for the workspace's chats, and back on", async () => {
    const row = () => $(`[data-testid="found-claude-${NAME}"]`);
    // The row is drawn again after each switch, and again when the plane's change reaches the
    // window, so the button is found afresh on every try rather than held.
    const press = async (label: string, until: RegExp, what: string) => {
      try {
        await browser.waitUntil(
          async () => {
            if (until.test(await (await row()).getText())) return true;
            const button = await (await row()).$(`button=${label}`);
            if (
              (await button.isExisting()) &&
              (await button.isEnabled()) &&
              (await button.getAttribute("aria-disabled")) !== "true"
            ) {
              await button.click().catch(() => undefined);
            }
            return false;
          },
          { timeout: 30_000, interval: 1_000 },
        );
      } catch {
        // Said with what the row and the studio's notices read, so a red run says why.
        const said = await $(".skill-studio-harnesses")
          .getText()
          .catch(() => "");
        throw new Error(`${what}. The row reads: ${await (await row()).getText()}. ${said}`);
      }
    };

    await press(
      "Turn off",
      /Left out: turned off for this project's chats/,
      "the repo's skill never read as turned off",
    );
    expect(readFileSync(manifest, "utf8")).toContain(NAME);

    // The cells read run together ("…probeOn Turn off"), so on is the Turn off button being back.
    await press("Turn on", /On Turn off/, "the repo's skill never read as on again");
    await browser.waitUntil(async () => !readFileSync(manifest, "utf8").includes(NAME), {
      timeout: 10_000,
      timeoutMsg: "turning it back on left its key in workspace.json",
    });
  });
});
