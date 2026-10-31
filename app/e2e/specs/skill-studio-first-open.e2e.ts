// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, rmSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
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
import { theDefaultProfileApproved } from "../started.js";

/**
 * **The Skill studio's first open and New skill…** (SK-22, #2785; decisions 42 and 13; #3684,
 * S4), in the built app against the real core, with the `skill-studio` flag on for the run
 * (`wdio.conf.ts`).
 *
 * The fixture project has no skill of its own. New skill… asks how, and each way works: *Install
 * one from a source* moves to the skill sources, *Write it yourself* makes one from the blank
 * template and opens its page with its text in the editor, and *Ask a chat to draft it* opens a
 * chat. The offered sources are listed, and one click fills in Add source and asks before
 * anything is added; the spec cancels there, so nothing is fetched. What is offered when, the
 * move of skills still in the old folder included, is the core's seam test
 * (`skills/first_open/tests.rs`) and the studio's unit test: a clone copies that folder while it
 * is there, so a spec that made it would reach the next spec's clones.
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

/** New skill…, pressed in the studio's top bar: its dialog, asking how. */
async function newSkill() {
  const studio = await $('[data-testid="skill-studio"]');
  await studio.waitForExist({ timeout: 20_000 });
  const button = await studio.$("button=New skill…");
  await button.waitForEnabled({ timeout: 20_000 });
  await button.click();
  const asking = await $('[data-testid="skill-new"]');
  await asking.waitForDisplayed({ timeout: 10_000 });
  return asking;
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
    // The studio's chats start without the picker, on a profile someone approved (#3562): in
    // the project and workspace they start in.
    await theDefaultProfileApproved();
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

  it("asks how in New skill…, and From a source and Write it yourself work", async () => {
    await runFromThePalette("Open Skill studio", "skills.studio");
    const asking = await newSkill();
    expect(await asking.$$('ul[aria-label="Ways to make a skill"] > li').length).toBe(3);

    // From a source goes to the studio's Sources page; Back returns to the Library (#3681).
    await (await asking.$("button=Install one from a source")).click();
    await untilStudioPage("Sources");
    await (await $('[data-scope="view-frame"] button[aria-label="Back"]')).click();

    await (await (await newSkill()).$("button=Write it yourself")).click();
    await typeInto("Name", BLANK);
    await typeInto("What it does, and when a chat should use it", "Probes starting a skill blank.");
    await (await $("button=Make it and open its page")).click();
    await $(`[data-testid="skill-${BLANK}"]`).waitForExist({ timeout: 20_000 });
    expect(existsSync(join(root, "skills", BLANK, "SKILL.md"))).toBe(true);
    // Its page opens with its text in the editor (#3683).
    await $('[data-testid="skill-editor"]').waitForExist({ timeout: 20_000 });
    rmSync(join(root, "skills", BLANK), { recursive: true, force: true });
  });

  it("opens a chat from New skill…'s Ask a chat to draft it", async () => {
    // A studio tab opened afresh reads the project again.
    await focusWorkspace("alpha");
    await closeTheViewTabsOpenedSince(wereOpen);
    await runFromThePalette("Open Skill studio", "skills.studio");
    await (await (await newSkill()).$("button=Ask a chat to draft it")).click();
    await typeInto("What should it do?", "Probes drafting a skill.");
    await (await $("button=Open the chat")).click();
    await browser.waitUntil(
      async () => (await runningIn(plane)).some((one) => !wereRunning.includes(one)),
      { timeout: 30_000, timeoutMsg: "Ask a chat to draft it started no chat" },
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
