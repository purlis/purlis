// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, mkdirSync, rmdirSync, rmSync, writeFileSync } from "node:fs";
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
import { projectInFront, runningIn } from "../running.js";

/**
 * **The Skill studio's first open and its empty state** (SK-22, #2785; decision 42), in the built
 * app against the real core, with the `skill-studio` flag on for the run (`wdio.conf.ts`).
 *
 * The fixture project has no skill of its own, so the studio offers its three starts above the
 * skills its chats already get, and each one works: *Make one with a chat* opens a chat, *Browse
 * sources* moves to the skill sources, and *Start blank* makes a skill from the blank template.
 * The offered sources are listed, and one click fills in Add source and asks before anything is
 * added; the spec cancels there, so nothing is fetched. A project whose skills are still in the
 * old folder is offered the move. What is offered when is the core's seam test
 * (`skills/first_open/tests.rs`).
 *
 * **It leaves the window and the project as it found them.**
 */

const BLANK = "e2e-started-blank";
const OLD = "e2e-old-folder-probe";

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
  /** Which of the old folder and its parent the spec made, so it takes away exactly those: a
   *  clone copies the project's `.claude/skills` while it is there. */
  let made: string[] = [];

  before(async () => {
    root = await projectRoot();
    made = [join(root, ".claude"), join(root, ".claude", "skills")].filter(
      (dir) => !existsSync(dir),
    );
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
    rmSync(join(root, ".claude", "skills", OLD), { recursive: true, force: true });
    for (const dir of [...made].reverse()) if (existsSync(dir)) rmdirSync(dir);
  });

  it("offers three starts in an empty studio, and Browse sources and Start blank work", async () => {
    await runFromThePalette("Open Skill studio", "skills.studio");
    const empty = await starts();
    await expect(empty).toHaveText(/This project has no skills of its own yet/);
    expect(await $$('ul[aria-label="Ways to start a skill"] > li').length).toBe(3);

    await (await empty.$("button=Browse sources")).click();
    await browser.waitUntil(
      async () =>
        (await browser.execute(() => document.activeElement?.textContent?.trim() ?? "")) ===
        "Skill sources",
      { timeout: 10_000, timeoutMsg: "Browse sources did not move to the skill sources" },
    );

    await (await empty.$("button=Start blank")).click();
    await typeInto("Name", BLANK);
    await typeInto("What it does and when to use it", "Probes starting a skill blank.");
    await (await $("button=Make it")).click();
    await $(`[data-testid="skill-${BLANK}"]`).waitForExist({ timeout: 20_000 });
    expect(existsSync(join(root, "skills", BLANK, "SKILL.md"))).toBe(true);
    // The project has a skill of its own now: the starts are gone.
    await $('[data-testid="skill-studio-empty"]').waitForExist({ reverse: true, timeout: 20_000 });
    rmSync(join(root, "skills", BLANK), { recursive: true, force: true });
  });

  it("opens a chat from Make one with a chat", async () => {
    // A studio tab opened afresh reads the project again.
    await focusWorkspace("alpha");
    await closeTheViewTabsOpenedSince(wereOpen);
    await runFromThePalette("Open Skill studio", "skills.studio");
    const empty = await starts();
    await (await empty.$("button=Make one with a chat")).click();
    await browser.waitUntil(
      async () => (await runningIn(plane)).some((one) => !wereRunning.includes(one)),
      { timeout: 30_000, timeoutMsg: "Make one with a chat started no chat" },
    );
    await endTheSessionsStartedSince(wereRunning);
  });

  it("fills in Add source from an offered source with one click, and asks first", async () => {
    // A studio tab opened afresh reads the project again.
    await focusWorkspace("alpha");
    await closeTheViewTabsOpenedSince(wereOpen);
    await runFromThePalette("Open Skill studio", "skills.studio");
    const offered = await $('[data-testid="skill-source-offered"]');
    await offered.waitForExist({ timeout: 20_000 });
    await (await $("aria/Add Anthropic's skills")).click();
    await expect($('[data-testid="skill-source-offered-asks"]')).toHaveText(
      /Nothing is installed until you review a skill from it\./,
    );
    await expect($("aria/Repository")).toHaveValue("https://github.com/anthropics/skills");
    await (await $('form[aria-label="Add Anthropic\'s skills"]').$("button=Cancel")).click();
    await expect($('[data-testid="skill-source-offered"]')).toExist();
  });

  it("offers to move skills that are still in the old folder", async () => {
    const folder = join(root, ".claude", "skills", OLD);
    mkdirSync(folder, { recursive: true });
    writeFileSync(
      join(folder, "SKILL.md"),
      `---\nname: ${OLD}\ndescription: Probes the move.\n---\n\nSay hello.\n`,
    );
    // A studio tab opened afresh reads the project again.
    await focusWorkspace("alpha");
    await closeTheViewTabsOpenedSince(wereOpen);
    await runFromThePalette("Open Skill studio", "skills.studio");
    const notice = await $('[data-testid="skill-studio-unmoved"]');
    await notice.waitForExist({ timeout: 20_000 });
    await expect(notice).toHaveText(/This project keeps 1 skill where only Claude Code reads/);
    await expect($("button=Move them")).toExist();
  });
});
