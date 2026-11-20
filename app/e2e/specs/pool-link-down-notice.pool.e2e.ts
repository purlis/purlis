// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import process from "node:process";
import { openThroughTheTrustQuestion } from "../opening.js";
import { addAndTryARemoteDockerPool } from "../poolsetup.js";
import { closeTheViewTabsOpenedSince, runFromThePalette, tabNames } from "../reach.js";

/**
 * **A dropped link says why within 30 seconds** (#7188, the fault suite's one UI test): a pool on
 * Docker on another machine, a chat running on it, and the link to the machine dropped with
 * `tests/remote-faults/faults.sh drop`. The window's Notice for the runner's link
 * (`RunnerLinkDownNotice`) shows its reason, in the host's own words, no more than 30 s after the
 * drop, and goes once the link is restored. Nothing is typed into a terminal.
 *
 * **In Pool setup's own run** (`wdio.pool.conf.ts`), on Linux's scenario job, which serves the
 * fixture of `tests/remote-docker/up.sh`.
 *
 * **Skipped until train A merges** (#7187): today the window says Reconnecting… and nothing
 * more for the first 30 s after it finds the link down (`QUIET_FOR_MS`), so a Notice 30 s after
 * the *drop* cannot be there. The ruling is that it is, and this is its spec; lifting the skip is
 * `NEEDS_TRAIN_A = false`.
 */
const NEEDS_TRAIN_A = true;

const POOL = "scenario-faults";
const IMAGE = process.env.PURLIS_E2E_REMOTE_POOL_IMAGE ?? "";
const ADDRESS = "pool@127.0.0.1:2222";
/** The fixture's sshd container, which the fault is put on. */
const SSHD = "sshd";
const FAULTS = fileURLToPath(new URL("../../../tests/remote-faults/faults.sh", import.meta.url));
/** The rule: nothing silent for more than this. */
const SAYS_WHY_WITHIN = 30_000;
/** The link comes back, and the Notice goes, inside the three-minute limit. */
const BACK_WITHIN = 180_000;

const faults = (...args: string[]) =>
  execFileSync("bash", [FAULTS, "--machine", SSHD, ...args], { stdio: "inherit" });

const runIt = NEEDS_TRAIN_A ? describe.skip : describe;

runIt("A dropped link to a pool's machine", () => {
  let wereOpen: string[] | null = null;

  after(async () => {
    faults("restore");
    if (wereOpen !== null) await closeTheViewTabsOpenedSince(wereOpen);
  });

  it("shows why the runner is not linked within 30 seconds, and goes when the link is back", async () => {
    expect(IMAGE).toMatch(/@sha256:[0-9a-f]{64}$/);
    await openThroughTheTrustQuestion({ within: 5_000, ifAsked: true });
    wereOpen = await tabNames();
    await runFromThePalette("Add a pool", "pool.add");
    await addAndTryARemoteDockerPool({ pool: POOL, image: IMAGE, address: ADDRESS });

    const notice = await $(`*=Pool ${POOL}'s runner is not linked`);
    expect(await notice.isExisting()).toBe(false);

    faults("drop");
    const droppedAt = Date.now();
    await notice.waitForDisplayed({
      timeout: SAYS_WHY_WITHIN,
      interval: 500,
      timeoutMsg: `no Notice said why the link was down within ${SAYS_WHY_WITHIN / 1000} s of the drop`,
    });
    expect(Date.now() - droppedAt).toBeLessThanOrEqual(SAYS_WHY_WITHIN + 1_000);
    // Its reason, in the host's words: not the quiet Reconnecting…, and not nothing.
    const said = (await notice.parentElement().getText()).replace(/\s+/g, " ");
    expect(said).not.toMatch(/^\s*Reconnecting…?\s*$/);
    expect(said.length).toBeGreaterThan(`Pool ${POOL}'s runner is not linked`.length + 10);

    faults("restore");
    await browser.waitUntil(async () => !(await notice.isDisplayed().catch(() => false)), {
      timeout: BACK_WITHIN,
      interval: 1_000,
      timeoutMsg: "the Notice never went after the link was restored",
    });
  });
});
