// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, expect } from "@wdio/globals";
import { projectRoot, stateOf } from "../acting.js";
import { closesTheTabsItOpens, focusWorkspace, runFromThePalette } from "../reach.js";
import { studioSkill, studioWhose, writeSkill } from "../studio.js";

/**
 * **A persona's skills in a workspace it was dispatched into** (#2935, #6491), in the built app
 * against the real core, with the `skill-studio` flag on for the run (`wdio.conf.ts`).
 *
 * The Skill studio's Whose chats picker lists the project's personas, and for one picked, the
 * workspaces it was dispatched into: what a chat dispatched there gets. A dispatch is recorded by
 * the app that made it (`<state>/app/dispatches/<id>.json`, `purlis_core::dispatchrecord`), and
 * a scenario cannot make a persona chat dispatch, so the spec writes one ended task's record, as
 * the Dispatches tab's spec does: `steward`, dispatched into `alpha` today. It gives `steward` a
 * skill of its own and `alpha` one of its own, as a person would.
 *
 * Opened on `alpha`'s chats, the picker's "A chat as steward" lists steward's skill and not alpha's;
 * In which workspace then offers "Dispatched into alpha", and there both are on.
 *
 * **It leaves the window and the project as it found them**: the record and the skills are
 * removed, and every tab it opened is closed.
 */

const PERSONA = "steward";
const WORKSPACE = "alpha";
const OWN = "e2e-steward-dispatched-probe";
const THERE = "e2e-alpha-dispatched-probe";
const DISPATCH = "01K7E2E0000000000000SKPRS1";

/** An ended task's record, as the app writes one (`dispatchrecord::Record`, version 1): steward,
 *  dispatched into alpha five minutes ago. */
function aTaskInAlpha(): string {
  const ended = new Date();
  const started = new Date(ended.getTime() - 5 * 60 * 1000);
  return JSON.stringify({
    v: 1,
    id: DISPATCH,
    mode: "task",
    asker: { chat: 9401, name: "e2e asking chat", workspace: WORKSPACE },
    persona: PERSONA,
    worker: { chat: 9402, name: `${PERSONA} e2e task`, persona: PERSONA },
    task: "e2e task in alpha",
    place: { workspace: WORKSPACE },
    brief: "Do the task in alpha.",
    started: started.toISOString(),
    ended: ended.toISOString(),
    report: { outcome: "done", text: "Did the task in alpha.", changed: {} },
  });
}

describe("a persona's skills in a workspace it was dispatched into", function () {
  this.timeout(180_000);

  const written: string[] = [];

  before(async () => {
    const root = await projectRoot();
    const store = join(stateOf(root), "app", "dispatches");
    mkdirSync(store, { recursive: true });
    const record = join(store, `${DISPATCH}.json`);
    writeFileSync(record, aTaskInAlpha());
    written.push(record);
    written.push(writeSkill(join(root, "personas", PERSONA, "skills"), OWN));
    written.push(writeSkill(join(root, "workspaces", WORKSPACE, "skills"), THERE));
    await focusWorkspace(WORKSPACE);
  });

  // Between the two: the strip is read once alpha is focused, and its view tabs are closed
  // before the files are tidied.
  closesTheTabsItOpens();

  after(() => {
    for (const path of written) rmSync(path, { recursive: true, force: true });
  });

  it("picks a chat as the persona, then the workspace it was dispatched into, and lists what a chat dispatched there gets", async () => {
    // The studio on alpha, as its Settings' Skills row opens it: a chat in the workspace.
    await runFromThePalette("Open Skill studio", `skills.studio:${WORKSPACE}`);
    await $(`[data-testid="skill-${THERE}"]`).waitForExist({ timeout: 20_000 });

    // As the persona, at the project's root: its own skill, and no workspace's.
    await studioWhose(`A chat as ${PERSONA}`, `A chat as ${PERSONA}`);
    const own = await $(`[data-testid="skill-${OWN}"]`);
    await own.waitForExist({ timeout: 20_000 });
    await expect(own).toHaveText(/persona/);
    await $(`[data-testid="skill-${THERE}"]`).waitForExist({
      reverse: true,
      timeout: 20_000,
      timeoutMsg: `${WORKSPACE}'s own skill stayed listed for a chat as ${PERSONA} at the root`,
    });

    // In the workspace it was dispatched into: the persona's skill and the workspace's, both on.
    await studioWhose(`Dispatched into ${WORKSPACE}`, `A chat as ${PERSONA} in ${WORKSPACE}`);
    const there = await $(`[data-testid="skill-${THERE}"]`);
    await there.waitForExist({ timeout: 20_000 });
    await expect(there).toHaveText(/workspace/);
    await expect(there).not.toHaveText(/Left out/);
    const thereItem = await studioSkill(THERE);
    await expect(thereItem.$('[data-testid="skill-in-a-chat"]')).toHaveText(/^On/);
    const ownItem = await studioSkill(OWN);
    await expect(ownItem.$('[data-testid="skill-in-a-chat"]')).toHaveText(/^On/);
  });
});
