// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { anAnswerIsWaiting, noAnswerWaits, theNextFolderPickAnswers } from "../dialogs.js";
import { THE_RUNS_TREE } from "../harness.js";
import { runFromThePalette } from "../reach.js";
import { ask } from "../switching.js";

/**
 * **The Extensions dialog, in the built app against the real core** (6f4401fb, 272e21ab): it lists
 * purlis's own themes, says an installed extension's trouble on its row as a polite live region
 * (`role="status"`), and offers Read again when this machine's extension record cannot be read,
 * which reads it again in place once it is mended.
 *
 * - **Opened from its palette row** (`extensions.show`), as a person opens it.
 * - **An extension of the spec's own**, added through Add an extension… with the folder the e2e
 *   build's dialog seam answers (`dialogs.ts`), left unapproved (Cancel), and then broken on disk
 *   so the core refuses its manifest: the row says so.
 * - **The record made unreadable for a moment**: its bytes kept, replaced with text that is not
 *   JSON, and put back before Read again is pressed.
 * - **Open in your editor is never pressed**: it starts a program outside the app.
 *
 * `Extensions.test.tsx` holds each line against a mocked core; what only the real window can say
 * is that the core's answers reach these places.
 *
 * **Nothing left behind**: the record's bytes are put back whatever happened, the extension is
 * removed from the record and its folder deleted, an unused folder answer is taken back, and the
 * dialog is closed by its Done.
 */

const DIALOG = '[role="dialog"].extensions';
const APPROVE = '[role="dialog"][aria-labelledby="approve-extension"]';

/** The id and name the spec's extension declares: nothing else in a run is called this. */
const ID = "e2e-extensions-dialog";
const NAME = "E2E extensions dialog";

/** The spec's extension: a manifest and the one theme it contributes, in the run's tree. */
const folder = (() => {
  const at = join(mkdtempSync(join(THE_RUNS_TREE, "extension-")), "ext");
  mkdirSync(at, { recursive: true });
  return at;
})();
// The manifest's name the core reads (`extension::MANIFEST`, names.rs `EXTENSION_MANIFEST`):
// still its first spelling, which every shipped extension uses.
const manifest = join(folder, "charter-extension.json");

/** A manifest this purlis reads: one theme, nothing else. */
const READABLE = JSON.stringify({
  version: 1,
  id: ID,
  name: NAME,
  contributes: { themes: [{ name: "E2E Dark", file: "dark.json" }] },
});
/** The same manifest asking for a capability no purlis has: the core refuses the whole of it. */
const REFUSED = JSON.stringify({
  version: 1,
  id: ID,
  name: NAME,
  capabilities: ["e2e-no-such-capability"],
  contributes: { themes: [{ name: "E2E Dark", file: "dark.json" }] },
});

writeFileSync(manifest, READABLE);
writeFileSync(join(folder, "dark.json"), "{}");

/** The rows the dialog lists, by name, with what each says as a live region; one pass. */
type Row = { name: string; statuses: string[] };
async function listed(): Promise<Row[]> {
  return browser.execute((dialog: string) => {
    const rows = document.querySelectorAll<HTMLElement>(`${dialog} .extension-rows > li`);
    return [...rows].map((row) => ({
      name: row.querySelector(".name")?.textContent?.trim() ?? "",
      statuses: [...row.querySelectorAll<HTMLElement>('[role="status"]')].map((one) =>
        one.innerText.trim(),
      ),
    }));
  }, DIALOG);
}

/** Presses the dialog's button whose words are `words`, from the page. */
async function pressIn(scope: string, words: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute(
        (within: string, said: string) => {
          const button = [...document.querySelectorAll<HTMLElement>(`${within} button`)].find(
            (one) => one.textContent?.trim() === said && !one.hasAttribute("disabled"),
          );
          button?.click();
          return button !== undefined;
        },
        scope,
        words,
      ),
    { timeout: 20_000, interval: 200, timeoutMsg: `${scope} offered no ${words}` },
  );
}

/** Opens the dialog from the palette and waits for its first read to land. */
async function openTheDialog(): Promise<void> {
  await runFromThePalette("Extensions", "extensions.show");
  await $(DIALOG).waitForDisplayed({ timeout: 20_000 });
  await browser.waitUntil(
    async () =>
      browser.execute(
        (dialog: string) => document.querySelector(`${dialog} .pending`) === null,
        DIALOG,
      ),
    { timeout: 30_000, timeoutMsg: "the dialog stayed on Reading the installed extensions…" },
  );
}

/** Closes the dialog by its Done, where it is open. */
async function closeTheDialog(): Promise<void> {
  if (!(await $(DIALOG).isExisting())) return;
  await pressIn(DIALOG, "Done");
  await $(DIALOG).waitForExist({ reverse: true, timeout: 10_000 });
}

/**
 * This machine's extension record: the one `extensions.json` in the run's config homes that
 * names the spec's extension, which only the app's own can, since the app wrote it there.
 */
function theRecord(): string | undefined {
  const found: string[] = [];
  for (const home of readdirSync(THE_RUNS_TREE).filter((one) => one.startsWith("config-")))
    for (const dir of readdirSync(join(THE_RUNS_TREE, home))) {
      const at = join(THE_RUNS_TREE, home, dir, "extensions.json");
      if (existsSync(at) && statSync(at).isFile() && readFileSync(at, "utf8").includes(ID))
        found.push(at);
    }
  return found.length === 1 ? found[0] : undefined;
}

describe("the Extensions dialog", function () {
  this.timeout(180_000);

  /** The record's own bytes while the spec has replaced them, to put back. */
  let kept: { at: string; bytes: Buffer } | undefined;

  before(async () => {
    await openTheDialog();
  });

  after(async () => {
    noAnswerWaits();
    if (kept !== undefined) writeFileSync(kept.at, kept.bytes);
    await closeTheDialog();
    // The record as it was found: the spec's extension is not in it.
    await ask("forget_extension", { id: ID }).catch(() => undefined);
    rmSync(folder, { recursive: true, force: true });
  });

  it("lists purlis's own themes", async () => {
    const themes = await browser.execute((dialog: string) => {
      const heading = [...document.querySelectorAll(`${dialog} h3`)].find((one) =>
        (one.textContent ?? "").startsWith("purlis"),
      );
      const list = heading?.nextElementSibling;
      return [...(list?.querySelectorAll("li code") ?? [])].map((one) => one.textContent ?? "");
    }, DIALOG);
    expect(themes.length).toBeGreaterThan(0);
    expect(themes.every((name) => name.trim() !== "")).toBe(true);
  });

  it("adds an extension from the folder picked, and says its row's trouble as a live region", async () => {
    theNextFolderPickAnswers(folder);
    await pressIn(DIALOG, "Add an extension…");
    // The question about it: left unanswered, so nothing it declares is put in force.
    await $(APPROVE)
      .waitForDisplayed({ timeout: 30_000 })
      .catch(async () => {
        throw new Error(
          `no question about the extension came; the folder pick's answer was ${anAnswerIsWaiting() ? "never read" : "read"}; the dialog says: ${await dialogSays()}`,
        );
      });
    await pressIn(APPROVE, "Cancel");
    await $(APPROVE).waitForExist({ reverse: true, timeout: 10_000 });
    await browser.waitUntil(async () => (await listed()).some((row) => row.name === NAME), {
      timeout: 20_000,
      timeoutMsg: "the extension added was never listed",
    });
    expect((await listed()).find((row) => row.name === NAME)?.statuses).toEqual([]);

    // Its manifest now asks for what no purlis has: the next read refuses it, on its row.
    writeFileSync(manifest, REFUSED);
    await closeTheDialog();
    await openTheDialog();
    let row: Row | undefined;
    await browser
      .waitUntil(
        async () => {
          // A row purlis could not read is named by its id: there is no manifest to name it.
          row = (await listed()).find(
            (one) => (one.name === ID || one.name === NAME) && one.statuses.length > 0,
          );
          return row !== undefined;
        },
        { timeout: 20_000 },
      )
      .catch(async () => {
        throw new Error(`no row said its trouble; the dialog says: ${await dialogSays()}`);
      });
    expect(row?.statuses.join(" ")).toContain("e2e-no-such-capability");
    writeFileSync(manifest, READABLE);
  });

  it("offers Read again for a record it cannot read, and reads it again in place", async () => {
    const at = theRecord();
    if (at === undefined)
      throw new Error(`no one extension record under ${THE_RUNS_TREE} names ${ID}`);
    kept = { at, bytes: readFileSync(at) };
    writeFileSync(at, "this is not JSON {");

    await closeTheDialog();
    await openTheDialog();
    await browser
      .waitUntil(
        async () =>
          browser.execute(
            (dialog: string) =>
              (document.querySelector(`${dialog} .came-back`)?.textContent ?? "").includes(
                "could not read this machine",
              ),
            DIALOG,
          ),
        { timeout: 20_000 },
      )
      .catch(async () => {
        throw new Error(
          `the unreadable record was never said; the dialog says: ${await dialogSays()}`,
        );
      });
    // Its two ways out, under the sentence. Open in your editor starts a program: never pressed.
    const offered = await browser.execute(
      (dialog: string) =>
        [...document.querySelectorAll(`${dialog} button`)].map((one) => one.textContent?.trim()),
      DIALOG,
    );
    expect(offered).toContain("Read again");
    expect(offered).toContain("Open in your editor");

    // Mended, then read again in place: the sentence goes and the row is back.
    writeFileSync(at, kept.bytes);
    kept = undefined;
    await pressIn(DIALOG, "Read again");
    await browser.waitUntil(
      async () =>
        browser.execute(
          (dialog: string) =>
            !(document.querySelector(`${dialog} .came-back`)?.textContent ?? "").includes(
              "could not read this machine",
            ),
          DIALOG,
        ),
      { timeout: 20_000, timeoutMsg: "Read again left the record said unreadable" },
    );
    await browser.waitUntil(async () => (await listed()).some((row) => row.name === NAME), {
      timeout: 20_000,
      timeoutMsg: "Read again did not list the extension again",
    });
    expect(await $(DIALOG).isDisplayed()).toBe(true);
  });
});

/** What the dialog says, for a failure's message. */
async function dialogSays(): Promise<string> {
  return browser.execute(
    (dialog: string) => document.querySelector<HTMLElement>(dialog)?.innerText.trim() ?? "(closed)",
    DIALOG,
  );
}
