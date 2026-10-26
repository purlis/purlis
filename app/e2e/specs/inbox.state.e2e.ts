// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect, $ } from "@wdio/globals";
import { rmSync, writeFileSync } from "node:fs";
import { pressGrant } from "../acting.js";
import { endEveryChat, pressAndStart } from "../opening.js";
import { ASKED_COMMAND, ASKS_A_PERMISSION, INBOX_ALLOWED } from "../harness.js";

/**
 * **A chat asks, the person answers in the Inbox, and the chat carries on** (#1692, spec #1688).
 *
 * The whole path, with nothing faked past the harness: the reporting harness runs Codex's
 * `PermissionRequest` hook through the real binary (`ASKS_A_PERMISSION`), the hook hands the ask
 * to the app over the socket the app opened and waits, the asks registry lists it, the title
 * bar's ✋ counts it and opens the Inbox, Allow there goes back on the chat's own hook, and the
 * harness prints what it does once it is allowed. Unanswered, the hook would decide nothing and
 * the line would never be printed.
 */

/** The title bar's hand, by its test id: it opens the Inbox now, not a menu. */
const HAND = '[data-testid="title-bar"] [data-testid="needs-you-button"]';

/** The Inbox view, wherever the right side's bar draws it. */
const INBOX = '[role="tabpanel"][data-view="inbox"]';

/** The permission ask the harness raised, as the Inbox draws it. */
const ASK = `${INBOX} .inbox-ask[data-source="permission"]`;

describe("the Inbox", () => {
  before(async () => {
    // One app for the whole run: a chat an earlier spec left open would be in the count.
    await endEveryChat();
    writeFileSync(ASKS_A_PERMISSION, "");
  });

  after(async () => {
    rmSync(ASKS_A_PERMISSION, { force: true });
    await endEveryChat();
  });

  it("lists a chat's permission ask, is opened by the ✋, and answering there lets the chat go on", async () => {
    await pressAndStart("New tab");
    // The harness holds its output until a line is typed; then it asks, and waits.
    const pane = await $('[data-testid="pane"]');
    await pane.click();
    await $(".xterm-helper-textarea").addValue("\n");

    // Counted on the hand, as one thing waiting.
    await browser.waitUntil(
      async () => {
        const hand = await $(HAND);
        if (!(await hand.isExisting())) return false;
        return /^\d+ things? waits? on you$/.test((await hand.getAttribute("aria-label")) ?? "");
      },
      { timeout: 30_000, interval: 250, timeoutMsg: "the hand never counted the ask" },
    );

    await $(HAND).click();
    const ask = await $(ASK);
    await ask.waitForDisplayed({
      timeout: 20_000,
      timeoutMsg: "the ✋ did not open the Inbox on the chat's ask",
    });
    // What it asks, as text, under the chat it came from.
    await expect(ask).toHaveText(expect.stringContaining(ASKED_COMMAND));
    await expect(await ask.$("button=Go to chat")).toBeDisplayed();

    // **Its scope and its ranks, as drawn** (qw359), in ranges and never exact pixels: the
    // switch opens on This project and fits the view; what needs you is the largest and
    // heaviest text of the row, and its kind the smallest; its kind's mark is drawn.
    const seen = await browser.execute(
      (inbox: string, row: string) => {
        const view = document.querySelector<HTMLElement>(inbox);
        const item = document.querySelector<HTMLElement>(row);
        const group = view?.querySelector<HTMLElement>('[role="radiogroup"]');
        const px = (selector: string) => {
          const part = item?.querySelector<HTMLElement>(selector);
          return part ? Number.parseFloat(getComputedStyle(part).fontSize) : 0;
        };
        const weight = (selector: string) => {
          const part = item?.querySelector<HTMLElement>(selector);
          return part ? Number.parseInt(getComputedStyle(part).fontWeight, 10) : 0;
        };
        return {
          picked:
            group?.querySelector('input[type="radio"]:checked')?.getAttribute("aria-label") ?? "",
          scopes: group?.querySelectorAll('input[type="radio"]').length ?? 0,
          // How far the switch runs past the view's right edge (≤ 0 when inside).
          past:
            group && view
              ? group.getBoundingClientRect().right - view.getBoundingClientRect().right
              : Number.NaN,
          primary: px(".inbox-primary"),
          meta: px(".inbox-meta"),
          heavy: weight(".inbox-primary"),
          light: weight(".inbox-meta"),
          mark: item?.querySelector(".inbox-kind svg")?.getBoundingClientRect().width ?? 0,
        };
      },
      INBOX,
      ASK,
    );
    expect(seen.picked).toMatch(/^This project, \d+ waiting on you$/);
    // This project and This workspace at least; All projects too, which the window lists.
    expect(seen.scopes).toBeGreaterThanOrEqual(2);
    expect(seen.past).toBeLessThanOrEqual(1);
    expect(seen.primary).toBeGreaterThan(seen.meta);
    expect(seen.heavy).toBeGreaterThan(seen.light);
    expect(seen.mark).toBeGreaterThan(0);

    // An Allow waits for its row to settle (`SETTLE_MS`, 1.5 s, #1695): the person reads the
    // ask before allowing it, or the press allows nothing.
    await pressGrant(await ask.$("button=Allow"));

    // The hook printed the window's decision, and the harness went on past it.
    await browser.waitUntil(
      async () => (await (await pane.$(".xterm-rows")).getText()).includes(INBOX_ALLOWED),
      { timeout: 30_000, interval: 250, timeoutMsg: "the chat never carried on after the Allow" },
    );
    // And the ask is gone from the Inbox: its source stopped waiting.
    await browser.waitUntil(async () => !(await $(ASK).isExisting()), {
      timeout: 20_000,
      timeoutMsg: "the answered ask stayed in the Inbox",
    });
  });
});
