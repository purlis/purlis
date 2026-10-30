// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { appStateOf, projectRoot } from "../acting.js";
import {
  closeTheViewTabsOpenedSince,
  endTheSessionsStartedSince,
  focusWorkspace,
  runFromThePalette,
  shownButton,
  tabNames,
} from "../reach.js";
import { studioPage } from "../studio.js";
import { projectInFront, runningIn } from "../running.js";

/**
 * **Approve and start, in place** (#3942), in the built app against the real core: the Skill
 * studio's Try it starts its chat in the core, without the new-chat picker, and the core refuses a
 * profile nobody approved on this machine. The refusal's notice offers Approve and start…, whose
 * question shows the profile's command in the picker's sentence; its answer approves that line
 * and starts the chat that was asked for.
 *
 * The profile is one this spec declares, named for it alone, so nobody has approved it whichever
 * spec ran first: a fresh app's case. It runs the `scenario` profile's program. **It leaves the
 * window, the project and its Local file as it found them**; the approval it records names a
 * profile no other spec declares.
 */

const PROFILE = "e2e-studio-approval";
const NAME = "e2e-approve-in-place-probe";
/** A draft id, as the core names a draft's folder: sixteen hex characters. */
const ID = "e2e0000000003942";

/** Puts a draft of a new project skill where the app keeps a chat's draft. */
function drafted(state: string): void {
  const at = join(state, "skill-drafts", ID);
  mkdirSync(join(at, "files"), { recursive: true });
  writeFileSync(
    join(at, "files", "SKILL.md"),
    `---\nname: ${NAME}\ndescription: Probes Approve and start.\n---\n\nSay that the probe ran.\n`,
  );
  writeFileSync(
    join(at, "draft.json"),
    JSON.stringify({
      level: { at: "Project", local: false },
      folder: NAME,
      chat: 1,
      bases: { "SKILL.md": null },
    }),
  );
}

/** `local`, with a profile `PROFILE` that runs what its `scenario` profile runs. */
function withTheProfile(local: string): string {
  const scenario = /\[harness\.scenario\]\n((?:[^[\n][^\n]*\n|\n)*)/.exec(local);
  if (scenario === null) throw new Error("the project's Local file declares no scenario profile");
  return `${local.endsWith("\n") ? local : `${local}\n`}\n[harness.${PROFILE}]\n${scenario[1].trimEnd()}\n`;
}

describe("a Skill studio start refused for its profile's approval", function () {
  this.timeout(180_000);

  let root = "";
  let state = "";
  let plane = "";
  /** The Local file as it was, or `null` where there was none. */
  let local: string | null = null;
  let wereOpen: string[] = [];
  let wereRunning: number[] = [];

  before(async () => {
    root = await projectRoot();
    state = appStateOf(root);
    await focusWorkspace("alpha");
    wereOpen = await tabNames();
    plane = await projectInFront();
    wereRunning = await runningIn(plane);
    const file = join(root, "charter.local.toml");
    local = existsSync(file) ? readFileSync(file, "utf8") : null;
    writeFileSync(file, withTheProfile(local ?? ""));
    drafted(state);
  });

  after(async () => {
    await endTheSessionsStartedSince(wereRunning);
    await focusWorkspace("alpha");
    await closeTheViewTabsOpenedSince(wereOpen);
    rmSync(join(state, "skill-drafts", ID), { recursive: true, force: true });
    if (root === "") return;
    const file = join(root, "charter.local.toml");
    if (local === null) rmSync(file, { force: true });
    else writeFileSync(file, local);
  });

  it("offers Approve and start, showing what it runs, and starts the chat asked for", async () => {
    await runFromThePalette("Open Skill studio", "skills.studio");
    await studioPage("Drafts");
    const draft = await $(`[data-testid="draft-${NAME}"]`);
    await draft.waitForExist({ timeout: 20_000 });
    await draft.click();

    // Try it on the profile nobody approved.
    const page = await $('[data-testid="skill-studio-draft"]');
    await page.$(".skill-draft-try button").click();
    const option = await $(`//*[@role="option"][contains(., "${PROFILE}")]`);
    await option.waitForClickable({ timeout: 20_000 });
    await option.click();
    await page.$("button=Try it").click();

    // Refused, with the approval offered beside the refusal; nothing started. The refusal pops
    // from the status bar's Notices item (#3653) and is listed in the Notices centre too, where
    // its copy may stand unseen: the one on screen.
    const offer = await shownButton("Approve and start…", 30_000);
    await offer.waitForClickable({
      timeout: 30_000,
      timeoutMsg: "the refusal offered no Approve and start…",
    });
    expect(await runningIn(plane)).toEqual(wereRunning);
    await offer.click();

    // The question shows the command it approves, as the picker does.
    const ask = await $('[role="alertdialog"]');
    await ask.waitForDisplayed({ timeout: 20_000 });
    await expect(ask).toHaveText(new RegExp(`Approve ${PROFILE}\\?`));
    await expect(ask.$("p.approve code")).toHaveText(/claude-stand-in/);
    await ask.$("button=Approve and start").click();

    await browser.waitUntil(
      async () => (await runningIn(plane)).some((one) => !wereRunning.includes(one)),
      { timeout: 30_000, timeoutMsg: "Approve and start started no chat" },
    );
  });
});
