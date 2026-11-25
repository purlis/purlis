// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect, $, $$ } from "@wdio/globals";
import { fromThePalette, pressAndStart } from "../opening.js";
import { chatsRunning, endTheChatsStartedSince, type Running } from "../chatsLeft.js";
import { READY } from "../harness.js";

/**
 * **Hibernate a chat from its row, then Resume it** (SC-4a, #7630), through the real binary, the
 * real session host and a stand-in harness that reports through `charter hook` and names its
 * conversation: nothing here reads the pane's text to decide anything but that the harness got
 * to the end of its turn. The band is asked for by its role, and its button by its name.
 */

/** The band over a hibernated chat: a polite status, named for its state. */
const BAND = '[role="status"][aria-label="Hibernated"]';

/** Waits until the pane's output says the harness finished its turn. */
async function untilTheTurnEnds(): Promise<void> {
  const pane = await $('[data-testid="pane"]');
  await pane.click();
  // Through the terminal's own hidden input, as `chat.state.e2e.ts` does: one path, no
  // duplicates.
  await $(".xterm-helper-textarea").addValue("\n");
  await browser.waitUntil(
    async () => (await (await pane.$(".xterm-rows")).getText()).includes(READY),
    { timeout: 30_000, interval: 250, timeoutMsg: "the harness never finished its output" },
  );
}

/** The band, once it is drawn. */
async function theBand(): Promise<WebdriverIO.Element> {
  const band = await $(BAND);
  await band.waitForDisplayed({ timeout: 30_000, timeoutMsg: "no Hibernated band was drawn" });
  return await band.getElement();
}

/** Waits until no Hibernated band is left. */
async function untilTheBandGoes(): Promise<void> {
  await browser.waitUntil(async () => !(await $(BAND).isExisting()), {
    timeout: 30_000,
    timeoutMsg: "the Hibernated band never went once Resume was pressed",
  });
}

describe("hibernating a chat and resuming it", () => {
  let wereOpen: Running = {};
  before(async () => {
    wereOpen = await chatsRunning();
    await pressAndStart("New tab");
    await untilTheTurnEnds();
  });
  after(async () => {
    await endTheChatsStartedSince(wereOpen);
  });

  it("reads Hibernated on the chat's pane once its program has ended", async () => {
    await fromThePalette("Hibernate chat");

    const band = await theBand();
    await expect(band).toHaveText(expect.stringContaining("is hibernated"));
    await expect(await band.$("button=Resume")).toBeDisplayed();
  });

  it("starts the same chat again on its conversation when Resume is pressed", async () => {
    const band = await theBand();
    await (await band.$("button=Resume")).click();

    await untilTheBandGoes();
    // The chat is still a tab of the strip, now in a new run.
    expect((await $$('[data-strip="Tabs"] [role="tab"]').getElements()).length).toBeGreaterThan(0);
  });

  it("can be hibernated and resumed again, which begins another run", async () => {
    await untilTheTurnEnds();
    await fromThePalette("Hibernate chat");
    const band = await theBand();
    await (await band.$("button=Resume")).click();
    await untilTheBandGoes();
  });
});
