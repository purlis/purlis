// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, readFileSync, realpathSync, renameSync, writeFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { openMenuOn } from "../acting.js";
import { ALL_WORKSPACES } from "../titleBarWorkspaces.js";
import { focusWorkspace } from "../reach.js";
import { anEmptyRecord, copyFixturePlane } from "../harness.js";
import { closeProject, openThroughTheTrustQuestion } from "../opening.js";

/**
 * **Settings › Theme's hover preview, in the built app** (#2331, spec #2313): pointing at a
 * theme's card draws that theme in the window, and leaving the card puts the person's theme
 * back, with nothing written.
 *
 * `src/settings/themeGallery.test.tsx` holds the gallery against a mock core: its cards, the
 * keyboard, what a press writes. What only the app can show is the whole path: the core's
 * theme engine reading a project's theme folder, resolving the card's theme, and the window
 * drawing it and putting its own back.
 *
 * **The pointer is the page's own events**, as `tab-chip.e2e.ts` sends them: a WebDriver move
 * does not make WebKit match `:hover` (run 37260967190), and React hears a card entered and left
 * from the `pointerout` and `pointerover` a pointer sends. The keyboard's way in is a focus, as
 * a WebDriver Tab moves none.
 *
 * **Its own project, copied into the run's tree**, holding a theme folder of the project's, and
 * closed through the window when it is done, as `settings-gears.e2e.ts` does.
 */

const PROJECTS = '[data-strip="Projects"]';
const LEVEL = '[role="radiogroup"][aria-label="Level"]';
const CARD = '[data-theme-card="e2e-dusk"]';

/** The theme folder's base surface in each appearance: near Purlis Classic's, and not it. */
const DUSK = { dark: "#141414", light: "#f7f6f3" };

const mine = (() => {
  const copied = copyFixturePlane();
  const renamed = join(dirname(copied), "gallery");
  renameSync(copied, renamed);
  const root = realpathSync(renamed);
  anEmptyRecord(root);
  const folder = join(root, "themes", "e2e-dusk");
  mkdirSync(folder, { recursive: true });
  writeFileSync(
    join(folder, "theme.json"),
    JSON.stringify({
      name: "E2E Dusk",
      extends: "purlis-classic",
      appearances: [
        { id: "dark", tokens: { "surface.base": DUSK.dark } },
        { id: "light", tokens: { "surface.base": DUSK.light } },
      ],
    }),
  );
  return root;
})();

/** The committed file as the copy had it: nothing here may change it. */
const sharedBefore = readFileSync(join(mine, "charter.toml"), "utf8");

/** What the app answered a command with, insisting it answered at all. */
async function ask<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  const answer = await browser.executeAsync(
    (
      name: string,
      passed: Record<string, unknown>,
      done: (out: { ok?: unknown; trouble?: string }) => void,
    ) => {
      void window.__TAURI__.core
        .invoke(name, passed)
        .then((ok) => done({ ok }))
        .catch((e: unknown) => done({ trouble: String(e) }));
    },
    command,
    args,
  );
  if (answer.trouble !== undefined) throw new Error(`${command} refused: ${answer.trouble}`);
  return answer.ok as T;
}

/** The base surface the window is drawn in right now, as the theme module wrote it. */
const surface = () =>
  browser.execute(() =>
    document.documentElement.style.getPropertyValue("--surface-base").trim().toLowerCase(),
  );

/** Sends the card the events a pointer sends going onto it (`over`) or off it to the nav. */
async function point(over: boolean): Promise<void> {
  await browser.execute(
    (card: string, onto: boolean) => {
      const it = document.querySelector<HTMLElement>(card);
      const away = document.querySelector<HTMLElement>('nav[aria-label="Groups"]');
      if (!it || !away) throw new Error("the card or the nav is not drawn");
      const at = it.getBoundingClientRect();
      const init = {
        bubbles: true,
        cancelable: true,
        pointerId: 1,
        pointerType: "mouse",
        isPrimary: true,
        clientX: at.left + at.width / 2,
        clientY: at.top + at.height / 2,
      };
      // React reads a move between two of its elements from the `pointerout` of the one left,
      // whose `relatedTarget` is the one entered (and passes over that one's `pointerover`).
      const [from, to] = onto ? [away, it] : [it, away];
      from.dispatchEvent(new PointerEvent("pointerout", { ...init, relatedTarget: to }));
      to.dispatchEvent(new PointerEvent("pointerover", { ...init, relatedTarget: from }));
      to.dispatchEvent(new PointerEvent("pointermove", init));
    },
    CARD,
    over,
  );
}

/** Waits until the window's base surface is `want`'s, or one of `want`. */
async function surfaceBecomes(want: string | readonly string[], why: string): Promise<void> {
  const wanted = typeof want === "string" ? [want] : want;
  let saw = "";
  await browser.waitUntil(
    async () => {
      saw = await surface();
      return wanted.includes(saw);
    },
    { timeout: 20_000, timeoutMsg: `${why}: the window drew ${saw}, not ${wanted.join(" or ")}` },
  );
}

describe("Settings › Theme's hover preview", function () {
  this.timeout(180_000);

  let first = "";

  before(async () => {
    first = (await ask<string[]>("open_planes"))[0];
    await $(`${PROJECTS} button[aria-label="Open a project…"]`).click();
    const box = await $('[data-setting="open-by-path"] input');
    await box.waitForDisplayed({ timeout: 20_000 });
    await box.addValue(mine);
    await $('form.by-path button[type="submit"]').click();
    await openThroughTheTrustQuestion();
    await browser.waitUntil(async () => (await ask<string[]>("open_planes")).includes(mine), {
      timeout: 30_000,
      timeoutMsg: "this spec's project never opened",
    });
    await $(ALL_WORKSPACES).waitForExist({ timeout: 30_000 });
    await focusWorkspace("Plane root");
    // The rail has no gear (#2915): its project's menu has the same row.
    await openMenuOn(`${PROJECTS} [role="tab"][aria-selected="true"]`);
    await $('[role="menu"]').$('[role="menuitem"]*=Settings').click();
    await browser.waitUntil(
      async () =>
        (await browser.execute(
          (selector: string) =>
            document
              .querySelector(`${selector} [role="radio"][aria-checked="true"]`)
              ?.textContent?.trim() ?? "",
          LEVEL,
        )) === "Project",
      { timeout: 30_000, timeoutMsg: "Settings never opened at the Project level" },
    );
    await $('nav[aria-label="Groups"]').$("button=Theme").click();
    await $(CARD).waitForExist({ timeout: 30_000 });
  });

  after(async () => {
    const closer = `${PROJECTS} [role="tab"][aria-label="${basename(mine)}"]`;
    if (await $(closer).isExisting()) {
      await closeProject(closer);
      await browser.waitUntil(async () => !(await $(closer).isExisting()), {
        timeout: 20_000,
        timeoutMsg: "this spec's project stayed on the strip",
      });
    }
    for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== first)) {
      await ask("close_plane", { plane });
    }
  });

  it("draws a card's theme while the pointer is on it, and puts yours back when it leaves", async () => {
    const yours = await surface();
    expect(yours).not.toBe("");
    expect([DUSK.dark, DUSK.light]).not.toContain(yours);

    await point(true);
    await surfaceBecomes([DUSK.dark, DUSK.light], "pointing at E2E Dusk's card");

    await point(false);
    await surfaceBecomes(yours, "leaving E2E Dusk's card");

    // Nothing was chosen, and no file of the project's was written.
    const gallery = await ask<{ yours: unknown }>("theme_gallery", {
      at: { plane: mine, workspace: null },
    });
    expect(gallery.yours).toBeNull();
    expect(readFileSync(join(mine, "charter.toml"), "utf8")).toBe(sharedBefore);
  });

  it("draws it while the keyboard is on the card, and puts yours back when it moves off", async () => {
    const yours = await surface();
    await browser.execute(
      (card: string) => document.querySelector<HTMLElement>(card)?.focus(),
      CARD,
    );
    await surfaceBecomes([DUSK.dark, DUSK.light], "the keyboard on E2E Dusk's card");

    await browser.execute(() =>
      document.querySelector<HTMLElement>('nav[aria-label="Groups"] button')?.focus(),
    );
    await surfaceBecomes(yours, "the keyboard off E2E Dusk's card");
  });
});
