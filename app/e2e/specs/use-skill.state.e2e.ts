// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, $$, browser, expect } from "@wdio/globals";
import { endEveryChat, pressAndStart } from "../opening.js";
import { USED_SKILL } from "../harness.js";

/**
 * **Use skill… on a Codex chat** (SK-12, #2775), in the built app against the real core, with the
 * `skill-studio` flag on for the run (`wdio.state.conf.ts`).
 *
 * The base run cannot start a Codex chat on macOS and Linux (a Codex chat runs only on the
 * sandbox-only profile, #2037); this run's profile is one, and its reporting harness says it waits
 * at its prompt as a Codex does. Use skill…, from the palette, lists the chat's skills with its
 * budget; picking a built-in skill that takes nothing types `$<name>`, Codex's mention, and the
 * window says so. Claude Code and opencode are the base run's (`use-skill.e2e.ts`).
 */
describe("Use skill… on a Codex chat", function () {
  this.timeout(120_000);

  before(async () => {
    await endEveryChat();
  });

  after(async () => {
    await browser.keys(["Escape"]);
    await endEveryChat();
  });

  it("types a picked skill as Codex's mention, and sends nothing", async () => {
    await pressAndStart("New tab");
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
      { timeout: 20_000, interval: 100, timeoutMsg: "the palette never offered Use skill…" },
    );

    const dialog = await $('[data-testid="use-skill"]');
    await dialog.waitForDisplayed({ timeout: 20_000 });
    // Every skill a Codex chat is briefed on is listed with its path; its count is the Skills
    // view's now (S8, #3694), and the picker says none.
    await expect($('[data-testid="use-skill-budget"]')).not.toExist();
    await browser.waitUntil(
      async () => {
        for (const row of await $$('[data-testid="use-skill-row"]')) {
          // The row is the skill's option (#2911), named by the skill: a press picks it.
          const name = await row.$('[data-part="label"]');
          if ((await name.getText()) === USED_SKILL) {
            await row.click();
            return true;
          }
        }
        return false;
      },
      { timeout: 20_000, timeoutMsg: `Use skill… never listed ${USED_SKILL}` },
    );
    await dialog.waitForDisplayed({ timeout: 20_000, reverse: true });

    const typed = `$${USED_SKILL}`;
    await browser.waitUntil(
      async () => {
        // Said on the status bar's card (#3653), or on the opener's page with no project.
        const lines = await browser.execute(() =>
          [
            ...document.querySelectorAll(
              '[data-chip="status/bar/notices"] > .action-result [role="status"], .window-work > .action-result [role="status"]',
            ),
          ].map((one) => one.textContent ?? ""),
        );
        return lines.some((line) => line.includes(typed));
      },
      { timeout: 20_000, timeoutMsg: `the window never said ${typed}` },
    );
  });
});
