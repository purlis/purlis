// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, readFileSync, realpathSync, renameSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { anEmptyRecord, copyFixturePlane } from "../harness.js";
import { ask, inFront, openFromTheStrip, PROJECTS } from "../switching.js";

/**
 * **Settings' tables keep each heading on one line, whole, in the real WebView** (DS-9, #1841;
 * spec #1832's seam 3). Both Dispatch tables wear the shared `.ui-table`: a heading never wraps,
 * and a long one is shortened (its whole words in its tooltip), so at the default window width no
 * heading is cut either.
 *
 * What jsdom cannot say: where WebKit breaks a line and how wide a word is drawn.
 * `DispatchLimits.test.tsx` and `DispatchGrants.test.tsx` hold what the cascade gives each
 * heading. Ranges, never pixels: a heading's words make one line box, and its words fit its cell.
 *
 * **Its own project**, copied into the run's tree: the fixture has one persona, and the grants
 * table is drawn only once the project grants something, so this copy adds a second persona and
 * a project grant from it. Nothing is written: the spec only measures, then closes the project's
 * Settings tab and the project, and brings the launch's project back to the front.
 */

const TABS = '[data-strip="Tabs"]';
const LEVEL = '[role="radiogroup"][aria-label="Level"]';
const GROUPS = 'nav[aria-label="Groups"]';
const LIMITS = 'table[aria-label="Dispatch limits"]';
const GRANTS = "table.dispatch-table";

/** This spec's project: the fixture with a second persona, `steward`, granted `devops`. */
const mine = (() => {
  const copied = copyFixturePlane();
  const renamed = join(dirname(copied), "settings-tables");
  renameSync(copied, renamed);
  const root = realpathSync(renamed);
  const devops = readFileSync(join(root, "personas", "devops", "persona.md"), "utf8");
  mkdirSync(join(root, "personas", "steward"), { recursive: true });
  writeFileSync(
    join(root, "personas", "steward", "persona.md"),
    devops.replace(/^name: devops$/m, "name: steward").replace(/devops/g, "steward"),
  );
  const manifest = join(root, "charter.toml");
  writeFileSync(
    manifest,
    `${readFileSync(manifest, "utf8").trimEnd()}\n\n[dispatch.grants]\nsteward = ["devops"]\n`,
  );
  anEmptyRecord(root);
  return root;
})();

/** One heading as drawn: its words, how many line boxes they make, and whether they are cut. */
type Heading = { words: string; lines: number; cut: boolean };

/** Every column heading of the table at `table`, measured; none where it is not drawn. */
async function headingsOf(table: string): Promise<Heading[] | null> {
  return browser.execute((selector: string) => {
    const drawn = document.querySelector(selector);
    if (drawn === null) return null;
    return [...drawn.querySelectorAll<HTMLElement>("thead th")].map((th) => {
      const range = document.createRange();
      range.selectNodeContents(th);
      // A line box per distinct top among the words' boxes, a pixel or two of rounding apart.
      const tops = [...range.getClientRects()]
        .filter((one) => one.width > 0)
        .map((one) => one.top)
        .sort((a, b) => a - b);
      let lines = 0;
      let last = Number.NEGATIVE_INFINITY;
      for (const top of tops)
        if (top - last > 2) {
          lines += 1;
          last = top;
        }
      return {
        words: th.textContent?.trim() ?? "",
        lines,
        cut: th.scrollWidth > th.clientWidth + 1,
      };
    });
  }, table);
}

/** Opens the group called `label` from the nav, and waits for it to be the one shown. */
async function openGroup(label: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute(
        (nav: string, wanted: string) => {
          const button = [...document.querySelectorAll<HTMLElement>(`${nav} button`)].find(
            (one) => one.textContent?.trim() === wanted,
          );
          if (button?.getAttribute("aria-current") === "true") return true;
          button?.click();
          return false;
        },
        GROUPS,
        label,
      ),
    { timeout: 20_000, interval: 250, timeoutMsg: `Settings offered no group ${label}` },
  );
}

/** Waits for the table at `table` and answers its headings, measured. */
async function measured(table: string): Promise<Heading[]> {
  let seen: Heading[] | null = null;
  await browser.waitUntil(
    async () => {
      seen = await headingsOf(table);
      return seen !== null && seen.length > 0;
    },
    { timeout: 30_000, interval: 250, timeoutMsg: `Settings never drew ${table}` },
  );
  return seen as unknown as Heading[];
}

/** Every heading drawn on one line and whole: each named, so a failure says which. */
function oneLineEach(headings: readonly Heading[]): void {
  for (const one of headings) {
    // The action column's heading has no words, so no line box.
    expect({ heading: one.words, lines: Math.min(one.lines, 2) }).toEqual({
      heading: one.words,
      lines: one.words === "" ? 0 : 1,
    });
    expect({ heading: one.words, cut: one.cut }).toEqual({ heading: one.words, cut: false });
  }
}

describe("Settings' tables, in the real WebView", function () {
  this.timeout(180_000);

  /** The project the launch opened, which this spec never touches. */
  let first = "";
  let was = { width: 1280, height: 800 };

  before(async () => {
    was = await browser.getWindowSize();
    // The default window's size (`settings-limits.e2e.ts` measures the page at it too).
    await browser.setWindowSize(1280, 800);
    first = (await ask<string[]>("open_planes"))[0];
    await openFromTheStrip(mine);

    await browser.executeAsync((done: () => void) => {
      void window.__TAURI__.event.emit("settings-asked").then(done, done);
    });
    await $(LEVEL).waitForExist({ timeout: 30_000 });
    await browser.waitUntil(
      async () =>
        browser.execute((selector: string) => {
          const radios = [...document.querySelectorAll<HTMLElement>(`${selector} [role="radio"]`)];
          const project = radios.find((one) => one.textContent?.trim() === "Project");
          if (project?.getAttribute("aria-checked") === "true") return true;
          project?.click();
          return false;
        }, LEVEL),
      { timeout: 30_000, interval: 250, timeoutMsg: "Settings never came to the Project level" },
    );
    await openGroup("Dispatch");
  });

  after(async () => {
    const tried = async (step: () => Promise<unknown>) => {
      try {
        await step();
      } catch {
        // The next step still runs.
      }
    };
    await tried(() =>
      browser.execute((strip: string) => {
        for (const closer of document.querySelectorAll<HTMLElement>(
          `${strip} button[aria-label^="Close Settings"]`,
        ))
          closer.click();
      }, TABS),
    );
    await tried(async () => {
      for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== first))
        await ask("close_plane", { plane });
    });
    await tried(() =>
      browser.waitUntil(
        async () => {
          if ((await inFront()) === first) return true;
          await browser.execute(
            (strip: string, path: string) =>
              [...document.querySelectorAll<HTMLElement>(`${strip} [role="tab"]`)]
                .find((tab) => tab.getAttribute("title") === path)
                ?.click(),
            PROJECTS,
            first,
          );
          return false;
        },
        { timeout: 20_000, interval: 250, timeoutMsg: "the launch's project never came back" },
      ),
    );
    await tried(() => browser.setWindowSize(was.width, was.height));
  });

  it("draws every heading of the limits table on one line, whole", async () => {
    const headings = await measured(LIMITS);
    expect(headings.map((one) => one.words)).toContain("Messages");
    oneLineEach(headings);
  });

  it("draws every heading of the grants table on one line, whole", async () => {
    const headings = await measured(GRANTS);
    expect(headings.map((one) => one.words)).toEqual([
      "Dispatch to",
      "Where it comes from",
      "Workspace",
      "Action",
    ]);
    oneLineEach(headings);
  });
});
