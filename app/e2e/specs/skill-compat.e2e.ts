// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { projectRoot } from "../acting.js";
import { chatsRunning, endTheChatsStartedSince, type Running } from "../chatsLeft.js";
import {
  closeTheViewTabsOpenedSince,
  focusWorkspace,
  runFromThePalette,
  tabNames,
} from "../reach.js";
import { studioSkill } from "../studio.js";
import { startAChat } from "../started.js";

/**
 * **A skill's compatibility badges, and a skill that cannot be loaded** (SK-8, #2771), in the
 * built app against the real core, with the `skill-studio` flag on for the run (`wdio.conf.ts`).
 *
 * The spec puts two project skills in the project's `skills/`, granted to every workspace, as a
 * person would: one that asks for what would widen a chat (`allowed-tools`) and for a model, and
 * one whose frontmatter names no description, and one that spells a widening key another way
 * (`allowedTools`). The Skill studio draws the first with a badge per harness: partial on Claude
 * Code, which is never handed what widens the chat, and partial on Codex, which applies no model
 * either; and the third as handed to no chat. Then a chat in `alpha`, on the scenario's program declared
 * as Claude Code, starts without the second: its pane's Notice names it with why, and its Open in
 * Skill studio opens the studio. What each harness is handed is proved on its arguments, its
 * environment and its folder in purlis-core (`a_skill_can_only_narrow_a_chat.rs`).
 *
 * **It leaves the window and the project as it found them.**
 */

const WIDENS = "e2e-compat-widens";
const BROKEN = "e2e-compat-broken";
const REFUSED = "e2e-compat-refused";

describe("a skill's compatibility and a skill that cannot be loaded", function () {
  this.timeout(240_000);

  const written: string[] = [];
  let local = "";
  let declared = "";
  let wereOpen: string[] = [];
  let wereRunning: Running = {};

  before(async () => {
    const root = await projectRoot();
    const skills = join(root, "skills");
    const widens = join(skills, WIDENS);
    mkdirSync(widens, { recursive: true });
    writeFileSync(
      join(widens, "SKILL.md"),
      `---\nname: ${WIDENS}\ndescription: Probes the compatibility badges.\nallowed-tools: Bash\nmodel: opus\n---\n\nSay hello.\n`,
    );
    const broken = join(skills, BROKEN);
    mkdirSync(broken, { recursive: true });
    writeFileSync(join(broken, "SKILL.md"), `---\nname: ${BROKEN}\n---\n\nSay hello.\n`);
    const refused = join(skills, REFUSED);
    mkdirSync(refused, { recursive: true });
    writeFileSync(
      join(refused, "SKILL.md"),
      `---\nname: ${REFUSED}\ndescription: Spells a widening key another way.\nallowedTools: Bash\n---\n\nSay hello.\n`,
    );
    for (const name of [WIDENS, BROKEN, REFUSED]) {
      const grant = join(skills, `${name}.toml`);
      writeFileSync(grant, 'grant = "all"\n');
      written.push(grant);
    }
    written.push(widens, broken, refused);
    local = join(root, "charter.local.toml");
    declared = readFileSync(local, "utf8");
    await focusWorkspace("alpha");
    wereOpen = await tabNames();
    wereRunning = await chatsRunning();
  });

  after(async () => {
    await endTheChatsStartedSince(wereRunning);
    await closeTheViewTabsOpenedSince(wereOpen);
    if (declared !== "") writeFileSync(local, declared);
    for (const path of written) rmSync(path, { recursive: true, force: true });
  });

  it("draws a badge per harness that says what is left out and what is not applied", async () => {
    await runFromThePalette("Open Skill studio", "skills.studio");
    // How each program takes a skill is said beside the Library, once it is chosen (#3681).
    await studioSkill(WIDENS);

    const claude = await $(`[data-testid="compat-${WIDENS}-claude"]`);
    await claude.waitForExist({ timeout: 20_000 });
    await expect(claude).toHaveText(/Claude Code: partial/);
    await (await claude.$("summary")).click();
    await expect(claude).toHaveText(/a skill may only narrow a chat/);

    const codex = await $(`[data-testid="compat-${WIDENS}-codex"]`);
    await expect(codex).toHaveText(/Codex: partial/);
    await (await codex.$("summary")).click();
    await expect(codex).toHaveText(/Ignored: the model it asks for/);

    // A widening key spelled another way: purlis hands the skill to no chat, and says so.
    await studioSkill(REFUSED);
    const refused = await $(`[data-testid="compat-${REFUSED}-claude"]`);
    await expect(refused).toHaveText(/Claude Code: handed to no chat/);
  });

  it("says on a chat's pane that it started without a skill that could not be loaded", async () => {
    // The scenario's own program, declared as Claude Code, as `use-skill.e2e.ts` declares it.
    const command = /command = (\[.*\])/.exec(declared)?.[1];
    if (!command) throw new Error(`no command in ${local}: ${declared}`);
    writeFileSync(
      local,
      `${declared.replace('default = "scenario"', 'default = "compat-claude"')}\n` +
        `[harness.compat-claude]\nkind = "claude"\ncommand = ${command}\n`,
    );
    await browser.execute(() => window.dispatchEvent(new Event("focus")));
    await startAChat();

    const notice = await $('[data-cause^="skills-missing:"]');
    await notice.waitForExist({ timeout: 40_000 });
    await expect(notice).toHaveText(/could not be loaded/);
    await expect(notice).toHaveText(new RegExp(`${BROKEN} \\(project\\)`));
    await expect(notice).toHaveText(
      new RegExp(`${REFUSED} \\(project\\), purlis hands it to no chat`),
    );
    await expect(notice).toHaveText(/names no `description`/);
    const open = await notice.$("button=Open Skill studio");
    await open.waitForClickable({ timeout: 10_000 });
    await open.click();
    await browser.waitUntil(
      async () => (await tabNames()).some((tab) => tab.startsWith("Skill studio")),
      {
        timeout: 20_000,
        timeoutMsg: "Open Skill studio never opened the studio",
      },
    );
  });
});
