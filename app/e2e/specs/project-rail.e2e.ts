// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, readFileSync } from "node:fs";
import { $, browser, expect } from "@wdio/globals";

/**
 * **The project rail, both modes, its peek and what it remembers** (#2915, spec #2897 R76–R89),
 * in the built app against the real core.
 *
 * - **Always shown**: the rail is a 52px column at the window's left below the title bar, its
 *   project buttons 36px with the current one selected, and the title bar has no show button.
 * - **Auto-hide**: the rail's own last button hides it; its column closes, the title bar's first
 *   control is the show button, and the rail is out of reach.
 * - **The peek**: the pointer at the window's left edge below the bar slides the rail in over
 *   the content; further than 70px away it goes; its last button keeps the rail shown.
 * - **Remembered**: the mode is written to the layout file under `rail` (`auto`, and nothing
 *   for always shown), which the next launch hands the window before it draws.
 *
 * Every test leaves the rail shown, which is what every spec after this one expects.
 */

const RAIL = '[data-scope="rail"][data-part="slot"]';
const PROJECTS = '[data-strip="Projects"]';
const SHOW = '[data-testid="title-bar"] [data-rail-show]';

/** What the rail looks like now, read in one pass. */
async function seen() {
  return browser.execute(() => {
    const slot = document.querySelector<HTMLElement>('[data-scope="rail"][data-part="slot"]');
    const peek = document.querySelector<HTMLElement>(
      '[data-scope="rail"][data-part="column"][data-peek]',
    );
    const current = slot?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]');
    return {
      width: slot?.getBoundingClientRect().width ?? -1,
      hidden: slot?.hasAttribute("data-hidden") ?? false,
      button: current?.getBoundingClientRect().width ?? -1,
      show: document.querySelector('[data-testid="title-bar"] [data-rail-show]') !== null,
      peek: peek !== null && !peek.hasAttribute("data-leaving"),
    };
  });
}

/** What the layout file keeps under `rail`, read where the window was told it is. */
async function kept(): Promise<unknown> {
  const path = await browser.execute(
    () =>
      (window as unknown as Record<string, { layout?: { path?: string } } | undefined>)
        .__CHARTER_AT_CREATION__?.layout?.path ?? "",
  );
  if (path === "" || !existsSync(path)) return undefined;
  return (JSON.parse(readFileSync(path, "utf8")) as { rail?: unknown }).rail;
}

/** Waits until the layout file keeps `rail` as `want`: the window writes it after the change. */
async function keptBecomes(want: unknown) {
  await browser.waitUntil(async () => (await kept()) === want, {
    timeout: 10_000,
    timeoutMsg: `the layout file never kept rail as ${String(want)}`,
  });
}

/** Moves the pointer to `x` px from the left edge of the window below the title bar. */
async function pointerAt(x: number) {
  await browser.execute((at: number) => {
    const below = document.querySelector<HTMLElement>(".window-below");
    if (!below) throw new Error("no window below the title bar");
    const box = below.getBoundingClientRect();
    document.dispatchEvent(
      new PointerEvent("pointermove", {
        bubbles: true,
        clientX: box.left + at,
        clientY: box.top + box.height / 2,
      }),
    );
  }, x);
}

/** Puts the rail back to always shown, however the test left it. */
async function shownAgain() {
  if (await $(SHOW).isExisting()) await $(SHOW).click();
  await browser.waitUntil(async () => !(await seen()).hidden, {
    timeout: 10_000,
    timeoutMsg: "the rail was never shown again",
  });
}

describe("the project rail", function () {
  this.timeout(120_000);

  before(async () => {
    await $(`${PROJECTS} [role="tab"][aria-selected="true"]`).waitForExist({ timeout: 30_000 });
  });

  afterEach(shownAgain);

  it("is a 52px column of 36px buttons at the window's left, the project in front selected", async () => {
    await shownAgain();
    const now = await seen();
    expect(Math.round(now.width)).toBe(52);
    expect(Math.round(now.button)).toBe(36);
    expect(now.show).toBe(false);
    expect(await $(`${PROJECTS} button[aria-label="Open a project…"]`).isExisting()).toBe(true);
  });

  it("hides from its own last button, and the title bar's first control shows it again", async () => {
    await $(`${RAIL} [data-rail-toggle="hide"]`).click();
    await browser.waitUntil(async () => Math.round((await seen()).width) === 0, {
      timeout: 5_000,
      timeoutMsg: "the rail's column never closed",
    });
    const hidden = await seen();
    expect(hidden.hidden).toBe(true);
    expect(hidden.show).toBe(true);
    await keptBecomes("auto");
    // Out of reach while hidden: nothing of it takes the keyboard.
    expect(await $(RAIL).getAttribute("aria-hidden")).toBe("true");

    await $(SHOW).click();
    await browser.waitUntil(async () => Math.round((await seen()).width) === 52, {
      timeout: 5_000,
      timeoutMsg: "the rail's column never opened again",
    });
    // Always shown is the default, which the file leaves out.
    await keptBecomes(undefined);
  });

  it("peeks at the window's left edge, goes past 70px, and is kept shown from the peek", async () => {
    await $(`${RAIL} [data-rail-toggle="hide"]`).click();
    // Its column closed: the slide is over before the pointer goes to the edge.
    await browser.waitUntil(async () => Math.round((await seen()).width) === 0, {
      timeout: 5_000,
      timeoutMsg: "the rail's column never closed",
    });

    await pointerAt(2);
    await browser.waitUntil(async () => (await seen()).peek, {
      timeout: 5_000,
      timeoutMsg: "the pointer at the edge never peeked the rail",
    });
    // Over the content: the column stays closed.
    expect(Math.round((await seen()).width)).toBe(0);

    await pointerAt(200);
    await browser.waitUntil(async () => !(await seen()).peek, {
      timeout: 5_000,
      timeoutMsg: "the peek never left once the pointer was away from the edge",
    });

    await pointerAt(2);
    await browser.waitUntil(async () => (await seen()).peek, { timeout: 5_000 });
    await $('[data-peek] [data-rail-toggle="keep"]').click();
    await browser.waitUntil(async () => !(await seen()).hidden, {
      timeout: 5_000,
      timeoutMsg: "Keep the projects sidebar shown never showed the rail",
    });
    expect((await seen()).peek).toBe(false);
    await keptBecomes(undefined);
  });
});
