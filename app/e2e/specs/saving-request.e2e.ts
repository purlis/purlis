// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { mkdirSync, realpathSync, renameSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { copyFixturePlane } from "../harness.js";
import { closeProject, openThroughTheTrustQuestion } from "../opening.js";

/**
 * **A save's red pull request, in the built app** (ADR 0051, amended 2026-10-13 and 2026-10-14;
 * the operator's rulings P1 and P2).
 *
 * A copy of the `daily` fixture made into a git repository with one commit, whose saves are
 * `pr-merge` on this machine, and which keeps a pull request the last look found red: the
 * state a look leaves in the clone's own `save-branch.json`. The Saving view then draws the red
 * Notice with its two answers.
 *
 * The project has no origin, so nothing here reaches a forge: each answer goes to the core and
 * comes back as the core's own refusal, which is what is read. What is proved here and not in
 * jsdom: the standing is read from the clone's kept state by the real `plane_saving`, the
 * confirmation is drawn over the window, and `merge_save_request` and `fix_save_request` are
 * commands the window reaches and the core answers in words.
 *
 * **It leaves the window as it found it**: the project it opens it lets go of.
 */

const PROJECTS = '[data-strip="Projects"]';
const BAR = '[data-testid="title-bar"]';
const RED = '[data-cause="save-request-red:this project"]';
const PAGE = "https://github.com/acme/plane/pull/12";

/** git in the fixture: never the machine's own config, so never its signer. Answers what it
 *  printed, trimmed. */
function git(dir: string, args: string[]): string {
  return execFileSync("git", ["-c", "commit.gpgsign=false", ...args], {
    cwd: dir,
    env: {
      ...process.env,
      GIT_CONFIG_GLOBAL: "/dev/null",
      GIT_CONFIG_NOSYSTEM: "1",
      GIT_AUTHOR_NAME: "fixture",
      GIT_AUTHOR_EMAIL: "fixture@example.invalid",
      GIT_COMMITTER_NAME: "fixture",
      GIT_COMMITTER_EMAIL: "fixture@example.invalid",
    },
    stdio: ["ignore", "pipe", "ignore"],
    encoding: "utf8",
  }).trim();
}

/** The words of the red Notice, or `null` while it is not drawn. */
async function redSays(): Promise<string | null> {
  return browser.execute((red: string) => document.querySelector(red)?.textContent ?? null, RED);
}

/** Waits until the red Notice says `want` somewhere in it, and says what it said if it never
 *  does. */
async function untilRedSays(want: string, what: string): Promise<void> {
  let last: string | null = null;
  try {
    await browser.waitUntil(
      async () => {
        last = await redSays();
        return last !== null && last.includes(want);
      },
      { timeout: 30_000, interval: 250 },
    );
  } catch {
    throw new Error(`${what}: the Notice said ${JSON.stringify(last)}`);
  }
}

describe("a save's red pull request in the Saving view", function () {
  this.timeout(120_000);

  const plane = (() => {
    const copied = copyFixturePlane();
    const renamed = join(dirname(copied), "red-request-plane");
    renameSync(copied, renamed);
    mkdirSync(join(renamed, ".charter"), { recursive: true });
    git(renamed, ["init", "-q", "-b", "main"]);
    git(renamed, ["config", "user.name", "fixture"]);
    git(renamed, ["config", "user.email", "fixture@example.invalid"]);
    git(renamed, ["add", "-A"]);
    git(renamed, ["commit", "-q", "-m", "the fixture"]);
    // This machine's own settings, which git ignores: the tree stays clean.
    writeFileSync(join(renamed, "charter.local.toml"), '[plane]\nmode = "pr-merge"\n');
    // What a look that found a failed check keeps, at the commit the project is on.
    writeFileSync(
      join(renamed, ".charter", "save-branch.json"),
      JSON.stringify({
        branch: "purlis/save/scenario",
        pushed: git(renamed, ["rev-parse", "HEAD"]),
        pr: {
          number: 12,
          url: PAGE,
          head: git(renamed, ["rev-parse", "HEAD"]),
          target: "main",
          merges_itself: true,
          pushed_at: Math.floor(Date.now() / 1000),
          merge: {
            state: "red",
            why: "1 check failed: lint",
            checks: [{ name: "lint", url: "https://github.com/acme/plane/actions/runs/9" }],
          },
        },
      }),
    );
    return realpathSync(renamed);
  })();

  after(async () => {
    const selector = `${PROJECTS} [role="tab"][aria-label="red-request-plane"]`;
    const closer = await $(selector);
    if (!(await closer.isExisting())) return;
    await closeProject(selector);
    await browser.waitUntil(async () => !(await closer.isExisting()), {
      timeout: 20_000,
      timeoutMsg: "the red-request-plane project's tab stayed on the strip",
    });
  });

  it("draws the red Notice with Fix in a chat and Merge as it stands", async () => {
    await $(`${PROJECTS} button[aria-label="Open a project…"]`).click();
    const box = await $('[data-setting="open-by-path"] input');
    await box.waitForDisplayed({ timeout: 20_000 });
    await box.addValue(plane);
    await $('form.by-path button[type="submit"]').click();
    await openThroughTheTrustQuestion();

    // The bar's stage button opens the Saving view.
    const stage = await $(`${BAR} button[aria-label^="Saving:"]`);
    await stage.waitForDisplayed({ timeout: 30_000 });
    await stage.click();

    await untilRedSays(
      "The pull request #12 of this project is red, so purlis did not merge it: 1 check failed: lint.",
      "the Saving view never drew the red request",
    );
    const answers = await browser.execute(
      (red: string) =>
        [...(document.querySelector(red)?.querySelectorAll("button") ?? [])].map(
          (button) => button.textContent,
        ),
      RED,
    );
    expect(answers).toEqual(["Fix in a chat", "Merge as it stands…", "Open the pull request"]);
    const failed = await $(`${RED} [aria-label="Checks that failed"]`);
    expect(await failed.getText()).toBe("lint");
  });

  it("asks before it merges, and Cancel merges nothing", async () => {
    await $(RED).$("button=Merge as it stands…").click();
    const asking = await $('[role="alertdialog"]');
    await asking.waitForDisplayed({ timeout: 20_000 });
    const said = await asking.getText();
    expect(said).toContain("Merge the pull request #12 of this project as it stands?");
    expect(said).toContain("purlis merges it into main now, without waiting for its checks.");
    expect(said).toContain("lint");

    await asking.$("button=Cancel").click();
    await asking.waitForExist({ reverse: true, timeout: 20_000 });
    // Still red, and nothing was said of a merge.
    expect(await redSays()).not.toContain("did not merge the pull request #12:");
  });

  it("says the core's refusal when the merge cannot be asked of a forge", async () => {
    await $(RED).$("button=Merge as it stands…").click();
    const asking = await $('[role="alertdialog"]');
    await asking.waitForDisplayed({ timeout: 20_000 });
    await asking.$("button=Merge as it stands").click();

    // No origin: the core refuses before any forge is asked, and the Notice says so in its
    // words, beside the request's link.
    await untilRedSays(
      "purlis did not merge the pull request #12:",
      "the Notice never said what the merge came to",
    );
    expect(await $(RED).$("button=Open the pull request").isExisting()).toBe(true);
  });

  it("says why no Fix chat started when what failed cannot be read", async () => {
    await $(RED).$("button=Fix in a chat").click();
    await untilRedSays(
      "purlis could not start a chat to fix it:",
      "the Notice never said what the Fix came to",
    );
  });
});
