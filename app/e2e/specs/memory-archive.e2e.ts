// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import {
  closeTab,
  press,
  projectRoot,
  tabTexts,
  textOf,
  typeInto,
  untilInFront,
  untilSays,
} from "../acting.js";
import { showView } from "../opening.js";

/**
 * **A long archive, in its own tab: searched, paged, and a numbered name restored under its
 * own** (KN-4, #1191; 849ddb97, b44618fa), in the built app against the real core.
 *
 * The spec writes sixteen archived memories into `steward`'s archive before it opens it, more
 * than one page of twelve (`PanelList.PAGE`): fourteen plain ones, and `freeze` with the
 * `freeze-2` that archiving a second `freeze` would have made. The tab, opened from the persona's
 * own heading, draws one page and *Show 4 more*; its search box finds `freeze-2` by a word of
 * its text, with the part that matched under the row; and the chosen `freeze-2` offers
 * *Restore as freeze* beside *Restore memory*, which puts it back as `freeze.md`.
 *
 * **It leaves the project as it found it**: every file it wrote is removed, the memory it
 * restored is taken out again, steward's index is put back as it was, and both tabs are closed.
 */

const PERSONA = "steward";
const TAB = `Archived memory · ${PERSONA}`;
const PANE = `section.view-pane[aria-label="${TAB}"]`;
const ROWS = `${PANE} [role="listbox"][aria-label="Archived memories in ${PERSONA}"] [data-part="root"]`;
const SEARCH = `${PANE} input[aria-label="Search archived memories in ${PERSONA}"]`;
const PLAIN = Array.from({ length: 14 }, (_, n) => `vh-archived-${String(n + 1).padStart(2, "0")}`);

/** An archived memory's file, as the store writes one. */
function memoryText(title: string, stamp: string, body: string): string {
  return `# ${title}\n\n_${stamp} · persistent_\n\n${body}\n`;
}

describe("a store's archive, long", function () {
  this.timeout(180_000);

  let project = "";
  const store = () => join(project, "personas", PERSONA, "memory");
  const archive = () => join(store(), "archive");
  let written: string[] = [];
  let hadArchive = false;
  let index: string | undefined;

  /** The archived rows the tab draws, by their words, read in one pass. */
  const rows = () =>
    browser.execute(
      (css: string) =>
        [...document.querySelectorAll(css)].map((row) => (row.textContent ?? "").trim()),
      ROWS,
    );

  before(async () => {
    project = await projectRoot();
    hadArchive = existsSync(archive());
    const was = join(store(), "MEMORY.md");
    index = existsSync(was) ? readFileSync(was, "utf8") : undefined;
    mkdirSync(archive(), { recursive: true });
    const files: [string, string][] = [
      ...PLAIN.map((slug, n): [string, string] => [
        slug,
        memoryText(
          `Archived note ${n + 1}`,
          `2026-09-${String(n + 1).padStart(2, "0")} 10:00`,
          `Note ${n + 1} of the long archive.`,
        ),
      ]),
      [
        "freeze",
        memoryText("Deploys freeze", "2026-09-20 09:00", "Nothing ships after Thursday noon."),
      ],
      [
        "freeze-2",
        memoryText(
          "Deploys freeze",
          "2026-09-21 09:00",
          "The second freeze holds through the quarter's last week.",
        ),
      ],
    ];
    for (const [slug, text] of files) writeFileSync(join(archive(), `${slug}.md`), text);
    written = files.map(([slug]) => `${slug}.md`);

    // Personas is a view of the right side (#1678); the persona's own tab carries its archive.
    await showView("Personas");
    await untilSays('[data-testid="panel-personas"]', PERSONA);
    await press('[data-testid="panel-personas"]', PERSONA, "containing");
    // The heading's archive button is a glyph, named by its row's words.
    await press(`[data-testid="view-pane-charter-persona-${PERSONA}"]`, "Open steward's archive");
    await untilInFront(TAB);
    await $(ROWS).waitForExist({ timeout: 20_000 });
  });

  after(async () => {
    for (const name of [TAB, PERSONA]) if ((await tabTexts()).includes(name)) await closeTab(name);
    for (const name of written) rmSync(join(archive(), name), { force: true });
    rmSync(join(store(), "freeze.md"), { force: true });
    if (!hadArchive && existsSync(archive()) && readdirSync(archive()).length === 0)
      rmSync(archive(), { recursive: true, force: true });
    const now = join(store(), "MEMORY.md");
    if (index === undefined) rmSync(now, { force: true });
    else writeFileSync(now, index);
    await showView("Inbox");
  });

  it("draws one page of a long archive, and Show more draws the rest", async () => {
    expect(await rows()).toHaveLength(12);
    expect(await textOf(`${PANE} button.panel-more`)).toMatch(/^Show 4 more\s*· 4 left$/);

    await press(PANE, "Show 4 more", "starting");

    await browser.waitUntil(async () => (await rows()).length === 16, {
      timeout: 10_000,
      timeoutMsg: "Show more never drew the archive's last four",
    });
    expect(await $(`${PANE} button.panel-more`).isExisting()).toBe(false);
  });

  it("finds an archived memory by a word of its text, and shows the part that matched", async () => {
    await typeInto(SEARCH, "quarter");

    await browser.waitUntil(async () => (await rows()).length === 1, {
      timeout: 10_000,
      timeoutMsg: "the search never narrowed the archive to the one match",
    });
    expect((await rows())[0]).toContain("Deploys freeze");
    expect(await textOf(`${PANE} [data-testid="row-snippet-freeze-2"] mark`)).toBe("quarter");
  });

  it("offers a numbered name its own, and restores it under it", async () => {
    await press(ROWS, "Deploys freeze", "starting");
    await untilSays(`${PANE} [data-testid="archived-body"]`, "quarter's last week");

    await press(PANE, "Restore as freeze");

    await untilSays(
      `${PANE} p[role="status"]`,
      "Deploys freeze is back in steward's memory as freeze.",
    );
    expect(readFileSync(join(store(), "freeze.md"), "utf8")).toContain("quarter's last week");
    expect(existsSync(join(archive(), "freeze-2.md"))).toBe(false);
    // The un-numbered one it was offered beside stays archived, untouched.
    expect(readFileSync(join(archive(), "freeze.md"), "utf8")).toContain("Thursday noon");
  });
});
