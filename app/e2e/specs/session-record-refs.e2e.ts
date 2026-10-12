// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import {
  closeTab,
  focusWorkspace,
  press,
  projectRoot,
  tabTexts,
  untilInFront,
  untilSays,
} from "../acting.js";
import { showView } from "../opening.js";

/**
 * **A file a session record names opens from the record** (#984, #1043; fe3c4c3f), in the built
 * app against the real core.
 *
 * The spec writes a session record into `alpha`'s `sessions/`, as Smart close writes one, whose
 * text names `svc/README.md:1` and `npm test` in code spans. The Sessions view lists it, its row
 * opens the record's tab, the span that names a file of the workspace is a button there, and
 * pressing it opens that file in its branch's Files tab. `npm test` stays code, and the files
 * the record names are listed after it.
 *
 * Only the tab and the file's name are asserted: where the line lands and what the preview draws
 * are the file tab's own specs.
 *
 * **It leaves the project as it found it**: the record is removed (and the folder, where the
 * spec made it), and both tabs are closed.
 */

const TITLE = "VH names two files";
const FILE = "20261012-093000-vh-names-two-files.md";
const RECORD_TAB = `Session · ${TITLE}`;
const FILES_TAB = "Files · svc";
const RECORD = '[data-testid="session-record"]';

const TEXT = `---
title: ${TITLE}
date: 2026-10-12 09:30:00
chat: 9401
chat-name: vh record
persona: none
harness: claude
profile: scenario
conversation: unknown
workspace: alpha
cwd: workspaces/alpha
---

# ${TITLE}

## Goal

Read \`svc/README.md:1\` and leave \`npm test\` as it is.

## Done

Nothing.

## Decisions

Nothing.

## Open

Nothing.

## How to resume

Open the record.
`;

describe("a session record's files", function () {
  this.timeout(180_000);

  let sessions = "";
  let made = false;

  before(async () => {
    sessions = join(await projectRoot(), "workspaces", "alpha", "sessions");
    made = !existsSync(sessions);
    mkdirSync(sessions, { recursive: true });
    writeFileSync(join(sessions, FILE), TEXT);

    // Focused away and back, so the workspace's panels are read with the record on the disk
    // whether or not a watch has told the window of it yet.
    await focusWorkspace("beta");
    await focusWorkspace("alpha");
    await showView("Sessions");
    await untilSays('[data-testid="panel-sessions"]', TITLE);
    await press('[data-testid="panel-sessions"]', TITLE, "containing");
    await untilInFront(RECORD_TAB);
    await $(RECORD).waitForExist({ timeout: 20_000 });
  });

  after(async () => {
    for (const name of [FILES_TAB, RECORD_TAB])
      if ((await tabTexts()).includes(name)) await closeTab(name);
    rmSync(join(sessions, FILE), { force: true });
    if (made && existsSync(sessions) && readdirSync(sessions).length === 0)
      rmSync(sessions, { recursive: true, force: true });
    // The Panel's right half opens on Memory (#1738).
    await showView("Memory");
  });

  it("draws a span that names a file of the workspace as its button, and leaves others code", async () => {
    // A span becomes a button once the workspace's repos are read, after the record is drawn.
    await browser.waitUntil(
      async () =>
        browser.execute(
          (record: string) => document.querySelector(`${record} button.record-ref`) !== null,
          RECORD,
        ),
      { timeout: 20_000, interval: 250, timeoutMsg: "no span of the record became a button" },
    );
    const spans = await browser.execute(
      (record: string) => ({
        buttons: [...document.querySelectorAll(`${record} button.record-ref`)].map(
          (button) => button.getAttribute("aria-label") ?? "",
        ),
        code: [...document.querySelectorAll(`${record} code`)]
          .filter((code) => code.closest("button") === null)
          .map((code) => (code.textContent ?? "").trim()),
      }),
      RECORD,
    );

    expect(spans.buttons).toEqual(["Open svc/README.md:1"]);
    expect(spans.code).toContain("npm test");
    // And the record lists the files it names, after itself.
    await untilSays('section[aria-label="Files this record names"]', "svc/README.md:1");
  });

  it("opens the file it names in its branch's Files tab", async () => {
    await press(RECORD, "Open svc/README.md:1");

    await untilInFront(FILES_TAB);
    await $('section.piece-files-shown[aria-label="README.md"]').waitForExist({
      timeout: 20_000,
      timeoutMsg: "the Files tab never picked README.md",
    });
  });
});
