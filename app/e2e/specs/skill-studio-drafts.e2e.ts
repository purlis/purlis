// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { projectRoot, stateOf } from "../acting.js";
import {
  closeTheViewTabsOpenedSince,
  endTheSessionsStartedSince,
  focusWorkspace,
  runFromThePalette,
  tabNames,
} from "../reach.js";
import { projectInFront, runningIn } from "../running.js";

/**
 * **Draft skills in the Skill studio** (SK-9, #2772), in the built app against the real core, with
 * the `skill-studio` flag on for the run (`wdio.conf.ts`).
 *
 * A chat's write to a skill's files is kept as a draft in this machine's state
 * (`skill-drafts/<id>/`, `docs/plane-format.md`); how a chat's write gets there is the core's seam
 * (`toolhooks` and `skills/verbs/drafts` tests), so the spec puts two drafts there as the app
 * keeps them. Then, in the studio:
 *
 * - each draft is listed, and **Changes** draws its file;
 * - **Reject** deletes one, and the project is untouched;
 * - **Try it** opens a scratch chat of its own in a throwaway folder, which is gone once that chat
 *   ends;
 * - **Accept** makes the other one a project skill, on disk, and the draft is gone;
 * - **Ask a chat to draft a skill** opens a chat.
 *
 * That an accept is one commit, and that a try's chat gets the built-in skills and the draft
 * alone, are the core's seam tests. **It leaves the window and the project as it found them.**
 */

const ACCEPTED = "e2e-drafted-probe";
const REJECTED = "e2e-rejected-probe";
/** Two draft ids, as the core names a draft's folder: sixteen hex characters. */
const ACCEPTED_ID = "e2e0000000000001";
const REJECTED_ID = "e2e0000000000002";

function skillText(name: string): string {
  return `---\nname: ${name}\ndescription: Probes a draft skill.\n---\n\nSay that the probe ran.\n`;
}

/** Puts a draft of a new project skill `name` where the app keeps a chat's draft. */
function drafted(state: string, id: string, name: string, chat: number): void {
  const at = join(state, "skill-drafts", id);
  mkdirSync(join(at, "files"), { recursive: true });
  writeFileSync(join(at, "files", "SKILL.md"), skillText(name));
  writeFileSync(
    join(at, "draft.json"),
    JSON.stringify({ level: "skills", folder: name, chat, bases: { "SKILL.md": null } }),
  );
}

/** The tries laid out now, by id. */
function tries(state: string): string[] {
  const dir = join(state, "skill-tries");
  return existsSync(dir) ? readdirSync(dir) : [];
}

describe("draft skills in the Skill studio", function () {
  this.timeout(240_000);

  let root = "";
  let state = "";
  let plane = "";
  let wereOpen: string[] = [];
  let wereRunning: number[] = [];

  before(async () => {
    root = await projectRoot();
    state = stateOf(root);
    await focusWorkspace("alpha");
    wereOpen = await tabNames();
    plane = await projectInFront();
    wereRunning = await runningIn(plane);
    drafted(state, ACCEPTED_ID, ACCEPTED, 1);
    drafted(state, REJECTED_ID, REJECTED, 1);
  });

  after(async () => {
    await endTheSessionsStartedSince(wereRunning);
    await closeTheViewTabsOpenedSince(wereOpen);
    for (const id of [ACCEPTED_ID, REJECTED_ID])
      rmSync(join(state, "skill-drafts", id), { recursive: true, force: true });
    rmSync(join(root, "skills", ACCEPTED), { recursive: true, force: true });
  });

  it("lists a chat's drafts, rejects one, tries and accepts the other", async () => {
    await runFromThePalette("Open Skill studio", "skills.studio");

    const kept = await $(`[data-testid="draft-${ACCEPTED}"]`);
    await kept.waitForExist({ timeout: 20_000 });
    await expect(kept).toHaveText(/a new skill/);
    await expect($(`[data-testid="draft-${REJECTED}"]`)).toExist();
    // A draft is never a skill any chat gets, nor a file in the project.
    await expect($(`[data-testid="skill-${ACCEPTED}"]`)).not.toExist();
    expect(existsSync(join(root, "skills", ACCEPTED))).toBe(false);

    await (await $(`aria/Changes in ${ACCEPTED}`)).click();
    await expect(
      $(`[data-testid="draft-${ACCEPTED}"] [data-testid="draft-file-SKILL.md"]`),
    ).toExist();

    await (await $(`aria/Reject ${REJECTED}`)).click();
    await $(`[data-testid="draft-${REJECTED}"]`).waitForExist({ reverse: true, timeout: 20_000 });
    expect(existsSync(join(state, "skill-drafts", REJECTED_ID))).toBe(false);
    expect(existsSync(join(root, "skills", REJECTED))).toBe(false);

    const triesBefore = tries(state);
    const tabsBefore = (await tabNames()).length;
    await (await $(`aria/Try ${ACCEPTED}`)).click();
    await browser.waitUntil(async () => (await tabNames()).length > tabsBefore, {
      timeout: 30_000,
      timeoutMsg: "Try it opened no scratch chat",
    });
    const laid = tries(state).filter((one) => !triesBefore.includes(one));
    expect(laid.length).toBe(1);
    await endTheSessionsStartedSince(wereRunning);
    await browser.waitUntil(async () => !tries(state).includes(laid[0]), {
      timeout: 30_000,
      timeoutMsg: "the try's throwaway folder outlived its chat",
    });

    await (await $(`aria/Accept ${ACCEPTED}`)).click();
    await $(`[data-testid="skill-${ACCEPTED}"]`).waitForExist({ timeout: 20_000 });
    expect(existsSync(join(root, "skills", ACCEPTED, "SKILL.md"))).toBe(true);
    await $(`[data-testid="draft-${ACCEPTED}"]`).waitForExist({ reverse: true, timeout: 20_000 });
    expect(existsSync(join(state, "skill-drafts", ACCEPTED_ID))).toBe(false);
  });

  it("opens a chat to draft a skill", async () => {
    // The chat Try it opened went, so the studio is brought to the front again.
    await runFromThePalette("Open Skill studio", "skills.studio");
    const ask = await $("button=Ask a chat to draft a skill");
    await ask.waitForClickable({ timeout: 20_000 });
    const tabsBefore = (await tabNames()).length;
    await ask.click();
    await browser.waitUntil(async () => (await tabNames()).length > tabsBefore, {
      timeout: 30_000,
      timeoutMsg: "Ask a chat to draft a skill opened no chat",
    });
    await endTheSessionsStartedSince(wereRunning);
  });
});
