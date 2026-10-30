// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
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
 * to every workspace, and starts a chat as a person does, on each harness in turn: the scenario's
 * own program, declared as Claude Code and as opencode. A Codex chat starts only in the state run
 * (#2037), which types a skill into one (`use-skill.state.e2e.ts`). From the palette it opens Use skill…,
 * which lists the chat's skills with its skill budget, picks the skill, fills its two boxes in
 * order and types it. The window then says the line it typed (or copied, where the chat is not
 * at its prompt yet), in that harness's syntax. The Skills chip on the chat's pane opens the
 * same picker. That a typed skill's run starts with the prompt it is sent in is proved in
 * purlis-core (`skillspan/tests.rs`, `eventlog/tests.rs`).
 *
 * **It leaves the window and the project as it found them.**
 */

const SKILL = "e2e-use-skill";

/** Opens the palette, finds the chat in front's Use skill… row and presses it. */
async function openThePickerFromThePalette(): Promise<void> {
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

/** Each harness the scenario's program is declared as here, and the line Use skill… types on
 *  it. */
const HARNESSES: readonly (readonly [string, string])[] = [
  ["claude", `/skills:${SKILL} 42 "the tests"`],
  ["opencode", `/${SKILL} 42 "the tests"`],
];

/** The project's machine-local settings, as the run declared them. */
let local = "";
let declared = "";

/** Makes the scenario's own program, declared as `kind`, the project's default profile, as
 *  `harness-card.e2e.ts` declares opencode. */
function onTheProfile(kind: string): void {
  const command = /command = (\[.*\])/.exec(declared)?.[1];
  if (!command) throw new Error(`no command in ${local}: ${declared}`);
  writeFileSync(
    local,
    `${declared.replace('default = "scenario"', `default = "use-skill-${kind}"`)}\n` +
      `[harness.use-skill-${kind}]\nkind = "${kind}"\ncommand = ${command}\n`,
  );
}

/** Picks the spec's skill in the chat in front's Use skill…, fills its two boxes and types it, and
 *  waits for the window to say `typed`, the line in the chat's harness's syntax. */
async function typeTheSkill(typed: string): Promise<void> {
  await openThePickerFromThePalette();

  const dialog = await $('[data-testid="use-skill"]');
  await dialog.waitForDisplayed({ timeout: 20_000 });
  // A quick picker (S8, #3694): its box takes the keys, and the budget is the Skills view's.
  await expect($('[data-testid="use-skill-budget"]')).not.toExist();

  await browser.waitUntil(
    async () => {
      for (const row of await $$('[data-testid="use-skill-row"]')) {
        if ((await row.getText()).includes(SKILL)) {
          // The row is the skill's option (#2911): a press picks it.
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
        [...document.querySelectorAll('[data-testid="use-skill"], [role="status"], [role="alert"]')]
          .map((one) => one.textContent ?? "")
          .join(" | "),
      );
      throw new Error(`Use skill… never closed; it says: ${said}`);
    });

  let lines: string[] = [];
  await browser.waitUntil(
    async () => {
      lines = await browser.execute(() =>
        [...document.querySelectorAll('.window-work > .action-result [role="status"]')].map(
          (one) => one.textContent ?? "",
        ),
      );
      return lines.some((line) => line.includes(typed));
    },
    { timeout: 20_000, timeoutMsg: `the window never said ${typed}: ${JSON.stringify(lines)}` },
  );
}

describe("Use skill…", function () {
  this.timeout(360_000);

  let folder = "";
  let grant = "";
  let wereRunning: Running = {};
  let size = { width: 0, height: 0 };

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
    local = join(root, "charter.local.toml");
    declared = readFileSync(local, "utf8");
    await focusWorkspace("alpha");
    wereRunning = await chatsRunning();
    size = await browser.getWindowSize();
  });

  after(async () => {
    await browser.keys(["Escape"]);
    await browser.setWindowSize(size.width, size.height);
    await endTheChatsStartedSince(wereRunning);
    if (declared !== "") writeFileSync(local, declared);
    rmSync(folder, { recursive: true, force: true });
    rmSync(grant, { force: true });
  });

  for (const [kind, typed] of HARNESSES)
    it(`types a picked skill with its arguments in ${kind}'s syntax, and sends nothing`, async () => {
      onTheProfile(kind);
      await browser.execute(() => window.dispatchEvent(new Event("focus")));
      await startAChat();
      await typeTheSkill(typed);
    });

  it("shows the chat's skills in the Skills view from the Skills chip on its pane (S8)", async () => {
    // Whatever the step before left open goes first, so this one opens its own.
    await browser.keys(["Escape"]);
    // The chip is left out of a narrow pane (`App.css`), so the window is wide enough for one.
    await browser.setWindowSize(1440, 900);
    const chip = await $('[data-testid="pane-skills"]');
    await chip.waitForDisplayed({ timeout: 20_000 });
    await expect(chip).toHaveText(/skills?$/);
    await chip.click();
    // The one answer to what the chat gets: the sidebar's Skills view, on this chat, with its
    // budget under whose set it is. Use skill… does not open.
    const view = await $('[data-testid="skills-view"]');
    await view.waitForDisplayed({ timeout: 20_000 });
    await expect($('[data-testid="skills-view-said"]')).toHaveText(
      /skills? of 30 · .* of 6,000 characters/,
    );
    await expect($('[data-testid="use-skill"]')).not.toBeDisplayed();
  });
});
