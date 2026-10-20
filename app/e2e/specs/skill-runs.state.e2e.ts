// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { endEveryChat, pressAndStart } from "../opening.js";
import { USED_SKILL, USES_A_SKILL } from "../harness.js";
import { closeTheViewTabsOpenedSince, tabNames } from "../reach.js";

/**
 * **A chat uses a skill, and its tab shows the run** (SK-3, #2766), in the built app against the
 * real core, with the `skill-studio` flag on for the run (`wdio.conf.ts`).
 *
 * Nothing is faked past the harness: the reporting harness, a Codex, reads a skill it was
 * briefed on and runs Codex's hook for that shell command through the real binary
 * (`USES_A_SKILL`); the hook tells the app over the socket the app opened, and the app opens
 * the run on the chat's tab and logs its start in the project's skill log. The harness holds
 * its turn, so the run stays open. Pressing the chip opens the skill's own tab, whose Runs panel
 * lists that run. Ending the chat ends the run, and the chip goes.
 */

const CHIP = '[data-testid="tab-skill-run"]';

/** Where the run keeps what it drew for the pull request (CI uploads it). */
const SHOTS = join(process.cwd(), "logs", "screenshots");

describe("a skill run on a chat's tab", function () {
  this.timeout(120_000);

  let wereOpen: string[] = [];

  before(async () => {
    await endEveryChat();
    wereOpen = await tabNames();
    writeFileSync(USES_A_SKILL, "");
  });

  after(async () => {
    rmSync(USES_A_SKILL, { force: true });
    await endEveryChat();
    await closeTheViewTabsOpenedSince(wereOpen);
  });

  it("shows the skill in use, opens its tab on the run, and goes with the chat", async () => {
    await pressAndStart("New tab");
    // The harness holds its output until a line is typed; then it uses the skill.
    const pane = await $('[data-testid="pane"]');
    await pane.click();
    await $(".xterm-helper-textarea").addValue("\n");

    const chip = await $(CHIP);
    await chip.waitForExist({ timeout: 30_000, timeoutMsg: "the tab never showed the run" });
    await expect(chip).toHaveText(expect.stringContaining(USED_SKILL));
    await expect(chip).toHaveAttribute(
      "aria-label",
      expect.stringContaining(`Using the skill ${USED_SKILL}`),
    );

    mkdirSync(SHOTS, { recursive: true });
    await chip.saveScreenshot(join(SHOTS, "skill-run-chip.png"));
    await browser.saveScreenshot(join(SHOTS, "skill-run-chip-window.png"));

    await chip.click();
    const runs = await $('[data-testid="skill-runs"]');
    await runs.waitForExist({ timeout: 20_000, timeoutMsg: "the skill's tab did not open" });
    const shown = await runs.$('tr[aria-current="true"]');
    await shown.waitForExist({ timeout: 20_000, timeoutMsg: "the run was not in its Runs panel" });
    await expect(shown).toHaveText(expect.stringContaining("The chat read the skill"));
    expect(await tabNames()).toContain(USED_SKILL);
    await browser.saveScreenshot(join(SHOTS, "skill-runs-panel.png"));

    await endEveryChat();
    await browser.waitUntil(async () => !(await $(CHIP).isExisting()), {
      timeout: 20_000,
      timeoutMsg: "the run's chip stayed after its chat ended",
    });
  });
});
