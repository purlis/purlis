// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, rmSync } from "node:fs";
import { join } from "node:path";
import { $, $$, browser, expect } from "@wdio/globals";
import { projectRoot } from "../acting.js";
import {
  closeTheViewTabsOpenedSince,
  endTheSessionsStartedSince,
  focusWorkspace,
  runFromThePalette,
  tabNames,
} from "../reach.js";
import { studioPage, untilStudioPage } from "../studio.js";
import { projectInFront, runningIn } from "../running.js";

/**
 * **The Skill studio's first open and its empty state** (SK-22, #2785; decision 42), in the built
 * app against the real core, with the `skill-studio` flag on for the run (`wdio.conf.ts`).
 *
 * The fixture project has no skill of its own, so the studio offers its three starts above the
 * skills its chats already get, and each one works: *Ask a chat to draft a skill* opens a chat,
 * *Browse sources* moves to the skill sources, and *New skill* makes one from the blank template.
 * The offered sources are listed, and one click fills in Add source and asks before anything is
 * added; the spec cancels there, so nothing is fetched. What is offered when, the move of skills
 * still in the old folder included, is the core's seam test (`skills/first_open/tests.rs`) and the
 * studio's unit test: a clone copies that folder while it is there, so a spec that made it would
 * reach the next spec's clones.
 *
 * **It leaves the window and the project as it found them.**
 */

const BLANK = "e2e-started-blank";

/** Types `text` into the box labelled `label`. */
async function typeInto(label: string, text: string) {
  const box = await $(`aria/${label}`);
  await box.waitForDisplayed({ timeout: 10_000 });
  await box.clearValue();
  await box.addValue(text);
}

/** The three starts, once the studio has drawn them. */
async function starts() {
  const empty = await $('[data-testid="skill-studio-empty"]');
  await empty.waitForExist({ timeout: 20_000 });
  return empty;
}

describe("the Skill studio's first open", function () {
  this.timeout(180_000);

  let root = "";
  let plane = "";
  let wereOpen: string[] = [];
  let wereRunning: number[] = [];

  before(async () => {
    root = await projectRoot();
    await focusWorkspace("alpha");
    wereOpen = await tabNames();
    plane = await projectInFront();
    wereRunning = await runningIn(plane);
  });

  after(async () => {
    await endTheSessionsStartedSince(wereRunning);
    await focusWorkspace("alpha");
    await closeTheViewTabsOpenedSince(wereOpen);
    rmSync(join(root, "skills", BLANK), { recursive: true, force: true });
  });

  it("offers three starts in an empty studio, and Browse sources and New skill work", async () => {
    await runFromThePalette("Open Skill studio", "skills.studio");
    const empty = await starts();
    await expect(empty).toHaveText(/This project has no skills of its own yet/);
    expect(await $$('ul[aria-label="Ways to start a skill"] > li').length).toBe(3);

    // Browse sources goes to the studio's Sources page; Back returns to the Library (#3681).
    await (await empty.$("button=Browse sources")).click();
    await untilStudioPage("Sources");
    await (await $('[data-scope="view-frame"] button[aria-label="Back"]')).click();
    const again = await starts();

    await (await again.$("button=New skill")).click();
    await typeInto("Name", BLANK);
    await typeInto("What it does and when to use it", "Probes starting a skill blank.");
    await (await $("button=Make it")).click();
    await $(`[data-testid="skill-${BLANK}"]`).waitForExist({ timeout: 20_000 });
    expect(existsSync(join(root, "skills", BLANK, "SKILL.md"))).toBe(true);
    // The project has a skill of its own now: the starts are gone.
    await $('[data-testid="skill-studio-empty"]').waitForExist({ reverse: true, timeout: 20_000 });
    rmSync(join(root, "skills", BLANK), { recursive: true, force: true });
  });

  it("opens a chat from the empty studio's Ask a chat to draft a skill", async () => {
    // A studio tab opened afresh reads the project again.
    await focusWorkspace("alpha");
    await closeTheViewTabsOpenedSince(wereOpen);
    await runFromThePalette("Open Skill studio", "skills.studio");
    const empty = await starts();
    await (await empty.$("button=Ask a chat to draft a skill")).click();
    await browser.waitUntil(
      async () => (await runningIn(plane)).some((one) => !wereRunning.includes(one)),
      { timeout: 30_000, timeoutMsg: "Ask a chat to draft a skill started no chat" },
    );
    await endTheSessionsStartedSince(wereRunning);
  });

  it("fills in Add source from an offered source with one click, and asks first", async () => {
    // A studio tab opened afresh reads the project again.
    await focusWorkspace("alpha");
    await closeTheViewTabsOpenedSince(wereOpen);
    await runFromThePalette("Open Skill studio", "skills.studio");
    await studioPage("Sources");
    const offered = await $('[data-testid="skill-source-offered"]');
    await offered.waitForExist({ timeout: 20_000 });
    await (await $("aria/Add Anthropic's skills")).click();
    await expect($('[data-testid="skill-source-offered-asks"]')).toHaveText(
      /Nothing is installed until you review a skill from it\./,
    );
    await expect($("aria/Repository")).toHaveValue("https://github.com/anthropics/skills");
    await (await $('[data-testid="skill-source-add"]').$("button=Cancel")).click();
    await expect($('[data-testid="skill-source-offered"]')).toExist();
  });
});
