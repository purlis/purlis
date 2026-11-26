// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, readFileSync } from "node:fs";
import { browser, expect, $, $$ } from "@wdio/globals";
import { fromThePalette, pressAndStart } from "../opening.js";
import { chatsRunning, endTheChatsStartedSince, type Running } from "../chatsLeft.js";
import { READY, SCENARIO_CONVERSATION, STARTED_WITH } from "../harness.js";
import { minimiseNoticesExpandedHere } from "../reach.js";

/**
 * **Hibernate a chat from its row, then Resume it** (SC-4a, #7630), through the real binary, the
 * real session host and a stand-in harness that reports through `charter hook` and names its
 * conversation: nothing here reads the pane's text to decide anything but that the harness got
 * to the end of its turn. The band is asked for by its role, and its button by its name.
 */

/** The band over a hibernated chat: a polite status, named for its state. */
const BAND = '[role="status"][aria-label="Hibernated"]';

/** The Notice that says why a chat was not hibernated. */
const REFUSED = '[role="status"][aria-label="Not hibernated"]';

/** What each start of the stand-in was handed, one line a start, oldest first. */
function startsSoFar(): string[] {
  if (!existsSync(STARTED_WITH)) return [];
  return readFileSync(STARTED_WITH, "utf8").split("\n").slice(0, -1);
}

/** Waits for the stand-in's `count`th start and answers what it was handed. */
async function theStart(count: number): Promise<string> {
  await browser.waitUntil(async () => startsSoFar().length >= count, {
    timeout: 30_000,
    timeoutMsg: `the harness was never started a ${count}th time`,
  });
  return startsSoFar()[count - 1];
}

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

/**
 * **The Notice `selector` names, shown**: it is a note, and a pane's notes fold into its bell
 * while notices are minimised, the window's default (#4754), so it stands in the band `hidden`.
 * A person presses the bell to see it, and so does this, once the Notice is there.
 */
async function shown(selector: string, timeoutMsg: string): Promise<WebdriverIO.Element> {
  const notice = await $(selector);
  await notice.waitForExist({ timeout: 30_000, timeoutMsg });
  const bell = await $(`.pane-frame:has(${selector}) [data-way="bell"]`);
  if ((await bell.isExisting()) && (await bell.getAttribute("aria-expanded")) === "false") {
    await bell.click();
    // The window keeps this choice: `minimiseNoticesExpandedHere` undoes it (#5586).
    await browser.execute(() => {
      (window as unknown as Record<string, boolean>).__e2eNoticesExpanded = true;
    });
  }
  await notice.waitForDisplayed({ timeout: 10_000, timeoutMsg });
  return await notice.getElement();
}

/** The band, once it is drawn. */
async function theBand(): Promise<WebdriverIO.Element> {
  return await shown(BAND, "no Hibernated band was drawn");
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
  let startsBefore = 0;
  before(async () => {
    wereOpen = await chatsRunning();
    startsBefore = startsSoFar().length;
    await pressAndStart("New tab");
    await untilTheTurnEnds();
  });
  after(async () => {
    // While its pane is still open: only a pane's bell sets the choice.
    await minimiseNoticesExpandedHere();
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
    // **A wake run on the same conversation**: the harness was started a second time, handed
    // the conversation its first start named (opencode's `-s <id>`), not started fresh.
    expect(await theStart(startsBefore + 1)).not.toContain(`-s ${SCENARIO_CONVERSATION}`);
    expect(await theStart(startsBefore + 2)).toContain(`-s ${SCENARIO_CONVERSATION}`);
  });

  it("refuses to hibernate a chat that is working, and says why", async () => {
    // Woken, its turn has begun and not ended: the stand-in reports its prompt as it starts.
    await fromThePalette("Hibernate chat");

    const refused = await shown(REFUSED, "no refusal was said for a chat that is working");
    await expect(refused).toHaveText(expect.stringContaining("this chat is working"));
    expect(await $(BAND).isExisting()).toBe(false);
    // Its Dismiss is an icon, named by its label, as every pane band's is.
    await (await refused.$('button[aria-label="Dismiss"]')).click();
  });

  it("can be hibernated and resumed again, which begins another run", async () => {
    await untilTheTurnEnds();
    await fromThePalette("Hibernate chat");
    const band = await theBand();
    await (await band.$("button=Resume")).click();
    await untilTheBandGoes();
    expect(await theStart(startsBefore + 3)).toContain(`-s ${SCENARIO_CONVERSATION}`);
  });
});
