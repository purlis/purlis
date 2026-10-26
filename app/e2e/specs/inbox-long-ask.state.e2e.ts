// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect, $ } from "@wdio/globals";
import { rmSync, writeFileSync } from "node:fs";
import { pressGrant } from "../acting.js";
import { endEveryChat, pressAndStart } from "../opening.js";
import { ASKS_A_LONG_PERMISSION, INBOX_ALLOWED, LONG_ASKED_COMMAND } from "../harness.js";

/**
 * **A 200-character ask reads whole in the Inbox, wrapped, with nothing scrolled sideways**
 * (#1146).
 *
 * The same path as `inbox.state.e2e.ts`, with a long command: the reporting harness runs Codex's
 * `PermissionRequest` hook through the real binary for {@link LONG_ASKED_COMMAND}, whose line is
 * as long as an ask's summary may be. Half of it is words a line breaks between, half a path it
 * can break only inside a word.
 *
 * What is measured is what the engine drew, never a pixel count: the line is on more than one
 * line box, nothing from the row up to the Inbox's panel is wider inside than it is outside, and
 * the row ends inside the panel.
 */

const HAND = '[data-testid="title-bar"] [data-testid="needs-you-button"]';
const INBOX = '[role="tabpanel"][data-view="inbox"]';
const ASK = `${INBOX} .inbox-ask[data-source="permission"]`;

/** What the engine drew of the ask's line, and what scrolls sideways around it. */
type Drawn = {
  says: string;
  lines: number;
  /** Each box from the row up to the Inbox's panel that is wider inside than out. */
  wider: string[];
  /** How far the row's right edge is past the Inbox's own, in CSS pixels (≤ 0 when inside). */
  past: number;
};

async function drawn(): Promise<Drawn> {
  return browser.execute(
    (ask: string, inbox: string) => {
      const row = document.querySelector<HTMLElement>(ask);
      const says = row?.querySelector<HTMLElement>(".ask-says");
      const view = document.querySelector<HTMLElement>(inbox);
      if (!row || !says || !view) return { says: "", lines: 0, wider: ["(not drawn)"], past: 0 };
      // The line boxes the text sits on: one rect per line, or more where a line holds inline
      // pieces, so the count is of distinct tops.
      const range = document.createRange();
      range.selectNodeContents(says);
      const tops = new Set([...range.getClientRects()].map((rect) => Math.round(rect.top)));
      const wider: string[] = [];
      // From the row up to the Inbox's own panel: what is outside it is the window's layout,
      // which other views share and this spec does not judge.
      for (let box: HTMLElement | null = row; box; box = box.parentElement) {
        // One pixel of rounding is the engine's, not a scroll.
        if (box.scrollWidth > box.clientWidth + 1 && getComputedStyle(box).overflowX !== "visible")
          wider.push(
            `${box.tagName.toLowerCase()}.${box.className} (${box.scrollWidth} > ${box.clientWidth})`,
          );
        if (box === view) break;
      }
      // **And what made it wide**: a red run names the elements in the Inbox that run past its
      // right edge, the furthest first, so the fix does not start with a guess.
      if (wider.length > 0) {
        const edge = view.getBoundingClientRect().left + view.clientWidth + 1;
        const past = [...view.querySelectorAll<HTMLElement>("*")]
          .map((el) => ({ el, right: el.getBoundingClientRect().right }))
          .filter((one) => one.right > edge)
          .sort((a, b) => b.right - a.right)
          .slice(0, 4)
          .map(
            ({ el, right }) =>
              `past the edge: ${el.tagName.toLowerCase()}.${el.className} ` +
              `"${(el.textContent ?? "").trim().slice(0, 40)}" ` +
              `(${Math.round(el.getBoundingClientRect().width)} wide, ${Math.round(right - edge)} over)`,
          );
        wider.push(...past);
        // Text that runs past its own box draws no box past the edge: name every element whose
        // content is wider than it, the deepest first, whatever its own overflow says.
        const spilling = [...view.querySelectorAll<HTMLElement>("*")]
          .filter((el) => el.scrollWidth > el.clientWidth + 1 && el.clientWidth > 0)
          .reverse()
          .slice(0, 4)
          .map(
            (el) =>
              `spills: ${el.tagName.toLowerCase()}.${el.className} ` +
              `"${(el.textContent ?? "").trim().slice(0, 40)}" ` +
              `(${el.scrollWidth} > ${el.clientWidth}, ${getComputedStyle(el).whiteSpace})`,
          );
        wider.push(...spilling);
      }
      return {
        says: says.textContent ?? "",
        lines: tops.size,
        wider,
        past: row.getBoundingClientRect().right - view.getBoundingClientRect().right,
      };
    },
    ASK,
    INBOX,
  );
}

describe("a long ask in the Inbox", () => {
  before(async () => {
    await endEveryChat();
    writeFileSync(ASKS_A_LONG_PERMISSION, "");
  });

  after(async () => {
    rmSync(ASKS_A_LONG_PERMISSION, { force: true });
    await endEveryChat();
  });

  it("reads the whole line, wraps it, and scrolls nothing sideways", async () => {
    await pressAndStart("New tab");
    // The harness holds its output until a line is typed; then it asks, and waits.
    const pane = await $('[data-testid="pane"]');
    await pane.click();
    await $(".xterm-helper-textarea").addValue("\n");

    await browser.waitUntil(
      async () => {
        const hand = await $(HAND);
        if (!(await hand.isExisting())) return false;
        return /^\d+ things? waits? on you$/.test((await hand.getAttribute("aria-label")) ?? "");
      },
      { timeout: 30_000, interval: 250, timeoutMsg: "the hand never counted the long ask" },
    );
    await $(HAND).click();
    const ask = await $(ASK);
    await ask.waitForDisplayed({
      timeout: 20_000,
      timeoutMsg: "the ✋ did not open the Inbox on the chat's long ask",
    });

    let seen: Drawn | undefined;
    await browser
      .waitUntil(
        async () => {
          seen = await drawn();
          return seen.says.includes(LONG_ASKED_COMMAND);
        },
        { timeout: 10_000, interval: 250 },
      )
      .catch(() => {
        throw new Error(`the Inbox did not read the long ask whole: ${JSON.stringify(seen)}`);
      });
    const said = seen as Drawn;
    // Whole: the command, and no ellipsis where the core would have cut it.
    expect(said.says).toContain(LONG_ASKED_COMMAND);
    expect(said.says).not.toContain("…");
    // Wrapped: on more than one line, however wide the Inbox is drawn.
    expect(said.lines).toBeGreaterThan(1);
    // And nothing it is drawn in scrolls sideways for it, or lets it out of the panel.
    expect(said.wider).toEqual([]);
    expect(said.past).toBeLessThanOrEqual(1);

    // Drawn whole, word for word, so the core offers Allow on it (`hooked::shown_in_full`).
    // It waits for its row to settle (`SETTLE_MS`, 1.5 s, #1695) before it allows anything.
    await pressGrant(await ask.$("button=Allow"));
    await browser.waitUntil(
      async () => (await (await pane.$(".xterm-rows")).getText()).includes(INBOX_ALLOWED),
      { timeout: 30_000, interval: 250, timeoutMsg: "the chat never carried on after the Allow" },
    );
  });
});
