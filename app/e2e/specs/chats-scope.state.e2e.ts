// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import { READY } from "../harness.js";
import { showView } from "../opening.js";
import { sessionInFront } from "../reach.js";
import { endChatsStartedHere, startAChat } from "../started.js";

/**
 * **A chat the Chats view's scope leaves out is named when it needs you** (#1679, #1671 story
 * 19), in the built app, with a chat whose harness really ended its turn.
 *
 * This is the run whose harness reports through `charter hook` (`wdio.state.conf.ts`): a line
 * typed into the first chat lets its output go and its `stop` fire, and from then on it waits on
 * the person. A second chat is started in a tab of its own, and the view is put on This tab:
 * the first chat is not listed, and the line under the filter names it, with Go beside it, which
 * brings it forward. Which chats a scope lists is `chats-scope.e2e.ts`.
 *
 * **It leaves the window as it found it**: the scope is put back and both chats are ended.
 */

const SWITCH = '[role="radiogroup"][aria-label="Show the chats of"]';
const HIDDEN = '[data-testid="chats-section"] .chats-hidden';
const GO = '[data-testid="chats-section"] .chats-hidden-go';

/** The scope the switch has picked, by its words. */
async function picked(): Promise<string> {
  return browser.execute(
    (group: string) =>
      document.querySelector(`${group} input[type="radio"]:checked`)?.closest("label")
        ?.textContent ?? "",
    SWITCH,
  );
}

/** Picks the scope that says `says`, by its own click. */
async function pick(says: string): Promise<void> {
  await browser.execute(
    (group: string, words: string) => {
      const radio = [
        ...document.querySelectorAll<HTMLElement>(`${group} input[type="radio"]`),
      ].find((one) => one.closest("label")?.textContent === words);
      if ((radio as HTMLInputElement | undefined)?.checked !== true) radio?.click();
    },
    SWITCH,
    says,
  );
  await browser.waitUntil(async () => (await picked()) === says, {
    timeout: 10_000,
    interval: 100,
    timeoutMsg: `the scope switch never picked ${says}`,
  });
}

/** The chats the list draws, by number. */
async function listed(): Promise<number[]> {
  return browser.execute(() =>
    [
      ...document.querySelectorAll(
        '[data-testid="chats-section"] [role="tree"] [role="treeitem"][data-session]',
      ),
    ].map((row) => Number(row.getAttribute("data-session"))),
  );
}

/** The name the list gives chat `session`, read in All, where every chat is listed. */
async function nameOf(session: number): Promise<string> {
  let name = "";
  await browser.waitUntil(
    async () => {
      name = await browser.execute(
        (number: number) =>
          document.querySelector(
            `[data-testid="chats-section"] [role="treeitem"][data-session="${number}"] .session`,
          )?.textContent ?? "",
        session,
      );
      return name !== "";
    },
    { timeout: 20_000, interval: 100, timeoutMsg: `the list never named chat ${session}` },
  );
  return name;
}

describe("a chat the scope leaves out that needs you", function () {
  this.timeout(240_000);

  let scopeBefore = "";
  let waiting = -1;
  let other = -1;

  before(async () => {
    // Every file starts with no chat running (the chatsLeft guard), so no chat an earlier spec
    // left waiting is named before this one's.
    await showView("Chats");

    waiting = (await startAChat()).session;
    // The harness holds its output until a line is typed; then its turn ends and it waits.
    const pane = await $(`[data-testid="pane"][data-session="${waiting}"]`);
    await pane.click();
    await $(".xterm-helper-textarea").addValue("\n");
    await browser.waitUntil(
      async () => (await (await pane.$(".xterm-rows")).getText()).includes(READY),
      { timeout: 30_000, interval: 250, timeoutMsg: "the first chat never finished its output" },
    );
    await browser.waitUntil(
      async () =>
        browser
          .execute(
            (number: number) =>
              document
                .querySelector(`[role="treeitem"][data-session="${number}"] .shown-state`)
                ?.getAttribute("data-state") ?? "",
            waiting,
          )
          .then((state) => state === "needs-you" || /wait/.test(state)),
      { timeout: 30_000, interval: 250, timeoutMsg: "the first chat never waited on the person" },
    );

    other = (await startAChat()).session;
    await browser.waitUntil(async () => (await picked()) !== "", {
      timeout: 20_000,
      interval: 100,
      timeoutMsg: "the Chats view drew no scope switch",
    });
    scopeBefore = await picked();
  });

  // Each step is tried even when the one before it failed: one app process serves the run.
  after(async () => {
    try {
      if (scopeBefore !== "") await pick(scopeBefore);
    } catch (trouble) {
      console.warn("chats-scope.state: the scope was not put back", trouble);
    }
    await endChatsStartedHere([waiting, other].filter((one) => one >= 0));
  });

  it("names the waiting chat outside this tab, under the filter, with Go beside it", async () => {
    await pick("All");
    const name = await nameOf(waiting);

    await pick("This tab");
    await browser.waitUntil(
      async () => {
        const rows = await listed();
        return rows.includes(other) && !rows.includes(waiting);
      },
      { timeout: 20_000, interval: 200, timeoutMsg: "This tab still listed the waiting chat" },
    );

    const line = await $(HIDDEN);
    await browser.waitUntil(async () => (await line.getText()).includes(name), {
      timeout: 20_000,
      interval: 200,
      timeoutMsg: `the line under the filter never named ${name}`,
    });
    expect(await line.getText()).toBe(`${name} needs you, outside this tab.`);
    // Said to a screen reader as it changes.
    expect(await line.getAttribute("role")).toBe("status");

    const go = await $(GO);
    await expect(go).toBeDisplayed();
    expect(await go.getAttribute("aria-label")).toBe(
      `Go to ${name}, which needs you outside this tab`,
    );
    expect(await go.getAttribute("data-leads-to")).toBe(String(waiting));
  });

  it("brings the waiting chat forward from Go", async () => {
    await browser.execute((selector: string) => {
      document.querySelector<HTMLElement>(selector)?.click();
    }, GO);
    await browser.waitUntil(async () => (await sessionInFront()) === waiting, {
      timeout: 20_000,
      interval: 100,
      timeoutMsg: "Go never brought the waiting chat forward",
    });
    // Its own tab now: This tab lists it, and names nothing outside.
    await browser.waitUntil(async () => (await listed()).includes(waiting), {
      timeout: 20_000,
      interval: 200,
      timeoutMsg: "This tab never listed the chat Go brought forward",
    });
    expect(await $(GO).isExisting()).toBe(false);
  });
});
