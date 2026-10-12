// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync } from "node:fs";
import { $, browser, expect } from "@wdio/globals";
import { ask } from "../switching.js";
import { GONE_DRAWN, GONE_KEPT, LAYOUT_FILE, THE_LAYOUT } from "../wdio.layout.conf.js";

/**
 * **A gone project's line dismissed at an earlier launch stays dismissed across a relaunch, at
 * window level** (#1291), in the built app against the real core.
 *
 * `wdio.layout.conf.ts` launches the app on a machine store that remembers two projects open in
 * the last window, both gone, and a layout file that keeps the first one's line dismissed under
 * its own path (`project-gone:<path>`). That is what a person who dismissed it before quitting
 * leaves behind, so this launch is the relaunch: the window is handed the kept dismissal before
 * it draws, and the core says both projects are gone.
 *
 * - **Only the undismissed project's line is drawn.** The other is asserted absent at the same
 *   moment the drawn one is present, so "absent" is never a list that had not arrived yet.
 * - **The core still says it is gone**, which is the one thing that keeps the dismissal: a cause
 *   that changed would let it go (`settleGoneKept`).
 * - **The kept dismissal is still in the layout file** once the window has settled it.
 *
 * It only reads, and presses nothing: the layout spec after it in this run holds the file to
 * exactly what was written.
 */

/** The two gone projects' lines in the window, by their causes, as `Notice` marks them. */
async function lines(): Promise<{ kept: boolean; drawn: boolean; causes: string[] }> {
  return browser.execute(
    (kept: string, drawn: string) => {
      const causes = [...document.querySelectorAll<HTMLElement>("[data-cause]")].map(
        (one) => one.dataset.cause ?? "",
      );
      return { kept: causes.includes(kept), drawn: causes.includes(drawn), causes };
    },
    `project-gone:${GONE_KEPT}`,
    `project-gone:${GONE_DRAWN}`,
  );
}

type Recents = { gone?: { path: string }[] | null };

describe("a gone project's line dismissed before a relaunch", () => {
  before(async () => {
    // The launch's own project is drawn, so the window has restored what it could.
    await $('[data-testid="explorer"]').waitForExist({
      timeout: 30_000,
      timeoutMsg: "the explorer was never drawn",
    });
  });

  it("is still remembered as gone by the core", async () => {
    let gone: string[] = [];
    await browser.waitUntil(
      async () => {
        gone = ((await ask<Recents>("recent_planes")).gone ?? []).map((one) => one.path);
        return gone.includes(GONE_KEPT) && gone.includes(GONE_DRAWN);
      },
      {
        timeout: 20_000,
        interval: 250,
        timeoutMsg: `the core never listed both projects as gone: ${JSON.stringify(gone)}`,
      },
    );
  });

  it("is not drawn, while the line of the gone project nobody dismissed is", async () => {
    let seen = await lines();
    await browser.waitUntil(
      async () => {
        seen = await lines();
        return seen.drawn;
      },
      {
        timeout: 30_000,
        interval: 250,
        timeoutMsg: `the window never said ${GONE_DRAWN} is gone; its lines: ${JSON.stringify(seen.causes)}`,
      },
    );
    expect(seen.kept).toBe(false);

    // And it stays away once the window has asked the core which projects are still gone.
    await browser.pause(2_000);
    seen = await lines();
    expect(seen.drawn).toBe(true);
    expect(seen.kept).toBe(false);
  });

  it("is still kept in the layout file", async () => {
    const file = JSON.parse(readFileSync(LAYOUT_FILE, "utf8")) as {
      dismissed?: Record<string, string[]>;
    };
    expect(file.dismissed?.[GONE_KEPT]).toEqual(THE_LAYOUT.dismissed[GONE_KEPT]);
  });
});
