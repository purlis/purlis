// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect } from "@wdio/globals";
import { showView } from "../opening.js";
import { focusWorkspaceAt, workspaceTabs } from "../reach.js";
import { bringChatForward, endChatsStartedHere, startAChat, type Started } from "../started.js";

/**
 * **The Chats view's scope: This tab, Workspace or All** (#1679, #1671 stories 17-19), in the
 * built app, with two real chats in two tabs.
 *
 * - **This tab** lists the chats of the tab in front, and follows the tab brought forward.
 * - **Workspace** lists the chats of the focused workspace: each chat under exactly one of the
 *   strip's places, and both of these under the same one, since both were started from it.
 * - **All** lists every chat, whichever place is focused.
 *
 * Which place a chat started from the strip is filed under is the core's to say, so this spec
 * does not assume one: it walks every place on the strip and asks where each chat is listed.
 * What `chatsScope.ts` decides row by row is its unit tests'; what is the window's here is the
 * switch drawn by the real engine, its picks reaching the real list, and the tab and the strip
 * driving it. A chat the scope hides that needs you is `chats-scope.state.e2e.ts`, the run whose
 * harness reports a turn's end.
 *
 * **It leaves the window as it found it**: the scope and the focused place are put back, and
 * both chats are ended.
 */

const SWITCH = '[role="radiogroup"][aria-label="Show the chats of"]';

/** The scope the switch has picked, by its words. */
async function picked(): Promise<string> {
  return browser.execute(
    (group: string) =>
      document.querySelector(`${group} input[type="radio"]:checked`)?.closest("label")
        ?.textContent ?? "",
    SWITCH,
  );
}

/** Picks the scope that says `says`, by its own click, and waits for the switch to say so. */
async function pick(says: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute(
        (group: string, words: string) => {
          const radio = [
            ...document.querySelectorAll<HTMLElement>(`${group} input[type="radio"]`),
          ].find((one) => one.closest("label")?.textContent === words);
          if ((radio as HTMLInputElement | undefined)?.checked !== true) radio?.click();
          return radio !== undefined;
        },
        SWITCH,
        says,
      ),
    { timeout: 20_000, interval: 100, timeoutMsg: `the scope switch never offered ${says}` },
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

/** The list once it has stopped changing: the same rows read twice, a beat apart. */
async function settled(): Promise<number[]> {
  let last = "";
  let rows: number[] = [];
  await browser.waitUntil(
    async () => {
      rows = await listed();
      const now = rows.join(",");
      const same = now === last;
      last = now;
      return same;
    },
    { timeout: 10_000, interval: 300, timeoutMsg: "the Chats list never settled" },
  );
  return rows;
}

/** Waits until the list draws every chat of `shown` and none of `hidden`. */
async function untilListing(shown: number[], hidden: number[], why: string): Promise<void> {
  let rows: number[] = [];
  await browser
    .waitUntil(
      async () => {
        rows = await listed();
        return (
          shown.every((one) => rows.includes(one)) && hidden.every((one) => !rows.includes(one))
        );
      },
      { timeout: 20_000, interval: 200 },
    )
    .catch(() => {
      throw new Error(`${why}: the list drew ${JSON.stringify(rows)}`);
    });
}

describe("the Chats view's scope", function () {
  this.timeout(240_000);

  let scopeBefore = "";
  let focusBefore = -1;
  let first: Started | undefined;
  let second: Started | undefined;
  /** The two chats' numbers, once both have started. */
  const one = () => first?.session ?? -1;
  const other = () => second?.session ?? -1;

  before(async () => {
    await showView("Chats");
    focusBefore = (await workspaceTabs()).findIndex((tab) => tab.selected);
    first = await startAChat();
    second = await startAChat();

    // The switch is drawn once there is a chat to list.
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
      console.warn("chats-scope: the scope was not put back", trouble);
    }
    try {
      if (focusBefore >= 0) await focusWorkspaceAt(focusBefore);
    } catch (trouble) {
      console.warn("chats-scope: the workspace was not focused again", trouble);
    }
    await endChatsStartedHere([first, second]);
  });

  it("draws the three scopes as one switch, narrowest first", async () => {
    const words = await browser.execute(
      (group: string) =>
        [...document.querySelectorAll(`${group} input[type="radio"]`)].map(
          (one) => one.closest("label")?.textContent,
        ),
      SWITCH,
    );
    expect(words).toEqual(["This tab", "Workspace", "All"]);
    // One row of segments: every segment on one line, inside the view.
    const boxes = await browser.execute((group: string) => {
      const section = document
        .querySelector('[data-testid="chats-section"]')
        ?.getBoundingClientRect();
      return [...document.querySelectorAll(`${group} input[type="radio"]`)].map((one) => {
        // A segment is drawn by its label; the radio in it is hidden.
        const box = (one.closest("label") ?? one).getBoundingClientRect();
        return {
          top: box.top,
          width: box.width,
          inside: section !== undefined && box.right <= section.right + 1,
        };
      });
    }, SWITCH);
    for (const box of boxes) {
      expect(box.width).toBeGreaterThan(0);
      expect(box.inside).toBe(true);
      expect(Math.abs(box.top - boxes[0].top)).toBeLessThanOrEqual(1);
    }
  });

  it("lists This tab's chats alone, and follows the tab brought forward", async () => {
    await pick("This tab");
    await untilListing([other()], [one()], "with the second tab in front");

    await bringChatForward(one());
    await untilListing([one()], [other()], "with the first tab brought forward");

    await bringChatForward(other());
    await untilListing([other()], [one()], "with the second tab back in front");
  });

  it("lists a chat under exactly one place in Workspace, and both of these under the same", async () => {
    await pick("Workspace");
    const places = await workspaceTabs();
    const under = new Map<number, string[]>([
      [one(), []],
      [other(), []],
    ]);
    for (let at = 0; at < places.length; at++) {
      await focusWorkspaceAt(at);
      const rows = await settled();
      for (const [session, where] of under) if (rows.includes(session)) where.push(places[at].name);
    }
    const firstUnder = under.get(one()) ?? [];
    const secondUnder = under.get(other()) ?? [];
    expect(firstUnder).toHaveLength(1);
    expect(secondUnder).toEqual(firstUnder);
  });

  it("lists every chat in All, whichever place is focused", async () => {
    await pick("All");
    const places = await workspaceTabs();
    for (let at = 0; at < places.length; at++) {
      await focusWorkspaceAt(at);
      await untilListing([one(), other()], [], `in All with ${places[at].name} focused`);
    }
  });
});
