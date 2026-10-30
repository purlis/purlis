// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
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
import { projectInFront, runningIn } from "../running.js";

/**
 * **Approve and start, in place, for a curation action** (#3968), in the built app against the
 * real core: a curation action starts its chat in the core on the project's default profile,
 * without the new-chat picker, and the core refuses a profile nobody approved on this machine.
 * The refusal's notice offers Approve and start…, whose question shows the profile's command in
 * the picker's sentence; its answer approves that line and starts the action's chat.
 *
 * The default is a profile this spec declares, named for it alone, so nobody has approved it
 * whichever spec ran first. It runs the `scenario` profile's program. **It leaves the window, the
 * project and its Local file as it found them**; the approval it records names a profile no
 * other spec declares.
 */

const PROFILE = "e2e-curation-approval";
const ACTION = "curate:workspace:alpha/charter/safe-remove";

/** `local`, with a profile `PROFILE` that runs what its `scenario` profile runs, as the default. */
function withTheProfileAsDefault(local: string): string {
  const scenario = /\[harness\.scenario\]\n((?:[^[\n][^\n]*\n|\n)*)/.exec(local);
  if (scenario === null) throw new Error("the project's Local file declares no scenario profile");
  if (!/^default = "scenario"$/m.test(local))
    throw new Error("the project's Local file does not make scenario the default");
  const made = local.replace(/^default = "scenario"$/m, `default = "${PROFILE}"`);
  return `${made.endsWith("\n") ? made : `${made}\n`}\n[harness.${PROFILE}]\n${scenario[1].trimEnd()}\n`;
}

describe("a curation action refused for its profile's approval", function () {
  this.timeout(180_000);

  let root = "";
  let plane = "";
  /** The Local file as it was, or `null` where there was none. */
  let local: string | null = null;
  let wereOpen: string[] = [];
  let wereRunning: number[] = [];

  before(async () => {
    root = await projectRoot();
    await focusWorkspace("alpha");
    wereOpen = await tabNames();
    plane = await projectInFront();
    wereRunning = await runningIn(plane);
    const file = join(root, "charter.local.toml");
    local = existsSync(file) ? readFileSync(file, "utf8") : null;
    writeFileSync(file, withTheProfileAsDefault(local ?? ""));
  });

  after(async () => {
    await endTheSessionsStartedSince(wereRunning);
    await focusWorkspace("alpha");
    await closeTheViewTabsOpenedSince(wereOpen);
    if (root === "") return;
    const file = join(root, "charter.local.toml");
    if (local === null) rmSync(file, { force: true });
    else writeFileSync(file, local);
  });

  it("offers Approve and start, showing what it runs, and starts the action's chat", async () => {
    await runFromThePalette("Safe remove", ACTION);

    // Refused, with the approval offered beside the refusal; nothing started.
    const offer = await $("button=Approve and start…");
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
    await expect(ask).toHaveText(/The curation action Safe remove starts a chat/);
    await expect(ask.$("p.approve code")).toHaveText(/claude-stand-in/);
    await ask.$("button=Approve and start").click();

    await browser.waitUntil(
      async () => (await runningIn(plane)).some((one) => !wereRunning.includes(one)),
      { timeout: 30_000, timeoutMsg: "Approve and start started no chat" },
    );
  });
});
