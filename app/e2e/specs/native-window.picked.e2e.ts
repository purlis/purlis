// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $$, browser, expect } from "@wdio/globals";
import { ask } from "../switching.js";
import { PICKED } from "../wdio.picked.conf.js";

/**
 * **A project's theme pick reaches the native window through the real path** (#2583, #2328).
 *
 * `wdio.picked.conf.ts` writes `[theme] use = "purlis-light"` into the fixture project's
 * `charter.toml` and starts the app on it. Nothing here draws: no call to `draw_native_window`.
 * The window is created by the core in the appearance of the theme its first frame draws
 * (`windowprefs.rs`, `theme::first_frame`), and the page hands the theme it draws on to the core
 * (`theme/native.ts`). What the platform then says the window is drawn in is the answer: its
 * appearance and the background colour it was given (#2760), each read back from the platform.
 * The default theme is Classic's dark one, drawn forced rather than following the system, so a
 * pick that never reached the window reads back dark, and in the dark theme's colour, on any
 * runner.
 */

type Handed = {
  first?: { follows_system?: boolean; frames?: { theme?: { appearance?: string } }[] } | null;
  built_in?: { name: string; values: Record<string, string> }[];
};

/** What `native_window_drawn` answers: the platform's appearance and colour for the window. */
type NativeDrawn = { appearance: string; background: string };

async function handed(): Promise<Handed> {
  return browser.execute(
    () => (window as unknown as Record<string, unknown>).__CHARTER_AT_CREATION__ as never,
  );
}

async function untilTheStripIsRead(): Promise<void> {
  await browser.waitUntil(
    async () => (await $$('[data-strip="Workspaces"] [role="tab"]').getElements()).length > 0,
    { timeout: 30_000, interval: 250, timeoutMsg: "the strip never listed the fixture plane" },
  );
}

describe("the native window, on a project that picks a light theme", () => {
  it("was created for the picked theme's first frame", async () => {
    await untilTheStripIsRead();
    const { first } = await handed();
    expect(first?.follows_system).toBe(false);
    const appearances = (first?.frames ?? []).map((frame) => frame.theme?.appearance);
    expect(appearances.length).toBeGreaterThan(0);
    expect(new Set(appearances)).toEqual(new Set(["light"]));
  });

  it(`is drawn light, as the page is, without the test drawing it (${PICKED})`, async () => {
    await untilTheStripIsRead();
    const page = await browser.execute(() => document.documentElement.style.colorScheme);
    expect(page).toBe("light");
    await browser.waitUntil(
      async () => (await ask<NativeDrawn>("native_window_drawn")).appearance === "light",
      {
        timeout: 10_000,
        interval: 250,
        timeoutMsg: "the native window was never drawn light for the project's pick",
      },
    );
  });

  it(`is given the picked theme's window.background (${PICKED})`, async () => {
    await untilTheStripIsRead();
    // The theme's own value, as the core resolved it, not a colour written here.
    const theme = (await handed()).built_in?.find((each) => each.name === PICKED);
    const background = theme?.values["window.background"]?.toLowerCase();
    expect(background).toMatch(/^#[0-9a-f]{6}$/);
    let drawn: NativeDrawn | undefined;
    await browser
      .waitUntil(
        async () => {
          drawn = await ask<NativeDrawn>("native_window_drawn");
          return drawn.background === background;
        },
        { timeout: 10_000, interval: 250 },
      )
      .catch(() => undefined);
    expect(drawn?.background).toBe(background);
  });
});
