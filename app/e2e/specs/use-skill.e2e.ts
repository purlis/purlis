// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, $$, browser, expect } from "@wdio/globals";
import { projectRoot } from "../acting.js";
import { chatsRunning, endTheChatsStartedSince, type Running } from "../chatsLeft.js";
import { focusWorkspace } from "../reach.js";
import { startAChat } from "../started.js";

/**
 * **Use skill…** (SK-12, #2775), in the built app against the real core, with the `skill-studio`
 * flag on for the run (`wdio.conf.ts`).
 *
 * The spec puts a project skill that declares two arguments in the project's `skills/`, granted
 * to every workspace, and starts a chat as a person does. From the palette it opens Use skill…,
 * which lists the chat's skills with its skill budget, picks the skill, fills its two boxes in
 * order and types it. The window then says the line it typed (or copied, where the chat is not
 * at its prompt yet), in the chat's harness's syntax: the scenario's profile is of Claude Code's
 * kind, so `/skills:<name>` with each argument a shell word. The Skills chip on the chat's pane
 * opens the same picker. What each harness is typed is proved in purlis-core (the adapters'
 * tests and `skills/tests.rs`); that the run starts with the prompt, in `eventlog/tests.rs`.
 *
 * **It leaves the window and the project as it found them.**
 */

const SKILL = "e2e-use-skill";

/** Opens the palette, finds the chat in front's Use skill… row and presses it. */
async function useSkillFromThePalette(): Promise<void> {
  await browser.keys(["F2"]);
  const box = await $("#palette-query");
  await box.waitForDisplayed({ timeout: 10_000 });
  await box.addValue("Use skill");
  await browser.waitUntil(
    async () =>
      browser.execute(() => {
        const row = document.querySelector<HTMLElement>('[id^="palette-row-tab.useskill:"]');
        row?.click();
        return row !== null;
      }),
    { timeout: 10_000, interval: 100, timeoutMsg: "the palette never offered Use skill…" },
  );
}

describe("Use skill…", function () {
  this.timeout(180_000);

  let folder = "";
  let grant = "";
  let wereRunning: Running = {};

  before(async () => {
    const root = await projectRoot();
    folder = join(root, "skills", SKILL);
    grant = join(root, "skills", `${SKILL}.toml`);
    mkdirSync(folder, { recursive: true });
    writeFileSync(
      join(folder, "SKILL.md"),
      `---\nname: ${SKILL}\ndescription: Probes Use skill.\narguments: pr focus\n---\n\nReview $pr for $focus.\n`,
    );
    writeFileSync(grant, 'grant = "all"\n');
    await focusWorkspace("alpha");
    wereRunning = await chatsRunning();
  });

  after(async () => {
    await browser.keys(["Escape"]);
    await endTheChatsStartedSince(wereRunning);
    rmSync(folder, { recursive: true, force: true });
    rmSync(grant, { force: true });
  });

  it("types a picked skill with its arguments in the chat's syntax, and sends nothing", async () => {
    await startAChat();
    await useSkillFromThePalette();

    const dialog = await $('[data-testid="use-skill"]');
    await dialog.waitForDisplayed({ timeout: 20_000 });
    const budget = await $('[data-testid="use-skill-budget"]');
    await budget.waitForExist({ timeout: 20_000 });
    await expect(budget).toHaveText(/skills? of 30 · .* of 6,000 characters/);

    await browser.waitUntil(
      async () => {
        for (const row of await $$('[data-testid="use-skill-row"]')) {
          if ((await row.getText()).includes(SKILL)) {
            await row.click();
            return true;
          }
        }
        return false;
      },
      { timeout: 20_000, timeoutMsg: `Use skill… never listed ${SKILL}` },
    );
    const boxes = await $$('[data-testid="use-skill"] input');
    expect(boxes).toHaveLength(2);
    await boxes[0].setValue("42");
    await boxes[1].setValue("the tests");
    // Pressed from the document, as the picker's other specs press a dialog's button: a
    // WebDriver click can land on the dialog's scrim edge on a small runner screen.
    const pressed = await browser.execute(() => {
      const button = [
        ...document.querySelectorAll<HTMLButtonElement>('[data-testid="use-skill"] button'),
      ].find((one) => one.textContent === "Type it");
      button?.click();
      return button !== undefined;
    });
    expect(pressed).toBe(true);
    // Closed once the core answered; a refusal stays in the dialog, and is what a red run says.
    await browser
      .waitUntil(async () => !(await dialog.isDisplayed().catch(() => false)), {
        timeout: 40_000,
        timeoutMsg: "",
      })
      .catch(async () => {
        const said = await browser.execute(() =>
          [
            ...document.querySelectorAll(
              '[data-testid="use-skill"], [role="status"], [role="alert"]',
            ),
          ]
            .map((one) => one.textContent ?? "")
            .join(" | "),
        );
        throw new Error(`Use skill… never closed; it says: ${said}`);
      });

    const said = await $('p[role="status"]');
    await said.waitForExist({ timeout: 20_000 });
    await expect(said).toHaveText(expect.stringContaining(`/skills:${SKILL} 42 "the tests"`));
  });

  it("opens the same picker from the Skills chip on the chat's pane", async () => {
    // Whatever the step before left open goes first, so this one opens its own.
    await browser.keys(["Escape"]);
    const chip = await $('[data-testid="pane-skills"]');
    await chip.waitForDisplayed({ timeout: 20_000 });
    await expect(chip).toHaveText(/skills?$/);
    await chip.click();
    const dialog = await $('[data-testid="use-skill"]');
    await dialog.waitForDisplayed({ timeout: 20_000 });
    await (await $("button=Cancel")).click();
    await dialog.waitForDisplayed({ timeout: 20_000, reverse: true });
  });
});
