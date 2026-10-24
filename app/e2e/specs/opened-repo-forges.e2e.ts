// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import process from "node:process";
import { $, browser } from "@wdio/globals";
import { THE_RUNS_TREE } from "../harness.js";
import { closeProject, fromThePalette } from "../opening.js";
import { ask } from "../switching.js";

/**
 * **The window says what opening a repo did to the project's forges** (#1669, 0d1d3e1a): a repo
 * whose `origin` is on gitlab.com, opened from New project into this machine's local project
 * while that project tracks only GitHub, leaves *GitLab was added to this project's forges.* on
 * the window's line, and the block in the project's file.
 *
 * Two repos, because a local project the GitLab repo itself made would be made with GitLab's
 * block already in it, and then there is nothing to say (`ForgeTaken::Tracked`). So a GitHub repo
 * goes first: it makes the local project with GitHub's block, or finds it, as `first-task.e2e.ts`
 * left it, already tracking GitHub. Either way the GitLab repo is then the one that adds.
 *
 * Neither remote is reached: each repo is cloned from its folder, and `origin` is only what the
 * forge is read from.
 *
 * **It leaves the window as it found it**: the local project is closed by its own `×`, and the
 * machine store forgets it, as `first-task.e2e.ts` does.
 */

const PROJECTS = '[data-strip="Projects"]';
const NEW_PROJECT = '[role="dialog"][aria-labelledby="new-project"]';
const PICKER = '[role="dialog"][aria-labelledby="start-chat"]';
const ADDED = "GitLab was added to this project's forges.";
/** How long a window with no trust gate and no picker up is waited on for one to come. */
const QUIET = 5_000;

/** A repo with one commit, whose `origin` is `remote`, named `name`. */
function aRepo(name: string, remote: string): string {
  const at = join(mkdtempSync(join(THE_RUNS_TREE, "forges-")), name);
  mkdirSync(at, { recursive: true });
  const git = (args: string[]) =>
    execFileSync(
      "git",
      ["-c", "user.name=purlis scenario", "-c", "user.email=scenario@example.invalid", ...args],
      {
        cwd: at,
        stdio: "pipe",
        env: { ...process.env, GIT_CONFIG_GLOBAL: "/dev/null", GIT_CONFIG_SYSTEM: "/dev/null" },
      },
    );
  git(["init", "-q", "-b", "main", "."]);
  writeFileSync(join(at, "README.md"), `# ${name}\n`);
  git(["add", "-A"]);
  git(["commit", "-q", "-m", "start"]);
  git(["remote", "add", "origin", remote]);
  return at;
}

/** Every line the window says as a status or an alert, as text. */
async function windowLines(): Promise<string[]> {
  return browser.execute(() =>
    [...document.querySelectorAll('[role="status"], [role="alert"]')]
      .map((one) => (one.textContent ?? "").trim())
      .filter((said) => said !== ""),
  );
}

/**
 * Opens `repo` from New project into this machine's local project, through the trust gate where
 * it asks, and lets the first chat's picker go: the picker is not this spec's.
 */
async function openFromNewProject(repo: string): Promise<void> {
  await fromThePalette("New project…");
  const dialog = await $(NEW_PROJECT);
  await dialog.waitForExist({ timeout: 20_000 });
  const box = await dialog.$('input[placeholder="/where/the/repo/is"]');
  await box.waitForExist({ timeout: 20_000 });
  await box.addValue(repo);
  await dialog.$("button=Open repo").click();

  // Done once the dialog is gone; the trust gate is answered on the way when it asks, which it
  // does only the first time this machine opens the local project.
  let refused: string[] = [];
  await browser
    .waitUntil(
      async () => {
        const gate = await $("button=Open project");
        if (await gate.isExisting().catch(() => false)) await gate.click().catch(() => undefined);
        if (!(await $(NEW_PROJECT).isExisting())) return true;
        refused = await browser.execute(
          (selector: string) =>
            [...(document.querySelector(selector)?.querySelectorAll('[role="alert"]') ?? [])].map(
              (one) => (one.textContent ?? "").trim(),
            ),
          NEW_PROJECT,
        );
        return false;
      },
      { timeout: 90_000, interval: 500 },
    )
    .catch(() => {
      throw new Error(`New project never opened ${basename(repo)}: ${JSON.stringify(refused)}`);
    });

  // Then the trust gate, where this machine has not approved the local project yet, and the
  // first chat's picker, which is let go. A window that draws neither in time goes on: the
  // sentence is what this spec reads, not the picker. "In time" is QUIET with neither up, not
  // the whole 45 s: neither repo here draws a picker today, and waiting the bound out twice was
  // 90 s of this spec's 95 on CI (#1765's notices).
  const until = Date.now() + 45_000;
  let quietSince = Date.now();
  while (Date.now() < until) {
    const gate = await $("button=Open project");
    if (await gate.isExisting().catch(() => false)) {
      await gate.click().catch(() => undefined);
      quietSince = Date.now();
    }
    if (
      await $(PICKER)
        .isExisting()
        .catch(() => false)
    ) {
      await browser.keys(["Escape"]);
      await $(PICKER).waitForExist({ timeout: 20_000, reverse: true });
      return;
    }
    if (Date.now() - quietSince > QUIET) return;
    await browser.pause(500);
  }
}

describe("opening a repo says what it did to the project's forges", function () {
  this.timeout(300_000);

  let first = "";
  let local = "";

  before(async () => {
    await $(`${PROJECTS} button[aria-label="Open a project…"]`).waitForClickable({
      timeout: 60_000,
    });
    first = (await ask<string[]>("open_planes"))[0];
  });

  after(async () => {
    for (let left = 0; left < 3; left++) {
      const up = await $('[role="dialog"]')
        .isExisting()
        .catch(() => false);
      if (!up) break;
      await browser.keys(["Escape"]);
    }
    if (local !== "") {
      const closer = `${PROJECTS} [role="tab"][aria-label="${basename(local)}"]`;
      if (await $(closer).isExisting()) await closeProject(closer);
    }
    for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== first))
      await ask("close_plane", { plane });
    if (local !== "") {
      await ask("revoke_approval", { path: local }).catch(() => undefined);
      await ask("forget_project", { path: local }).catch(() => undefined);
    }
  });

  it("says GitLab was added when a GitLab repo joins a project that tracks GitHub", async () => {
    const before = await ask<string[]>("open_planes");
    await openFromNewProject(aRepo("forges-hub", "https://github.com/scenario/forges-hub.git"));
    await browser.waitUntil(
      async () => (await ask<string[]>("open_planes")).some((one) => !before.includes(one)),
      { timeout: 60_000, timeoutMsg: "the local project never opened" },
    );
    local = (await ask<string[]>("open_planes")).find((one) => !before.includes(one)) ?? "";

    await openFromNewProject(aRepo("forges-lab", "https://gitlab.com/scenario/forges-lab.git"));

    // Said on the window's line, once, in the core's words.
    let lines: string[] = [];
    await browser
      .waitUntil(
        async () => {
          lines = await windowLines();
          return lines.some((said) => said.includes(ADDED));
        },
        { timeout: 30_000, interval: 250 },
      )
      .catch(() => {
        throw new Error(`the window never said "${ADDED}": it said ${JSON.stringify(lines)}`);
      });

    // And it is so: the project's file holds a GitLab block now.
    const manifest = ["purlis.toml", "charter.toml"]
      .map((name) => join(local, name))
      .find((file) => existsSync(file));
    const text = manifest === undefined ? "" : readFileSync(manifest, "utf8");
    if (!/kind\s*=\s*"gitlab"/.test(text))
      throw new Error(`the local project's file has no GitLab block: ${text}`);
  });
});
