// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, readFileSync } from "node:fs";
import { $, browser, expect } from "@wdio/globals";
import { hearAnimations } from "../animations.js";
import { DEFAULT_WINDOW, theWindowHoldsTheRail, windowAt } from "../rail.js";

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
 * - **Never crowding the work** (#3077): at the narrowest window the rail hides itself, peeks
 *   as on auto-hide, and comes back as the window widens, with nothing written; a rail the person
 *   shows there stays shown, and one they hide stays hidden in a wide window.
 *
 * **A run of its own** (`wdio.rail.conf.ts`): a show the person presses holds over the window's
 * own hide for the rest of the launch, and the base run's specs measure narrow windows with the
 * rail as the window draws it, in the one app process a run keeps. The narrow window comes first
 * here, before anything has been shown. Every test leaves the rail shown.
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

/**
 * Puts the rail back to always shown, however the test left it, and waits until its column has
 * opened to its 52px. The person's show slides the column open over `duration.slide` (R84), and
 * the slot stops saying it is hidden as the slide starts, so a reading taken then finds the
 * column part of the way open.
 */
async function shownAgain() {
  if (await $(SHOW).isExisting()) await $(SHOW).click();
  await browser.waitUntil(
    async () => {
      const now = await seen();
      return !now.hidden && Math.round(now.width) === 52;
    },
    { timeout: 10_000, timeoutMsg: "the rail was never shown again at its 52px" },
  );
}

/** Runs `step`, and goes on whatever it threw: a clean-up that fails leaves the next one to run. */
async function tried(step: () => Promise<unknown>): Promise<void> {
  try {
    await step();
  } catch {
    // The next step still runs.
  }
}

describe("the project rail in a narrow window (#3077)", () => {
  /** ADR 0054's narrowest window. */
  const NARROWEST = { width: 1024, height: 768 } as const;

  /** Waits until the rail's column is `width` px wide. */
  async function columnIs(width: number, why: string) {
    await browser.waitUntil(async () => Math.round((await seen()).width) === width, {
      timeout: 10_000,
      timeoutMsg: why,
    });
  }

  before(async () => {
    await $(`${PROJECTS} [role="tab"][aria-selected="true"]`).waitForExist({ timeout: 30_000 });
  });

  afterEach(async () => {
    await tried(theWindowHoldsTheRail);
    await tried(shownAgain);
  });

  it("hides itself at the narrowest window, still peeks, and comes back as it widens, writing nothing", async () => {
    await theWindowHoldsTheRail();
    await shownAgain();
    await keptBecomes(undefined);

    await windowAt(NARROWEST);
    await columnIs(0, "the rail kept its column at the narrowest window");
    const narrow = await seen();
    expect(narrow.hidden).toBe(true);
    expect(narrow.show).toBe(true);
    // The edge still peeks it, as on auto-hide.
    await pointerAt(2);
    await browser.waitUntil(async () => (await seen()).peek, {
      timeout: 5_000,
      timeoutMsg: "the pointer at the edge never peeked the rail in the narrow window",
    });
    await pointerAt(200);
    await browser.waitUntil(async () => !(await seen()).peek, { timeout: 5_000 });

    await windowAt(DEFAULT_WINDOW);
    await columnIs(52, "the rail never came back as the window widened");
    expect((await seen()).show).toBe(false);
    // The window's hide was never the person's: the layout file says nothing of it.
    expect(await kept()).toBeUndefined();
  });

  it("stays shown when the window narrows once the person pinned it in a wide one", async () => {
    await theWindowHoldsTheRail();
    await $(`${RAIL} [data-rail-toggle="hide"]`).click();
    await columnIs(0, "the rail's own hide never closed its column");
    // The pin (R88): kept shown from its peek, in a window with room for it.
    await pointerAt(2);
    await browser.waitUntil(async () => (await seen()).peek, { timeout: 5_000 });
    await $('[data-peek] [data-rail-toggle="keep"]').click();
    await columnIs(52, "Keep the projects sidebar shown never showed the rail");
    await windowAt(NARROWEST);
    // The window's hide gives way to the person's pin, and goes on giving way: read again over
    // a second, so a hide that came late would be seen.
    for (let reading = 0; reading < 5; reading++) {
      const now = await seen();
      expect(Math.round(now.width)).toBe(52);
      expect(now.hidden).toBe(false);
      await browser.pause(250);
    }
    await keptBecomes(undefined);
  });

  it("never overrides the person: shown in a narrow window it stays, hidden in a wide one it stays", async () => {
    // Hidden by the person, then narrowed: the title bar's show is the way back.
    await theWindowHoldsTheRail();
    await $(`${RAIL} [data-rail-toggle="hide"]`).click();
    await columnIs(0, "the rail's own hide never closed its column");
    await windowAt(NARROWEST);

    // Shown by the person in the narrow window: it stays, however the window moves after.
    await $(SHOW).click();
    await columnIs(52, "the title bar's show never showed the rail in the narrow window");
    await windowAt(DEFAULT_WINDOW);
    await windowAt(NARROWEST);
    await columnIs(52, "the window hid a rail the person had shown");
    expect(await kept()).toBeUndefined();

    // Hidden by the person: written, and it stays hidden in a window with room for it.
    await $(`${RAIL} [data-rail-toggle="hide"]`).click();
    await columnIs(0, "the rail's own hide never closed its column");
    await keptBecomes("auto");
    await windowAt(DEFAULT_WINDOW);
    const wide = await seen();
    expect(Math.round(wide.width)).toBe(0);
    expect(wide.hidden).toBe(true);
    expect(await kept()).toBe("auto");
  });
});

describe("the project rail", () => {
  before(async () => {
    await $(`${PROJECTS} [role="tab"][aria-selected="true"]`).waitForExist({ timeout: 30_000 });
  });

  afterEach(() => tried(shownAgain));

  it("is a 52px column of 36px buttons at the window's left, the project in front selected", async () => {
    await shownAgain();
    // Read once the column has opened: shown, it grows to its width over the rail's transition.
    await browser.waitUntil(async () => Math.round((await seen()).width) === 52, {
      timeout: 10_000,
      timeoutMsg: "the shown rail never reached its width",
    });
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

    const peek = '[data-scope="rail"][data-part="column"][data-peek]';
    const coming = await hearAnimations(peek);
    await pointerAt(2);
    await browser.waitUntil(async () => (await seen()).peek, {
      timeout: 5_000,
      timeoutMsg: "the pointer at the edge never peeked the rail",
    });
    await coming.until(peek, "rail-peek-in");
    // Over the content: the column stays closed.
    expect(Math.round((await seen()).width)).toBe(0);

    const going = await hearAnimations(peek);
    await pointerAt(200);
    await going.until(peek, "rail-peek-out");
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
