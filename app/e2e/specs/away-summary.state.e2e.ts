// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect, $ } from "@wdio/globals";
import { READY } from "../harness.js";
import { endEveryChat, pressAndStart, showView } from "../opening.js";

/**
 * **While you were away, in a narrow window** (#1551, the summary of #1514): the summary's
 * links wrap inside its band, in the real WebView, and nothing is wider than the band.
 *
 * What the summary counts and says is `awayCounts.test.ts`'s and `AwaySummary.window.test.tsx`'s.
 * Only the engine can say how the sentence and its links break in the Inbox at the side, which
 * at the narrowest window purlis supports (1024 px, ADR 0054) is far narrower than the sentence.
 *
 * **A real chat comes to wait on the person while they are away.** This is the reporting
 * harness's run (`wdio.state.conf.ts`): its chat runs a turn until a line is typed and then
 * says, through its own `stop` hook, that it waits on you, as `chat.state.e2e.ts` shows.
 *
 * **The time away is pretended; nothing else is.** A summary is drawn after five minutes or
 * more away (`AWAY_AFTER_MS`), which a scenario cannot spend. So the spec tells the window it
 * was left (`blur`), lets the chat come to wait, moves the page's clock past five minutes, and
 * tells the window the person is back (`focus`): the events the window hears when a person
 * goes to another app and comes back. The clock is put back after.
 */

/** The Inbox view, wherever the right side's bar draws it. */
const INBOX = '[role="tabpanel"][data-view="inbox"]';

/** The summary, as the Inbox lists it among its Notices. */
const SUMMARY = `${INBOX} [data-cause="away-summary"]`;

/** The narrowest window purlis supports (ADR 0054, amended 2026-09-26). */
const NARROWEST = 1024;

/** What a tab says its chat is doing, by the accessible name on its state mark. */
async function doing(): Promise<string> {
  const mark = await $('[data-strip="Tabs"] [role="tab"] .state');
  return (await mark.isExisting()) ? ((await mark.getAttribute("aria-label")) ?? "") : "";
}

/** Waits until the chat says `state`, and says what it said instead if it never does. */
async function until(state: string): Promise<void> {
  let last = "";
  try {
    await browser.waitUntil(
      async () => {
        last = await doing();
        return last === state;
      },
      { timeout: 30_000, interval: 200 },
    );
  } catch {
    throw new Error(`the chat never showed ${JSON.stringify(state)}; it showed ${last}`);
  }
}

describe("the away summary in a narrow window (#1551)", () => {
  let was = { width: 0, height: 0 };

  before(async () => {
    // One app for the whole run: a chat an earlier spec left open would be counted too.
    await endEveryChat();
    was = await browser.getWindowSize();
    await browser.setWindowSize(NARROWEST, 768);
    await browser.waitUntil(async () => browser.execute(() => window.innerWidth <= 1024), {
      timeout: 20_000,
      timeoutMsg: `the window never became ${NARROWEST} px wide`,
    });
    // The page's clock, which the spec moves past the time away. Put back in `after`.
    await browser.execute(() => {
      const page = window as unknown as { awayShift?: number; realNow?: () => number };
      page.realNow = Date.now.bind(Date);
      page.awayShift = 0;
      Date.now = () => (page.realNow ?? (() => 0))() + (page.awayShift ?? 0);
    });
  });

  after(async () => {
    await browser.execute(() => {
      const page = window as unknown as { realNow?: () => number };
      if (page.realNow) Date.now = page.realNow;
    });
    const dismiss = await $(`${SUMMARY} button[data-way=dismiss]`);
    if (await dismiss.isExisting()) await dismiss.click();
    await endEveryChat();
    await showView("Memory");
    await browser.setWindowSize(was.width, was.height);
  });

  it("wraps the sentence and its link inside the band, and nothing is wider than it", async () => {
    await pressAndStart("New tab");
    await until("running");

    // The keyboard on the chat's terminal first: its line is typed after the person has left.
    const pane = await $('[data-testid="pane"]');
    await pane.click();
    // The person goes to another app.
    await browser.execute(() => window.dispatchEvent(new Event("blur")));

    // Releasing the output lets the harness reach its `stop` hook: the chat waits on you.
    // Through the terminal's hidden input, as `chat.state.e2e.ts` types it.
    await $(".xterm-helper-textarea").addValue("\n");
    await browser.waitUntil(
      async () => (await (await pane.$(".xterm-rows")).getText()).includes(READY),
      { timeout: 30_000, interval: 250, timeoutMsg: "the harness never finished its output" },
    );
    await until("waiting on you");

    // Six minutes later, the person is back.
    await browser.execute(() => {
      (window as unknown as { awayShift: number }).awayShift = 6 * 60 * 1000;
      window.dispatchEvent(new Event("focus"));
    });

    await showView("Inbox");
    const summary = await $(SUMMARY);
    await summary.waitForDisplayed({
      timeout: 20_000,
      timeoutMsg: "no away summary was drawn in the Inbox after the time away",
    });
    await expect(summary).toHaveText(
      expect.stringContaining("While you were away: 1 chat waiting on you"),
    );

    const seen = await browser.execute((which: string) => {
      const line = document.querySelector<HTMLElement>(which);
      if (!line) throw new Error("the summary went before it was measured");
      const band = line.getBoundingClientRect();
      // The sentence, as the Notice sets it beside its ways out.
      const body = line.querySelector<HTMLElement>("[data-scope=notice][data-part=message]");
      const text = body?.getBoundingClientRect();
      // Every box each link is drawn in: a link that wraps is drawn in more than one.
      const links = [...line.querySelectorAll<HTMLElement>(".away-part")].map((link) =>
        [...link.getClientRects()].map((box) => ({ left: box.left, right: box.right })),
      );
      // The rows the sentence takes: one top per row its words are set on.
      const range = document.createRange();
      if (body) range.selectNodeContents(body);
      const rows = new Set([...range.getClientRects()].map((box) => Math.round(box.top))).size;
      return {
        band: { left: band.left, right: band.right },
        text: text ? { left: text.left, right: text.right } : null,
        links,
        rows,
        sideways: line.scrollWidth - line.clientWidth,
        page: document.documentElement.scrollWidth - document.documentElement.clientWidth,
      };
    }, SUMMARY);

    // The case this is about: the band is narrower than the sentence, so it wraps.
    expect(seen.rows).toBeGreaterThan(1);
    // Each link is drawn inside the band, on whichever row it is set.
    expect(seen.links.length).toBeGreaterThan(0);
    for (const boxes of seen.links) {
      expect(boxes.length).toBeGreaterThan(0);
      for (const box of boxes) {
        expect(box.left).toBeGreaterThanOrEqual(seen.band.left - 0.5);
        expect(box.right).toBeLessThanOrEqual(seen.band.right + 0.5);
      }
    }
    // Nothing is wider than the band, and the page never scrolls sideways.
    expect(seen.text).not.toBeNull();
    expect(seen.text?.right ?? Infinity).toBeLessThanOrEqual(seen.band.right + 0.5);
    expect(seen.sideways).toBeLessThanOrEqual(0);
    expect(seen.page).toBeLessThanOrEqual(0);
  });
});
