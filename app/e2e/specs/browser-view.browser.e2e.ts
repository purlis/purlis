// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import { openThroughTheTrustQuestion } from "../opening.js";
import { runFromThePalette } from "../reach.js";

/**
 * **A browser view tab draws frames and takes a click** (#2025, spec #2008's one window
 * end-to-end test), in the built app against its real session host and the pinned engine: the
 * person opens a browser from the palette, its view tab draws the engine's frames on its
 * canvas, and a click on the page takes the browser over, which the bar above the page says.
 * Every hop below the window has its own tests against a stand-in for the next; here none is.
 *
 * **A run of its own** (`wdio.browser.conf.ts`): the host downloads the engine on the first
 * browser, which takes as long as the download does.
 */

const VIEW = '[data-testid="browser-view"]';
const FRAME = `${VIEW} canvas.browser-frame`;
const SURFACE = `${VIEW} [data-testid="browser-surface"]`;
const BAR = `${VIEW} .browser-bar`;

/** How long the first browser may take: the engine's download, then its start. */
const FIRST_BROWSER = 300_000;

/** What the window says on its trouble line, where it says anything: why a browser did not start. */
async function troubleSaid(): Promise<string> {
  return browser.execute(() =>
    [...document.querySelectorAll('[role="alert"], [role="status"]')]
      .map((one) => one.textContent ?? "")
      .filter((said) => said.trim() !== "")
      .join(" | "),
  );
}

/** Waits until `done`, at most `timeout`; one that never comes fails with what the window said. */
async function until(done: () => Promise<boolean>, timeout: number, what: string): Promise<void> {
  try {
    await browser.waitUntil(done, { timeout, interval: 500 });
  } catch {
    throw new Error(`${what}; the window said: ${await troubleSaid()}`);
  }
}

describe("a browser view tab", () => {
  it("draws the engine's frames and takes a click, which takes the browser over", async () => {
    await openThroughTheTrustQuestion({ within: 5_000, ifAsked: true });

    await runFromThePalette("New browser", "browser.new");

    await $(VIEW).waitForDisplayed({
      timeout: FIRST_BROWSER,
      timeoutMsg: "no browser view tab was drawn",
    });
    // A frame drawn sizes the canvas to the page it shows.
    await until(
      async () =>
        browser.execute((selector: string) => {
          const canvas = document.querySelector<HTMLCanvasElement>(selector);
          return canvas !== null && canvas.hasAttribute("width") && canvas.width > 0;
        }, FRAME),
      FIRST_BROWSER,
      "the browser view drew no frame",
    );
    expect(await $(BAR).getAttribute("data-driving")).toBe("agent");

    await $(SURFACE).click();

    await until(
      async () => (await $(BAR).getAttribute("data-driving")) === "person",
      30_000,
      "the click did not take the browser over",
    );
    await expect($(`${BAR} [role="status"]`)).toHaveText(
      expect.stringContaining("You are driving this browser"),
    );

    // Handed back, as the person leaves it.
    await $(`${BAR} button=Done`).click();
    await until(
      async () => (await $(BAR).getAttribute("data-driving")) === "agent",
      30_000,
      "Done did not hand the browser back",
    );
  });
});
