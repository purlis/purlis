// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createHash } from "node:crypto";
import { readFileSync, readdirSync, realpathSync, renameSync, writeFileSync } from "node:fs";
import { dirname, join, relative, sep } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { anEmptyRecord, copyFixturePlane } from "../harness.js";
import { ask } from "../switching.js";
import { openThroughTheTrustQuestion } from "../opening.js";
import { tooltipOf } from "../tooltips.js";

/**
 * **A project this purlis opens read-only says why, and the window writes nothing to it**
 * (#826, FR-24), in the built app against the real core: a project whose manifest requires a
 * feature no purlis has, opened through the opener and the trust gate as a person opens one.
 *
 * What only a real window can say: the project's tab draws the core's sentence (which feature
 * is missing, which version has it), a button that would write a committed project file is off
 * with that sentence as its reason, and after the window has drawn and settled every committed
 * file is byte for byte what it was. `ReadOnlyLine.test.tsx` and `actions.test.ts` hold the
 * pieces against a mocked side; `compat.rs` and the hook tests hold the core's refusal.
 *
 * **Its own project, copied into the run's tree**, and let go of at the end: one app process
 * serves the whole run.
 */

const PROJECTS = '[data-strip="Projects"]';
const WORKSPACES = '[data-strip="Workspaces"]';
const LINE = '[role="note"][aria-label="Read-only project"]';

/** The feature the project requires: a name no purlis will ever have. */
const FEATURE = "e2e-from-the-future";
/** The version the manifest says has it, which the sentence names. */
const SINCE = "99.0.0";

/** This spec's own project: the fixture, renamed, with a manifest that requires `FEATURE`. */
const mine = (() => {
  const copied = copyFixturePlane();
  const renamed = join(dirname(copied), "read-only");
  renameSync(copied, renamed);
  const root = realpathSync(renamed);
  const manifest = join(root, "charter.toml");
  const said = readFileSync(manifest, "utf8");
  // `requires` takes effect with `schema = 2` (V37a); an entry is `{ feature, since }` (V37b).
  writeFileSync(
    manifest,
    said.replace(
      /^schema = 1$/m,
      `schema = 2\nrequires = [{ feature = "${FEATURE}", since = "${SINCE}" }]`,
    ),
  );
  // Nothing to put back, so opening it starts no chat.
  anEmptyRecord(root);
  return root;
})();

/** The LIVE workspace's files the fixture's `.gitignore` publishes; every other one is the clone's. */
const PUBLISHED_IN_A_WORKSPACE = ["workspace.json", "workspace.md", "memory", "todos", "changes"];

/**
 * Whether `rel` is a committed project file, by the fixture's own `.gitignore`: not `.charter/`
 * (clone state and machine stores, which a read-only project still writes), not this machine's
 * local files, and in `workspaces/` only what the LIVE workspace publishes.
 */
function committed(rel: string): boolean {
  const parts = rel.split(sep);
  if (parts[0] === ".charter" || parts[0] === ".purlis") return false;
  if (rel === "charter.local.toml" || rel === join(".claude", "settings.local.json")) return false;
  if (parts[0] !== "workspaces" || parts.length < 3) return true;
  if (parts[1] !== "alpha" || !PUBLISHED_IN_A_WORKSPACE.includes(parts[2])) return false;
  return !(parts[2] === "changes" && parts[3] === "log");
}

/** Every committed file of the project, each with a hash of its bytes, sorted by path. */
function committedFiles(root: string): Record<string, string> {
  const files: Record<string, string> = {};
  const walk = (dir: string) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      const rel = relative(root, path);
      if (entry.isDirectory()) walk(path);
      else if (entry.isFile() && committed(rel))
        files[rel] = createHash("sha256").update(readFileSync(path)).digest("hex");
    }
  };
  walk(root);
  return Object.fromEntries(Object.entries(files).sort(([a], [b]) => a.localeCompare(b)));
}

describe("a project this purlis opens read-only", function () {
  this.timeout(180_000);

  /** The project the launch opened, which this spec never touches. */
  let first = "";
  /** The committed files before the window opened the project. */
  let found: Record<string, string> = {};

  before(async () => {
    first = (await ask<string[]>("open_planes"))[0];
    found = committedFiles(mine);
    // Through the opener, as a person opens one: the read-only project still goes through
    // the trust gate (ADR 0035).
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
    await $(`${WORKSPACES} [role="tab"].plane-root`).waitForExist({ timeout: 30_000 });
  });

  after(async () => {
    // Let go of it in the core: nothing of the project on disk goes, and the next spec finds
    // the window holding the launch's project alone.
    for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== first))
      await ask("close_plane", { plane });
  });

  it("says once, on the project's tab, which feature is missing and which version has it", async () => {
    const line = await $(LINE);
    await line.waitForExist({ timeout: 30_000 });
    const said = await line.getText();
    expect(said).toContain(FEATURE);
    expect(said).toContain(SINCE);
    // Once: not again per workspace, pane or panel.
    expect(
      await browser.execute((selector: string) => document.querySelectorAll(selector).length, LINE),
    ).toBe(1);
  });

  it("turns off a button that would write a committed file, with that sentence", async () => {
    const said = await $(LINE).getText();
    // The workspace strip's `+`, read in one pass: the catalogue's `workspace.create`.
    const plus = await browser.execute(() => {
      const button = document.querySelector<HTMLButtonElement>(
        '.workspaces .strip-doing button[aria-label="New workspace…"]',
      );
      // Unavailable, and kept in reach so its reason can be read (#2908).
      return button === null
        ? null
        : { disabled: button.disabled || button.getAttribute("aria-disabled") === "true" };
    });
    expect(plus).not.toBeNull();
    expect(plus?.disabled).toBe(true);
    // Its tooltip says its name and then why; why is its description for good (#2908).
    const tip = await tooltipOf('.workspaces .strip-doing button[aria-label="New workspace…"]');
    expect(tip).toContain(FEATURE);
    const why = await browser.execute(() => {
      const button = document.querySelector<HTMLButtonElement>(
        '.workspaces .strip-doing button[aria-label="New workspace…"]',
      );
      const id = button?.getAttribute("aria-describedby") ?? "";
      return id === "" ? null : (document.getElementById(id)?.textContent ?? null);
    });
    expect(why).toContain(FEATURE);
    expect(said).toContain(why ?? "(no description)");
  });

  it("writes no committed project file", async () => {
    // Long enough for the window's reads after an open (the panels, the sidebar, the hooks'
    // wiring) to have landed, which is when a writer that ignored the rule would have written.
    await browser.pause(3_000);
    expect(committedFiles(mine)).toEqual(found);
  });
});
