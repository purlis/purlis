// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect } from "@wdio/globals";
import { readPng, type Pixels } from "../png.js";
import { builtInAppearance } from "../builtInThemes.js";
import { recordBar } from "../timings.js";

/**
 * **The theme specimen, against the real app** (#2330, `src-tauri/src/specimen.rs`).
 *
 * `Specimen.test.tsx` holds what the page draws and that it asks the core for nothing, and
 * `specimen/tests.rs` what it is handed. What only the real app can show is the picture: that a
 * hidden window of its own is drawn and photographed within the budget, that the picture is in
 * the theme and appearance asked for, and that it holds nothing of the window's.
 *
 * **The budget is measured here, not asserted** (#4419): a loaded macOS runner draws past 5 s
 * for reasons of its own, and the app then answers what it answers a chat, "ask for it again".
 * So the spec asks again, as a chat would, until it holds the picture it needs, and each time,
 * met or missed, goes on the job as a speed bar (`timings.ts`'s `recordBar`).
 *
 * A chat's `theme_preview` reaches the same painter on its hook socket; the window's
 * `theme_specimen` is how a scenario asks for it.
 */

/** The budget the app holds one specimen to, in ms (`themepreview::BUDGET`). */
const BUDGET_MS = 5_000;

type Picture = { png: string; width: number; height: number; millis: number; appearance: string };

/**
 * What the app answers when a specimen ran past its budget, or another was being drawn: both say
 * to ask again (`specimen.rs`'s `over_budget` and `BUSY`).
 */
const ASK_AGAIN = /ask (for it )?again/;

/** How long the spec keeps asking for one picture: the wait is for the picture, not for 5 s. */
const PATIENCE_MS = 60_000;

/**
 * The specimen in `appearance`, asked for again while the app says to. How long each took is the
 * app's own `millis`, timed from asking for its window to holding its picture, as the budget is
 * (#3022); a clock around the WebDriver round trip would count a loaded runner's queueing too.
 */
async function specimen(appearance: "dark" | "light"): Promise<{ picture: Picture }> {
  const until = Date.now() + PATIENCE_MS;
  const bar = `the theme specimen, ${appearance}`;
  for (;;) {
    const answer = await browser.executeAsync(
      (shown: string, done: (out: { ok?: unknown; trouble?: string }) => void) => {
        void window.__TAURI__.core
          .invoke("theme_specimen", { theme: "purlis", appearance: shown })
          .then((ok) => done({ ok }))
          .catch((e: unknown) => done({ trouble: String(e) }));
      },
      appearance,
    );
    if (answer.trouble === undefined) {
      const picture = answer.ok as Picture;
      recordBar(bar, picture.millis, BUDGET_MS);
      return { picture };
    }
    if (!ASK_AGAIN.test(answer.trouble) || Date.now() > until)
      throw new Error(`theme_specimen refused: ${answer.trouble}`);
    recordBar(bar, null, BUDGET_MS, answer.trouble);
    await browser.pause(1_000);
  }
}

const pixels = (picture: Picture): Pixels => readPng(Buffer.from(picture.png, "base64"));

/** Purlis's own `surface.base` in `appearance`, as the core resolves it, in `[r, g, b]`. */
function surfaceOf(appearance: string): number[] {
  const hex = builtInAppearance("purlis", appearance).values["surface.base"];
  return [1, 3, 5].map((at) => Number.parseInt(hex.slice(at, at + 2), 16));
}

/** Whether two colours are the same within what a display's colour management moves. */
const near = (a: number[], b: number[]) => a.every((channel, n) => Math.abs(channel - b[n]) <= 12);

const CANARY = "canary-7f3a-their-secret-project";

describe("the theme specimen", () => {
  /** The window's title before a test planted the canary in it; `null` when none was planted. */
  let titleWas: string | null = null;

  // The canary covers the whole window: it comes down however the test ended, a budget missed
  // included, or every later spec finds it on top (#4712).
  afterEach(async () => {
    await browser.execute((was: string | null) => {
      document.getElementById("specimen-canary")?.remove();
      localStorage.removeItem("purlis.specimen-canary");
      sessionStorage.removeItem("purlis.specimen-canary");
      if (was !== null) document.title = was;
    }, titleWas);
    titleWas = null;
  });

  it("is drawn, timed against its budget, in the theme and appearance asked for", async () => {
    const dark = await specimen("dark");
    const light = await specimen("light");
    for (const { picture } of [dark, light]) {
      // Timed, and the time recorded against the budget (`specimen`); a miss is no failure.
      expect(picture.millis).toBeGreaterThan(0);
      expect(picture.width).toBeGreaterThanOrEqual(1280);
      expect(picture.height).toBeGreaterThanOrEqual(800);
    }
    expect(dark.picture.appearance).toBe("dark");
    expect(light.picture.appearance).toBe("light");
    // The page's own ground, in its corner, is the theme's `surface.base`.
    const [darkGround, lightGround] = [
      pixels(dark.picture).at(2, 2),
      pixels(light.picture).at(2, 2),
    ];
    const [darkSurface, lightSurface] = [surfaceOf("dark"), surfaceOf("light")];
    expect({ ground: darkGround, near: near(darkGround, darkSurface) }).toEqual({
      ground: darkGround,
      near: true,
    });
    expect({ ground: lightGround, near: near(lightGround, lightSurface) }).toEqual({
      ground: lightGround,
      near: true,
    });
    expect(near(darkGround, lightGround)).toBe(false);
  });

  it("holds nothing of the window's: the same picture whatever the window holds", async () => {
    const before = pixels((await specimen("dark")).picture);
    // What the window shows and keeps, changed: a string no fixture holds, written over the
    // whole window and into its storage. A picture of the window would change with it.
    titleWas = await browser.execute(() => document.title);
    await browser.execute((canary: string) => {
      localStorage.setItem("purlis.specimen-canary", canary);
      sessionStorage.setItem("purlis.specimen-canary", canary);
      document.title = canary;
      const over = document.createElement("div");
      over.id = "specimen-canary";
      over.textContent = canary.repeat(200);
      over.style.cssText = "position:fixed;inset:0;z-index:99999;background:#ff00ff;color:#00ff00";
      document.body.append(over);
    }, CANARY);
    const after = pixels((await specimen("dark")).picture);
    expect([after.width, after.height]).toEqual([before.width, before.height]);
    let differ = 0;
    for (let y = 0; y < before.height; y += 4)
      for (let x = 0; x < before.width; x += 4)
        if (before.at(x, y).join() !== after.at(x, y).join()) differ++;
    expect(differ).toBe(0);
  });
});
