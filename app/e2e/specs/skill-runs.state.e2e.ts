// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { rmSync, writeFileSync } from "node:fs";
import { $, browser, expect } from "@wdio/globals";
import { endEveryChat, pressAndStart } from "../opening.js";
import { USED_SKILL, USES_A_SKILL } from "../harness.js";
import { closesTheTabsItOpens, tabNames } from "../reach.js";

/**
 * **A chat uses a skill, and its pane head shows the run** (SK-3, #2766; off its tab since #5524), in the built app against the
 * real core.
 *
 * Nothing is faked past the harness: the reporting harness, a Codex, reads a skill it was
 * briefed on and runs Codex's hook for that shell command through the real binary
 * (`USES_A_SKILL`); the hook tells the app over the socket the app opened, and the app opens
 * the run on the chat's pane head and logs its start in the project's skill log. The harness holds
 * its turn, so the run stays open. Pressing the chip opens the Skill studio at the skill's page,
 * on its Runs (#3682), which lists that run marked, and its totals count it by what it ran on, persona and workspace (SK-20).
 * Ending the chat ends the run, and the chip goes.
 */

const CHIP = '[data-testid="pane-skill-run"]';

describe("a skill run on a chat's pane head", function () {
  this.timeout(120_000);

  // Hooks of a kind run in the order they are registered: the strip is read once every chat is
  // ended, before the skill is put in use, and its view tabs are closed once the chats are ended
  // again.
  before(async () => {
    await endEveryChat();
  });

  after(async () => {
    rmSync(USES_A_SKILL, { force: true });
    await endEveryChat();
  });

  closesTheTabsItOpens();

  before(() => {
    writeFileSync(USES_A_SKILL, "");
  });

  it("shows the skill in use, opens its Runs in the studio on the run, and goes with the chat", async () => {
    await pressAndStart("New tab");
    // The harness holds its output until a line is typed; then it uses the skill.
    const pane = await $('[data-testid="pane"]');
    await pane.click();
    await $(".xterm-helper-textarea").addValue("\n");

    const chip = await $(CHIP);
    await chip.waitForExist({ timeout: 30_000, timeoutMsg: "the pane head never showed the run" });
    await expect(chip).toHaveText(expect.stringContaining(USED_SKILL));
    // Named by what it shows, the rest of its sentence heard and not drawn (#5614).
    await expect(chip).toHaveComputedLabel(
      expect.stringContaining(`Using the skill ${USED_SKILL}`),
    );

    await chip.click();
    const runs = await $('[data-testid="skill-runs"]');
    await runs.waitForExist({ timeout: 20_000, timeoutMsg: "the skill's Runs did not open" });
    const shown = await runs.$('tr[aria-current="true"]');
    await shown.waitForExist({ timeout: 20_000, timeoutMsg: "the run was not in its Runs panel" });
    await expect(shown).toHaveText(expect.stringContaining("The chat read the skill"));
    // The studio's one tab, at the skill's page: the skill's own tab is retired (#3682).
    expect(await tabNames()).toContain("Skill studio");
    await expect($('[data-testid="skill-studio-skill"] h2')).toHaveText(
      expect.stringContaining(USED_SKILL),
    );
    // Its totals count the run, still open, by what it ran on, persona and workspace (SK-20).
    for (const by of ["by_harness", "by_persona", "by_workspace"]) {
      const totals = await $(`[data-testid="skill-totals-${by}"]`);
      await totals.waitForExist({ timeout: 20_000, timeoutMsg: `no totals ${by}` });
      await expect(totals).toHaveText(expect.stringContaining("1 run, 0 ended"));
    }

    await endEveryChat();
    await browser.waitUntil(async () => !(await $(CHIP).isExisting()), {
      timeout: 20_000,
      timeoutMsg: "the run's chip stayed after its chat ended",
    });
  });
});
