// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect } from "@wdio/globals";
import { readPng, type Pixels } from "../png.js";

/**
 * **The theme specimen, against the real app** (#2330, `src-tauri/src/specimen.rs`).
 *
 * `Specimen.test.tsx` holds what the page draws and that it asks the core for nothing, and
 * `specimen/tests.rs` what it is handed. What only the real app can show is the picture: that a
 * hidden window of its own is drawn and photographed within the budget, that the picture is in
 * the theme and appearance asked for, and that it holds nothing of the window's.
 *
 * A chat's `theme_preview` reaches the same painter on its hook socket; the window's
 * `theme_specimen` is how a scenario asks for it.
 */

/** The budget the app holds one specimen to, in ms (`themepreview::BUDGET`). */
const BUDGET_MS = 5_000;

type Picture = { png: string; width: number; height: number; millis: number; appearance: string };

/** The specimen in `appearance`, and how long the whole ask took. */
async function specimen(appearance: "dark" | "light"): Promise<{ picture: Picture; took: number }> {
  const started = Date.now();
  const answer = await browser.executeAsync(
    (shown: string, done: (out: { ok?: unknown; trouble?: string }) => void) => {
      void window.__TAURI__.core
        .invoke("theme_specimen", { theme: "purlis", appearance: shown })
        .then((ok) => done({ ok }))
        .catch((e: unknown) => done({ trouble: String(e) }));
    },
    appearance,
  );
  if (answer.trouble !== undefined) throw new Error(`theme_specimen refused: ${answer.trouble}`);
  return { picture: answer.ok as Picture, took: Date.now() - started };
}

const pixels = (picture: Picture): Pixels => readPng(Buffer.from(picture.png, "base64"));

/** The theme's own `surface.base`, as the window holds it, in `[r, g, b]`. */
async function surfaceOf(pick: string): Promise<number[]> {
  // The window is handed each built-in by the pick that names it (#3034).
  const hex = await browser.execute((theme: string) => {
    const given = (
      window as unknown as Record<
        string,
        { built_in: Record<string, { values: Record<string, string> }> }
      >
    ).__CHARTER_AT_CREATION__;
    return given.built_in[theme]?.values["surface.base"] ?? "";
  }, pick);
  return [1, 3, 5].map((at) => Number.parseInt(hex.slice(at, at + 2), 16));
}

/** Whether two colours are the same within what a display's colour management moves. */
const near = (a: number[], b: number[]) => a.every((channel, n) => Math.abs(channel - b[n]) <= 12);

describe("the theme specimen", () => {
  it("is drawn within its budget, in the theme and appearance asked for", async () => {
    const dark = await specimen("dark");
    const light = await specimen("light");
    for (const { picture, took } of [dark, light]) {
      expect(picture.millis).toBeLessThanOrEqual(BUDGET_MS);
      expect(took).toBeLessThan(BUDGET_MS + 1_000);
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
    const [darkSurface, lightSurface] = [
      await surfaceOf("purlis-dark"),
      await surfaceOf("purlis-light"),
    ];
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
    const CANARY = "canary-7f3a-their-secret-project";
    const title = await browser.execute(() => document.title);
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
    await browser.execute((was: string) => {
      document.getElementById("specimen-canary")?.remove();
      localStorage.removeItem("purlis.specimen-canary");
      sessionStorage.removeItem("purlis.specimen-canary");
      document.title = was;
    }, title);
  });
});
