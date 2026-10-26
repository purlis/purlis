// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, $$, browser, expect } from "@wdio/globals";
import { harnessRowsDrawn, pressOnly } from "../opening.js";
import { ask } from "../switching.js";

/**
 * **A harness the project declares in `harnesses/<name>.toml`** (ADR 0073), in the built app:
 * the new-chat picker's approval shows every word the core's approval text names (#969, V66),
 * and Settings lists the declaration read-only with Open file (#1389).
 *
 * The approval is the picker's, as for a profile (`picker.e2e.ts`), and its words are the core's
 * (`profiletrust::shown`): the profile's line and kind, the declaration's file and whole digest,
 * its program, the words a new chat and a resumed chat add, its ACP agent's words, its `env`
 * namespace, and a warning beside a flag value that names a folder in the project. The digest is
 * worked out here from the bytes this spec wrote, so the dialog is held to the file and not to
 * itself.
 *
 * Nothing is approved and nothing starts: the approval is the person's click, and a declared
 * harness's program is not on this machine. What the dialog draws is the whole of #969: with the
 * words, the names of the declaration's `env` namespace.
 *
 * **It leaves the project as it found it**: one app process serves the whole run, and a
 * declaration left behind would be a row in every picker after this spec. The file goes, and the
 * folder too when this spec made it.
 */

const NAME = "e2e-declared";
const TITLE = "E2E declared";
const PROGRAM = "e2e-declared-harness";
const FILE = `harnesses/${NAME}.toml`;
/** The words the declaration adds, as the core quotes them (`profiles::quote`: braces are not
 *  in a shell's safe set). */
const NEW_WORDS = ["'--title={name}'", "--project=workspaces"];
const RESUME_WORDS = ["'--resume={id}'"];
/** The folder the fixture project has at its root, which `--project=workspaces` names. */
const WARNING =
  "--project=workspaces names the folder workspaces in this project; the program may read it";

const DECLARATION = [
  "# Written by the scenario tests (harness-declaration.e2e.ts), and removed by them.",
  `name = "${NAME}"`,
  `title = "${TITLE}"`,
  `program = "${PROGRAM}"`,
  'env = ["E2E_DECLARED_*"]',
  "",
  "[session]",
  'new = ["--title={name}", "--project=workspaces"]',
  'resume = ["--resume={id}"]',
  'named_by = ["--resume"]',
  "",
  "[levels]",
  `acp = ["${PROGRAM}", "acp"]`,
  "",
].join("\n");

const DIGEST = `sha256:${createHash("sha256").update(DECLARATION, "utf8").digest("hex")}`;

const DIALOG = '[role="dialog"][aria-labelledby="start-chat"]';
const TABS = '[data-strip="Tabs"]';
const PALETTE = '[role="dialog"][aria-label="Command palette"]';
const GROUPS = 'nav[aria-label="Groups"]';
const ROW = `[data-setting="project.harness.declared.${NAME}"]`;

/** Opens Settings at the Project level through the palette, as the operator does. */
async function openProjectSettings(): Promise<void> {
  await browser.keys(["F2"]);
  await $(PALETTE).waitForDisplayed({ timeout: 20_000 });
  const box = await $("#palette-query");
  await browser.waitUntil(async () => box.isFocused(), {
    timeout: 10_000,
    timeoutMsg: "the palette did not take the keyboard when it opened",
  });
  await box.addValue("Project settings");
  await browser.waitUntil(
    async () =>
      browser.execute(
        () =>
          document
            .querySelector('[role="option"][aria-selected="true"] .palette-title')
            ?.textContent?.startsWith("Project settings") ?? false,
      ),
    { timeout: 10_000, timeoutMsg: "the palette did not offer Project settings…" },
  );
  await browser.keys(["Enter"]);
  await $(GROUPS).waitForDisplayed({ timeout: 20_000 });
}

/** Closes the Settings tab, if one is open. */
async function closeSettings(): Promise<void> {
  const closer = await $(`${TABS} button[aria-label="Close Settings"]`);
  if (await closer.isExisting()) {
    await closer.click();
    await closer.waitForExist({ timeout: 20_000, reverse: true });
  }
}

/** Takes down whatever dialog is up, a failure's included. */
async function noDialog(): Promise<void> {
  for (let left = 0; left < 3; left++) {
    const up = await $('[role="dialog"]')
      .isExisting()
      .catch(() => false);
    if (!up) return;
    await browser.keys(["Escape"]);
  }
}

describe("a harness the project declares", function () {
  this.timeout(180_000);

  let root = "";
  let madeTheFolder = false;

  before(async () => {
    await $('button[aria-label="New tab"]').waitForClickable({ timeout: 60_000 });
    root = (await ask<string[]>("open_planes"))[0];
    const folder = join(root, "harnesses");
    madeTheFolder = !existsSync(folder);
    mkdirSync(folder, { recursive: true });
    writeFileSync(join(root, FILE), DECLARATION);
    // Settings reads the declarations when it opens: a tab an earlier spec left open would be
    // drawing what it read before the file was here.
    await closeSettings();
  });

  after(async () => {
    await noDialog();
    await closeSettings();
    if (root === "") return;
    rmSync(join(root, FILE), { force: true });
    const folder = join(root, "harnesses");
    if (madeTheFolder && existsSync(folder) && readdirSync(folder).length === 0)
      rmSync(folder, { recursive: true, force: true });
  });

  it("asks before running it, showing every word that would run (#969)", async () => {
    const panes = (await $$('[data-testid="pane"]').getElements()).length;
    await pressOnly("New tab");
    await harnessRowsDrawn();
    // The declaration is a profile named after it, picked by its row's words.
    const row = await $(`label*=${NAME}`);
    await row.waitForExist({
      timeout: 20_000,
      timeoutMsg: `the picker drew no row for the declared harness ${NAME}`,
    });
    await row.click();

    const approve = await $("button=Approve and start");
    await approve.waitForDisplayed({
      timeout: 20_000,
      timeoutMsg: "picking the declared harness did not ask for an approval",
    });

    // The approval's sentence, and the words in it: the core's, as one line of text.
    const sentence = await $(`${DIALOG} p.approve`);
    await expect(sentence).toHaveText(
      expect.stringContaining("purlis has not run this profile before. It would run:"),
    );
    const shown = await browser.execute(
      (dialog: string) =>
        document.querySelector(`${dialog} p.approve code`)?.textContent?.trim() ?? "",
      DIALOG,
    );

    const whole =
      /^(.*) \(kind (.*), declared in (.*), (sha256:[0-9a-f]+); program (.*); a new chat adds (.*); a resumed chat adds (.*); its ACP agent runs (.*); it passes (.*) from your environment; warning: (.*)\)$/.exec(
        shown,
      );
    if (!whole) throw new Error(`the approval did not say every field, in order: ${shown}`);
    const [, line, kind, file, digest, program, fresh, resumed, acp, passed, warning] = whole;
    // What runs: the profile's line is the program, and the program is the declaration's.
    expect(line).toBe(PROGRAM);
    expect(program).toBe(PROGRAM);
    expect(kind).toBe(NAME);
    // Which file, and exactly which bytes of it: the digest of what this spec wrote.
    expect(file).toBe(FILE);
    expect(digest).toBe(DIGEST);
    // Every word a chat is started with, new and resumed, and what its ACP agent runs.
    expect(fresh.split(" ")).toEqual(NEW_WORDS);
    expect(resumed.split(" ")).toEqual(RESUME_WORDS);
    expect(acp.split(" ")).toEqual([PROGRAM, "acp"]);
    // What of the operator's environment the declaration's namespace reaches (#969).
    expect(passed.split(" ")).toEqual(["E2E_DECLARED_*"]);
    // And the residual the approval covers, said beside it.
    expect(warning).toBe(WARNING);

    // The row says the same line, and that it is new to this machine.
    const meta = await browser.execute(
      (dialog: string, name: string) => {
        // The row is an option card (#2907): named by its title, its radio inside it.
        const label = [...document.querySelectorAll(`${dialog} label`)].find(
          (one) => (one.querySelector('[data-part="title"]') ?? one).textContent?.trim() === name,
        );
        const control = label?.querySelector("input") ?? null;
        const says = (control?.getAttribute("aria-describedby") ?? "")
          .split(" ")
          .map((id) => document.getElementById(id))
          .filter((one) => one !== null);
        return {
          said: says.map((one) => one.textContent ?? "").join(" "),
          approval: says
            .map((one) => one.querySelector(".needs-approval")?.textContent?.trim())
            .find((one) => one !== undefined),
        };
      },
      DIALOG,
      NAME,
    );
    expect(meta.said).toContain(shown);
    expect(meta.approval).toBe("new");

    // Escaped, nothing ran and nothing was approved.
    await browser.keys(["Escape"]);
    await $(DIALOG).waitForExist({ timeout: 20_000, reverse: true });
    expect(await $$('[data-testid="pane"]').getElements()).toHaveLength(panes);
    // The record is in the project's state folder under either name (`.purlis` or the older
    // `.charter`), so both are read: one that is not there holds no approval.
    for (const state of [".purlis", ".charter"]) {
      const approved = join(root, state, "harness-declarations-approved.json");
      expect(existsSync(approved) ? readFileSync(approved, "utf8") : "").not.toContain(NAME);
    }
  });

  it("is listed in Settings read-only, with Open file (#1389)", async () => {
    await openProjectSettings();
    await $(GROUPS).$("button=Harness & profiles").click();

    const row = await $(ROW);
    await row.waitForDisplayed({
      timeout: 20_000,
      timeoutMsg: `Settings › Harness & profiles drew no row for ${FILE}`,
    });
    const said = await row.getText();
    expect(said).toContain(`${TITLE} (${NAME})`);
    expect(said).toContain(FILE);
    expect(said).toContain(`it starts ${PROGRAM}`);
    expect(said).toContain("can run as an ACP agent");
    expect(said).toContain("Read-only here");

    // Read-only: its one control is Open file, and nothing in it takes a value.
    const controls = await browser.execute(
      (selector: string) =>
        [
          ...(document
            .querySelector(selector)
            ?.querySelectorAll(
              'button, input, select, textarea, [contenteditable="true"], [role="radio"], [role="checkbox"], [role="switch"], [role="combobox"], [role="textbox"], [role="slider"]',
            ) ?? []),
        ].map((one) => `${one.tagName.toLowerCase()} ${(one.textContent ?? "").trim()}`),
      ROW,
    );
    expect(controls).toEqual(["button Open file"]);
  });
});
