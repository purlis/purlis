// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { press, projectRoot, stateOf, untilSays } from "../acting.js";
import { showView } from "../opening.js";
import { closeTheViewTabsOpenedSince, tabNames } from "../reach.js";
import { studioPage, studioMore, studioSkill } from "../studio.js";

/**
 * **A persona's own skills, changed in the Skill studio** (SK-32, #3003), in the built app
 * against the real core, with the `skill-studio` flag on for the run (`wdio.conf.ts`).
 *
 * The spec opens the studio on a chat as `steward` from its page, makes a skill there with New
 * skill, which lands in `steward`'s own skills, renames it, deletes it after Delete has said that
 * `steward`'s chats use it, and restores it from the Notice. Last it moves the skill into the
 * project's skills: the grant is asked first, and `steward`'s `skills:` line names it after, so
 * the persona keeps it. The run's project names no save mode, so each change stays on disk; that
 * each is one commit is the core's seam test (`skills/verbs/tests.rs`).
 *
 * **It leaves the window and the project as it found them**: `persona.md` is written back, every
 * folder it made is removed, and every tab it opened is closed.
 */

const PERSONA = "steward";
const PANE = `[data-testid="view-pane-charter-persona-${PERSONA}"]`;
const STUDIO = '[data-testid="skill-studio"]';
const SEED = "e2e-steward-seed-probe";
const MADE = "e2e-steward-made-probe";
const RENAMED = "e2e-steward-renamed-probe";

/** Types `text` into the box labelled `label`. */
async function typeInto(label: string, text: string) {
  const box = await $(`aria/${label}`);
  await box.waitForDisplayed({ timeout: 10_000 });
  await box.clearValue();
  await box.addValue(text);
}

describe("changing a persona's own skills", function () {
  this.timeout(240_000);

  let root = "";
  let file = "";
  let was = "";
  let keptBefore: string[] = [];
  let wereOpen: string[] = [];
  const kept = () => {
    const dir = join(stateOf(root), "skills-deleted");
    return existsSync(dir) ? readdirSync(dir) : [];
  };
  const own = (name: string) => join(root, "personas", PERSONA, "skills", name);

  before(async () => {
    root = await projectRoot();
    file = join(root, "personas", PERSONA, "persona.md");
    was = readFileSync(file, "utf8");
    keptBefore = kept();
    // One skill of its own, so its page offers the studio.
    mkdirSync(own(SEED), { recursive: true });
    writeFileSync(
      join(own(SEED), "SKILL.md"),
      `---\nname: ${SEED}\ndescription: Probes ${SEED}.\n---\n\nSay hello.\n`,
    );
    wereOpen = await tabNames();
    await showView("Personas");
    await untilSays('[data-testid="panel-personas"]', PERSONA);
    await press('[data-testid="panel-personas"]', PERSONA, "containing");
    await untilSays(PANE, "into every workspace");
  });

  after(async () => {
    writeFileSync(file, was);
    for (const name of [SEED, MADE, RENAMED]) {
      rmSync(own(name), { recursive: true, force: true });
      rmSync(join(root, "skills", name), { recursive: true, force: true });
      rmSync(join(root, "skills", `${name}.toml`), { force: true });
    }
    for (const one of kept())
      if (!keptBefore.includes(one))
        rmSync(join(stateOf(root), "skills-deleted", one), { recursive: true, force: true });
    await closeTheViewTabsOpenedSince(wereOpen);
    await showView("Inbox");
  });

  it("makes, renames, deletes and restores a skill of the persona's own", async () => {
    await press(PANE, "Brings", "starting");
    await $(STUDIO).waitForExist({ timeout: 20_000 });

    await press(STUDIO, "New skill…");
    await expect($('[data-testid="skill-new-level"]')).toHaveText(
      new RegExp(`It lands in the persona ${PERSONA} skills\\.`),
    );
    await typeInto("Name", MADE);
    await typeInto("What it does and when to use it", "Probes it.");
    await press(STUDIO, "Make it");
    const made = await $(`[data-testid="skill-${MADE}"]`);
    await made.waitForExist({ timeout: 20_000 });
    await expect(made).toHaveText(/persona/);
    expect(existsSync(join(own(MADE), "SKILL.md"))).toBe(true);

    // Each change is made on the skill chosen in the Library, beside the list (#3681).
    await studioSkill(MADE);
    await studioMore(MADE, "Rename");
    await typeInto("New name", RENAMED);
    // Rename is a dialog that says what changes with it (#3685).
    await press('[data-testid="skill-rename"]', "Rename");
    const renamed = await $(`[data-testid="skill-${RENAMED}"]`);
    await renamed.waitForExist({ timeout: 20_000 });
    expect(existsSync(join(own(RENAMED), "SKILL.md"))).toBe(true);
    expect(existsSync(own(MADE))).toBe(false);

    await studioSkill(RENAMED);
    await studioMore(RENAMED, "Delete");
    const users = await $('[data-testid="skill-users"]');
    await users.waitForExist({ timeout: 20_000 });
    await expect(users).toHaveText(new RegExp(`[Cc]hats as ${PERSONA}, in every workspace`));
    await press('[data-testid="skill-delete"]', "Delete");
    await renamed.waitForExist({ reverse: true, timeout: 20_000 });
    expect(existsSync(own(RENAMED))).toBe(false);

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
    expect(existsSync(join(own(RENAMED), "SKILL.md"))).toBe(true);
  });

  it("moves it into the project's skills after asking its grant, and the persona keeps it", async () => {
    // Move is a dialog of the levels, each with who gets it there (#3685); its first is the
    // project's shared skills.
    await studioSkill(RENAMED);
    await studioMore(RENAMED, "Move to another level");
    const prompt = '[data-testid="skill-move"]';
    // The level by its name, as a screen reader reads the radio, never by its tag.
    const project = await (await $(prompt)).$("aria/project");
    await project.waitForExist({ timeout: 20_000 });
    await project.click();
    await untilSays(`${prompt} [data-testid="persona-keeps"]`, `${PERSONA} keeps it`);
    await press(prompt, "Move");

    await browser.waitUntil(async () => existsSync(join(root, "skills", RENAMED, "SKILL.md")), {
      timeout: 20_000,
      timeoutMsg: "the skill never landed in the project's skills",
    });
    // The move puts the folder in place first and writes the persona's `skills:` line after
    // (`verbs::move_to`), so the folder alone does not say the line is written yet.
    const listed = new RegExp(`^skills:.*${RENAMED}`, "m");
    await browser.waitUntil(async () => listed.test(readFileSync(file, "utf8")), {
      timeout: 20_000,
      timeoutMsg: `${PERSONA}'s skills: line never named ${RENAMED} after the move`,
    });
    expect(existsSync(own(RENAMED))).toBe(false);
    expect(readFileSync(file, "utf8")).toMatch(listed);
  });
});
