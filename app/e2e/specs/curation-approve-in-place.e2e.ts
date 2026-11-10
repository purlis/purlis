// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { projectRoot } from "../acting.js";
import {
  closesTheTabsItOpens,
  endTheSessionsStartedSince,
  focusWorkspace,
  runFromThePalette,
} from "../reach.js";
import { projectInFront, runningIn } from "../running.js";
import { wrapsClaudeCode } from "../harness.js";
import { openTheStatusCard } from "../reading.js";

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

/**
 * **Not on Linux**, where every sandboxed Claude Code chat runs inside purlis's wrap, on a
 * sandbox-only profile (ADR 0067 as amended 2026-11-10): a curation action starts its chat sandboxed, so a
 * profile a project declares, the only kind that asks for an approval, starts none there. The
 * approval in place is the same on every system, and macOS runs it.
 */
const describeHere = wrapsClaudeCode() ? describe.skip : describe;

describeHere("a curation action refused for its profile's approval", function () {
  this.timeout(180_000);

  let root = "";
  let plane = "";
  /** The Local file as it was, or `null` where there was none. */
  let local: string | null = null;
  let wereRunning: number[] = [];

  // Hooks of a kind run in the order they are registered: the strip is read once alpha is
  // focused, and its view tabs are closed once the chats are ended, before the Local file is put
  // back.
  before(async () => {
    root = await projectRoot();
    await focusWorkspace("alpha");
  });

  after(async () => {
    await endTheSessionsStartedSince(wereRunning);
    await focusWorkspace("alpha");
  });

  closesTheTabsItOpens();

  before(async () => {
    plane = await projectInFront();
    wereRunning = await runningIn(plane);
    const file = join(root, "charter.local.toml");
    local = existsSync(file) ? readFileSync(file, "utf8") : null;
    writeFileSync(file, withTheProfileAsDefault(local ?? ""));
  });

  after(() => {
    if (root === "") return;
    const file = join(root, "charter.local.toml");
    if (local === null) rmSync(file, { force: true });
    else writeFileSync(file, local);
  });

  it("offers Approve and start, showing what it runs, and starts the action's chat", async () => {
    await runFromThePalette("Safe remove", ACTION);

    // Refused, with the approval offered beside the refusal; nothing started.
    // The chip's card, which stands in front: the Inbox's Results list the same offer, and the card
    // can cover that copy, which is then in the DOM first and never clickable.
    // No card pops by itself (#6004): the refusal waits behind the Notices item's dot, whose
    // press shows its card, as a person opens it.
    await openTheStatusCard();
    const offer = await $(
      '//*[@data-chip="status/bar/notices"]//button[contains(., "Approve and start…")]',
    );
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
    await ask.$("aria/Approve and start").click();

    await browser.waitUntil(
      async () => (await runningIn(plane)).some((one) => !wereRunning.includes(one)),
      { timeout: 30_000, timeoutMsg: "Approve and start started no chat" },
    );
  });
});
