// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { press, projectRoot, typeInto, untilSays } from "../acting.js";
import { showView } from "../opening.js";
import { closeTheViewTabsOpenedSince, tabNames } from "../reach.js";

/**
 * **A persona's Skills section** (SK-5, #2768), in the built app against the real core, with the
 * `skill-studio` flag on for the run (`wdio.conf.ts`).
 *
 * The spec gives `steward` a skill of its own and names a project skill granted to no workspace
 * in its `skills:` line, as a person would. `steward`'s tab lists both under Skills. Its Add
 * names one more skill, written into `persona.md`; Remove takes it out again. A row opens the
 * Skill studio on a chat as `steward`, where the project skill it names is on.
 *
 * **It leaves the window and the project as it found them**: `persona.md` is written back, the
 * skills are removed, and every tab it opened is closed.
 */

const PERSONA = "steward";
const PANE = `[data-testid="view-pane-charter-persona-${PERSONA}"]`;
const OWN = "e2e-steward-own-probe";
const NAMED = "e2e-steward-named-probe";
const ADDED = "e2e-steward-added-probe";

function writeSkill(dir: string, name: string): string {
  const folder = join(dir, name);
  mkdirSync(folder, { recursive: true });
  writeFileSync(
    join(folder, "SKILL.md"),
    `---\nname: ${name}\ndescription: Probes ${name}.\n---\n\nSay hello.\n`,
  );
  return folder;
}

describe("a persona's skills", function () {
  this.timeout(180_000);

  const written: string[] = [];
  let file = "";
  let was = "";
  let wereOpen: string[] = [];

  before(async () => {
    const root = await projectRoot();
    file = join(root, "personas", PERSONA, "persona.md");
    was = readFileSync(file, "utf8");
    written.push(writeSkill(join(root, "personas", PERSONA, "skills"), OWN));
    written.push(writeSkill(join(root, "skills"), NAMED));
    // Its own `skills:` line, whatever it said: a later line of a key answers instead of an
    // earlier one.
    writeFileSync(file, was.replace(/^skills:.*\n/m, "").replace(/^---\n/, `---\nskills: ${NAMED}\n`));
    wereOpen = await tabNames();
    await showView("Personas");
    await untilSays('[data-testid="panel-personas"]', PERSONA);
    await press('[data-testid="panel-personas"]', PERSONA, "containing");
    await untilSays(PANE, NAMED);
  });

  after(async () => {
    writeFileSync(file, was);
    for (const path of written) rmSync(path, { recursive: true, force: true });
    await closeTheViewTabsOpenedSince(wereOpen);
    await showView("Inbox");
  });

  it("lists its own skill and the project skill it names", async () => {
    await untilSays(PANE, OWN);
    await untilSays(PANE, "into every workspace");
  });

  it("adds a skill by name, and removes it again", async () => {
    await typeInto(`${PANE} [data-testid="persona-skill-name"]`, ADDED);
    const add = await $(
      `${PANE} form[aria-label="Add a skill to ${PERSONA}"] button[type="submit"]`,
    );
    await add.waitForClickable({ timeout: 10_000 });
    await add.click();
    await browser.waitUntil(async () => readFileSync(file, "utf8").includes(ADDED), {
      timeout: 20_000,
      timeoutMsg: "Add never wrote the skill into persona.md",
    });
    expect(readFileSync(file, "utf8")).toMatch(new RegExp(`skills: ${NAMED}, ${ADDED}`));

    const remove = await $(`${PANE} button[aria-label="Remove ${ADDED} from ${PERSONA}"]`);
    await remove.waitForClickable({ timeout: 20_000 });
    await remove.click();
    await browser.waitUntil(async () => !readFileSync(file, "utf8").includes(ADDED), {
      timeout: 20_000,
      timeoutMsg: "Remove never took the skill out of persona.md",
    });
  });

  it("opens the Skill studio on a chat as the persona", async () => {
    await press(PANE, NAMED, "starting");

    const named = await $(`[data-testid="skill-${NAMED}"]`);
    await named.waitForExist({ timeout: 20_000 });
    await expect(named).toHaveText(/On/);
    const own = await $(`[data-testid="skill-${OWN}"]`);
    await expect(own).toHaveText(/persona/);
    expect(await tabNames()).toContain(`Skill studio · ${PERSONA}`);
  });
});
