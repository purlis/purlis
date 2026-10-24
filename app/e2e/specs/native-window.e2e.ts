// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $$, browser, expect } from "@wdio/globals";
import { builtInAppearance } from "../builtInThemes.js";

/**
 * **The native window follows the theme the page draws** (#2328, `src-tauri/src/nativewindow.rs`,
 * `src/theme/native.ts`), against the real app.
 *
 * The engine's tests hold which appearance and which colour a theme draws natively, and the
 * page's which it hands over and when. What only the real window can say is what the operating
 * system then draws: the appearance its own parts (the traffic lights, the menus, the dialogs)
 * are drawn in, read back from the platform. So this asks the window, rather than the page, and
 * draws a light theme natively whatever the runner's own appearance is.
 */

type Answer<T> = { value?: T; trouble?: string };

/** What `native_window_drawn` answers: the platform's appearance and colour for the window,
 *  the colour `null` where the platform gives none. */
type NativeDrawn = { appearance: string; background: string | null };

/** Asks the core `command` from the window, and insists it answered. */
async function ask<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const answer: Answer<T> = await browser.executeAsync(
    (cmd: string, given: Record<string, unknown> | null, done: (out: Answer<T>) => void) => {
      void window.__TAURI__.core
        .invoke(cmd, given ?? undefined)
        .then((value) => done({ value: value as T }))
        .catch((e: unknown) => done({ trouble: String(e) }));
    },
    command,
    args ?? null,
  );
  if (answer.trouble !== undefined) throw new Error(`${command} failed: ${answer.trouble}`);
  return answer.value as T;
}

/** What the page draws now: its appearance and, for the default theme, Purlis, the
 *  `window.background` of its dark (as the window was handed it) and its light (as the core
 *  resolves it: a pick of it asks the core, #3034). */
async function drawn(): Promise<{ appearance: string; backgrounds: Record<string, string> }> {
  const page = await browser.execute(() => {
    const handed = (window as unknown as Record<string, unknown>).__CHARTER_AT_CREATION__ as {
      built_in?: Record<string, { appearance: string; values: Record<string, string> }>;
    };
    const backgrounds: Record<string, string> = {};
    // The default's dark, as the window was handed it.
    const dark = handed?.built_in?.["purlis-dark"];
    if (dark !== undefined) backgrounds[dark.appearance] = dark.values["window.background"];
    return { appearance: document.documentElement.style.colorScheme, backgrounds };
  });
  page.backgrounds.light ??= builtInAppearance("purlis", "light").values["window.background"];
  return page;
}

async function untilTheStripIsRead(): Promise<void> {
  await browser.waitUntil(
    async () =>
      (await $$('[data-testid="title-bar"] [data-titlebar-all]').getElements()).length > 0,
    { timeout: 30_000, interval: 250, timeoutMsg: "the strip never listed the fixture plane" },
  );
}

describe("the native window", () => {
  it("is drawn in the appearance the page draws", async () => {
    await untilTheStripIsRead();
    const page = await drawn();
    expect(["dark", "light"]).toContain(page.appearance);
    await browser.waitUntil(
      async () => (await ask<NativeDrawn>("native_window_drawn")).appearance === page.appearance,
      {
        timeout: 10_000,
        interval: 250,
        timeoutMsg: `the native window was never drawn ${page.appearance}`,
      },
    );
  });

  it("shows light native chrome for a light theme, whatever the system's appearance, and dark for a dark one", async () => {
    await untilTheStripIsRead();
    const page = await drawn();
    expect(page.backgrounds.light).toMatch(/^#/);
    expect(page.backgrounds.dark).toMatch(/^#/);
    try {
      for (const appearance of ["light", "dark", "light"]) {
        await ask("draw_native_window", {
          appearance,
          background: page.backgrounds[appearance],
          followsSystem: false,
        });
        await browser.waitUntil(
          async () => (await ask<NativeDrawn>("native_window_drawn")).appearance === appearance,
          {
            timeout: 10_000,
            interval: 250,
            timeoutMsg: `the native window was not drawn ${appearance}`,
          },
        );
      }
    } finally {
      // Back to what the page draws, for the specs after this one.
      await ask("draw_native_window", {
        appearance: page.appearance,
        background: page.backgrounds[page.appearance],
        followsSystem: false,
      });
    }
  });

  it("refuses what is not an appearance or a colour", async () => {
    await untilTheStripIsRead();
    for (const args of [
      { appearance: "sepia", background: "#000000", followsSystem: false },
      { appearance: "dark", background: "black", followsSystem: false },
    ])
      await expect(ask("draw_native_window", args)).rejects.toThrow(/draw_native_window failed/);
  });
});
