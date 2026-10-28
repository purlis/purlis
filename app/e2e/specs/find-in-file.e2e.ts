// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";
import { $, browser, expect } from "@wdio/globals";
import { pressMenuPath, ROW } from "../menus.js";
import {
  closeTab,
  focusOn,
  focusWorkspace,
  keyWithFallback,
  openMenuOn,
  projectRoot,
  tabTexts,
  typeInto,
  untilInFront,
} from "../acting.js";
import { showView } from "../opening.js";

/**
 * **Find in a file** (#946, #1088; #1797's third line), in the built app against the real core:
 * the find bar a file tab opens on `⌘F` (`Ctrl+F` off a Mac), its count, Next and Previous, the
 * wrap from the last match to the first and back, Enter and Shift+Enter in its field, and Escape.
 *
 * The spec puts a file of five lines beside the fixture branch's `README.md`
 * (`.worktrees/svc/fix-login`, untracked, which a branch's tree lists), with `needle` on three of
 * them and once in capitals, so a find that minded case would count two. It opens the branch's
 * *Files* tab from its menu in the Explorer, as `file-preview.e2e.ts` does, and picks the file.
 * Each match is on a line of its own, so which match the bar is on is read off the line the
 * active mark is drawn in.
 *
 * **It leaves the project as it found it**: the file is removed, the tab closed, and the left
 * side shows Chats again.
 */

const PIECE = "fix-login";
const FILE = "e2e-find.txt";
const TEXT = ["first needle here", "nothing on this one", "second needle", "plain", "NEEDLE third"];
const TAB = `Files · ${PIECE}`;
const SHOWN = `section.piece-files-shown[aria-label="${FILE}"]`;
const BAR = `${SHOWN} [role="search"]`;
const FIELD = `${BAR} input[aria-label="Find"]`;
const MAC = process.platform === "darwin";

/** What the find bar says and marks, read in one pass. */
function found() {
  return browser.execute(
    (shown: string, bar: string) => {
      const section = document.querySelector(shown);
      const search = document.querySelector(bar);
      return {
        open: search !== null,
        label: search?.getAttribute("aria-label") ?? "",
        count: (search?.querySelector(".pane-find-count")?.textContent ?? "").trim(),
        // Every match: the one the bar is on is drawn with its own mark instead.
        marked: section?.querySelectorAll(".cm-find-match, .cm-find-match-active").length ?? 0,
        // The line the active mark is drawn in, which says which match the bar is on.
        on: (
          section?.querySelector(".cm-find-match-active")?.closest(".cm-line")?.textContent ?? ""
        ).trim(),
      };
    },
    SHOWN,
    BAR,
  );
}

/** Waits until the bar says `count` with its active mark on the line `on`. */
async function until(count: string, on: string): Promise<void> {
  let seen = await found();
  await browser.waitUntil(
    async () => {
      seen = await found();
      return seen.count === count && seen.on === on;
    },
    {
      timeout: 10_000,
      interval: 150,
      timeoutMsg: `the find bar never said ${count} on "${on}": ${JSON.stringify(seen)}`,
    },
  );
}

/** Presses the bar's button named `name`, by its own click from the page. */
async function pressInTheBar(name: string): Promise<void> {
  const pressed = await browser.execute(
    (bar: string, label: string) => {
      const button = document.querySelector<HTMLElement>(`${bar} button[aria-label="${label}"]`);
      button?.click();
      return button !== null;
    },
    BAR,
    name,
  );
  if (!pressed) throw new Error(`the find bar has no ${name} button`);
}

describe("find in a file", function () {
  this.timeout(180_000);

  let branch = "";

  before(async () => {
    branch = join(await projectRoot(), "workspaces", "alpha", ".worktrees", "svc", PIECE);
    writeFileSync(join(branch, FILE), TEXT.join("\n") + "\n");

    // The branch's menu in the Explorer opens its Files tab (FM-2).
    await showView("Explorer");
    await focusWorkspace("alpha");
    await openMenuOn(`.spot[data-testid="piece-svc-${PIECE}"]`);
    await pressMenuPath(ROW.browseFiles);
    await untilInFront(TAB);

    const row = `[data-testid="piece-files-tree"] [data-row="file:svc/${PIECE}:${FILE}"]`;
    await $(row).waitForExist({ timeout: 20_000, timeoutMsg: `the tree never listed ${FILE}` });
    await browser.execute((css: string) => document.querySelector<HTMLElement>(css)?.click(), row);
    await $(`${SHOWN} .light-editor .cm-content`).waitForExist({
      timeout: 20_000,
      timeoutMsg: `${FILE} was never drawn in the light editor`,
    });
  });

  after(async () => {
    if ((await tabTexts()).includes(TAB)) await closeTab(TAB);
    rmSync(join(branch, FILE), { force: true });
    await showView("Chats");
  });

  it("opens its find bar on the platform's find key, with the keyboard in the field", async () => {
    expect((await found()).open).toBe(false);
    await focusOn(`${SHOWN} .light-editor .cm-content`);

    await keyWithFallback(
      [MAC ? "Meta" : "Control", "f"],
      { key: "f", metaKey: MAC, ctrlKey: !MAC },
      async () => (await found()).open,
    );

    await browser.waitUntil(async () => (await found()).open, {
      timeout: 10_000,
      timeoutMsg: "the find key opened no find bar on the file",
    });
    const bar = await found();
    expect(bar.label).toBe(`Find in ${FILE}`);
    await browser.waitUntil(
      async () =>
        browser.execute(
          (field: string) => document.activeElement === document.querySelector(field),
          FIELD,
        ),
      { timeout: 10_000, timeoutMsg: "the find bar opened without the keyboard in its field" },
    );
  });

  it("counts every match in any case, marks them, and is on the first", async () => {
    await typeInto(FIELD, "needle");

    await until("1 of 3", TEXT[0]);
    expect((await found()).marked).toBe(3);
  });

  it("goes to the next match, and from the last back to the first", async () => {
    await pressInTheBar("Next match");
    await until("2 of 3", TEXT[2]);
    await pressInTheBar("Next match");
    await until("3 of 3", TEXT[4]);
    await pressInTheBar("Next match");
    await until("1 of 3", TEXT[0]);
  });

  it("goes to the previous match, and from the first back to the last", async () => {
    await pressInTheBar("Previous match");
    await until("3 of 3", TEXT[4]);
    await pressInTheBar("Previous match");
    await until("2 of 3", TEXT[2]);
  });

  it("goes on at Enter and back at Shift+Enter in its field", async () => {
    await focusOn(FIELD);
    await keyWithFallback(
      ["Enter"],
      { key: "Enter" },
      async () => (await found()).count === "3 of 3",
    );
    await until("3 of 3", TEXT[4]);

    // Shift+Enter as a page keydown in the field: the driver's Shift is not held for the
    // Enter on every WebView, and its bare Enter would step on, not back.
    await browser.execute((field: string) => {
      document.querySelector(field)?.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Enter",
          shiftKey: true,
          bubbles: true,
          cancelable: true,
        }),
      );
    }, FIELD);
    await until("2 of 3", TEXT[2]);
  });

  it("says when nothing matches", async () => {
    await typeInto(FIELD, "haystack");

    await browser.waitUntil(async () => (await found()).count === "No matches", {
      timeout: 10_000,
      timeoutMsg: `the find bar never said No matches: ${JSON.stringify(await found())}`,
    });
    expect((await found()).marked).toBe(0);
  });

  it("closes at Escape, and leaves nothing marked", async () => {
    await typeInto(FIELD, "needle");
    await until("1 of 3", TEXT[0]);
    await focusOn(FIELD);

    await keyWithFallback(["Escape"], { key: "Escape" }, async () => !(await found()).open);

    await browser.waitUntil(async () => !(await found()).open, {
      timeout: 10_000,
      timeoutMsg: "Escape left the find bar open",
    });
    expect((await found()).marked).toBe(0);
  });
});
