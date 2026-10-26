// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createHash } from "node:crypto";
import { readFileSync, readdirSync, realpathSync, renameSync, writeFileSync } from "node:fs";
import { dirname, join, relative, sep } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { anEmptyRecord, copyFixturePlane } from "../harness.js";
import { PROJECTS, ask, inFront, openFromTheStrip } from "../switching.js";
import { tooltipOf } from "../tooltips.js";

/**
 * **Settings › Project in a read-only project: a control is off or on by the file it writes**
 * (#1759, train 74), in the built app against the real core: a project whose manifest requires a
 * feature no purlis has, opened through the opener and the trust gate as a person opens one.
 *
 * - A row that writes the project's shared file is off, inside a disabled fieldset that carries
 *   the project's sentence as its reason: every row of General, and the Default harness.
 * - A row kept in this machine's own file is on: the Default profile, and the environment passed
 *   to chats.
 * - A row that says it writes nothing, this machine only, or a committed file by some of its
 *   presses alone (`LiveSetting.writes`) is on (#1795): the Network page's rows and the Dispatch
 *   page's grants and locks. Each press of a committed file inside one is off by itself, which
 *   `GrantedList` and `DispatchGrants`' own tests hold.
 * - Opening and reading Settings writes no committed file.
 *
 * Only what holds whichever way the rows that move between the files are drawn is asserted here:
 * `SettingsTab.test.tsx` and `ProjectLevel.test.tsx` hold each kind of row against a mocked side.
 *
 * **Its own project, copied into the run's tree**, and let go of at the end, with its Settings tab
 * closed by its own × and the launch's project back in front: one app process serves the run.
 */

const TABS = '[data-strip="Tabs"]';
const LEVEL = '[role="radiogroup"][aria-label="Level"]';
const GROUPS = 'nav[aria-label="Groups"]';

/**
 * General's rows that only the shared file keeps (`fileControls.ts` SHARED, none of them a key
 * either file may hold): never movable, so off however a movable row comes to be drawn.
 */
const SHARED_ONLY = ["Default workspace", "Default persona", "Update channel", "Version lock"];

/**
 * The Network page's rows, each of which says what it writes (`GrantedList.tsx` `networkGroup`):
 * nothing, this machine's file, or the committed file by some presses only. None is off whole.
 */
const NETWORK_ON = [
  "Open hosts",
  "Allowed",
  "Blocked lately",
  "Folders a block's Allow may name",
  "Who may dispatch to whom",
];

/** The Dispatch page's rows that say what they write (`dispatch.tsx` `dispatchGroup`). */
const DISPATCH_ON = ["Dispatch grants", "Policy locks"];

/** The feature the project requires: a name no purlis will ever have. */
const FEATURE = "e2e-settings-from-the-future";
/** The version the manifest says has it, which the sentence names. */
const SINCE = "99.0.0";

/** This spec's own project: the fixture, renamed, with a manifest that requires `FEATURE`. */
const mine = (() => {
  const copied = copyFixturePlane();
  const renamed = join(dirname(copied), "read-only-settings");
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
  anEmptyRecord(root);
  return root;
})();

/** The LIVE workspace's files the fixture's `.gitignore` publishes; every other one is the clone's. */
const PUBLISHED_IN_A_WORKSPACE = ["workspace.json", "workspace.md", "memory", "todos", "changes"];

/** Whether `rel` is a committed project file, by the fixture's own `.gitignore` (as
 *  `read-only-project.e2e.ts` reads it). */
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

/** A row as the window draws it: whether it is off, the reason its fieldset gives, and whether
 *  its own control is disabled. */
type Row = { off: boolean; reason: string; controlOff: boolean | null };

/** Every row of the group on screen, by its label, read in one pass. */
async function rows(): Promise<Record<string, Row>> {
  return browser.execute(() => {
    const out: Record<string, { off: boolean; reason: string; controlOff: boolean | null }> = {};
    /** What describes `element`, as a screen reader hears it after its name. */
    const description = (element: Element | null) =>
      (element?.getAttribute("aria-describedby") ?? "")
        .split(/\s+/)
        .map((id) => (id === "" ? "" : (document.getElementById(id)?.textContent ?? "")))
        .join(" ")
        .trim();
    for (const row of document.querySelectorAll<HTMLElement>(".ui-setting-row")) {
      const label = row.querySelector(".ui-setting-label")?.textContent?.trim() ?? "";
      if (label === "" || label in out) continue;
      const fieldset = row.closest<HTMLFieldSetElement>("fieldset.read-only-off");
      const control = row.querySelector<HTMLElement>(
        ".ui-setting-control input, .ui-setting-control select, .ui-setting-control textarea, .ui-setting-control button",
      );
      out[label] = {
        off: fieldset?.disabled ?? false,
        // Why it is off: its group's description, which every control in it carries too (#2908).
        reason: description(fieldset),
        controlOff: control === null ? null : control.matches(":disabled"),
      };
    }
    return out;
  });
}

/** Opens the group called `label` from the nav, and waits for its rows. */
async function openGroup(label: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute(
        (nav: string, wanted: string) => {
          const button = [...document.querySelectorAll<HTMLElement>(`${nav} button`)].find(
            (one) => one.textContent?.trim() === wanted,
          );
          // The chosen group says so with `aria-current` (`SettingsLayout.tsx`).
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

/** Waits until `label`'s row is drawn in the group on screen, and answers how it is drawn. */
async function rowCalled(label: string): Promise<Row> {
  let found: Row | undefined;
  await browser
    .waitUntil(
      async () => {
        found = (await rows())[label];
        return found !== undefined;
      },
      { timeout: 20_000, interval: 250 },
    )
    .catch(async () => {
      throw new Error(`no row ${label}; the group draws ${Object.keys(await rows()).join(", ")}`);
    });
  return found as Row;
}

describe("Settings › Project in a read-only project", function () {
  this.timeout(180_000);

  /** The project the launch opened, which this spec never touches. */
  let first = "";
  /** The committed files before the window opened the project. */
  let found: Record<string, string> = {};

  before(async () => {
    first = (await ask<string[]>("open_planes"))[0];
    found = committedFiles(mine);
    await openFromTheStrip(mine);

    // Settings, for the project in front, at the Project level.
    await browser.executeAsync((done: () => void) => {
      void window.__TAURI__.event.emit("settings-asked").then(done, done);
    });
    await $(LEVEL).waitForExist({ timeout: 30_000 });
    await browser.waitUntil(
      async () =>
        browser.execute((selector: string) => {
          const radios = [
            ...document.querySelectorAll<HTMLElement>(`${selector} input[type="radio"]`),
          ];
          const project = radios.find(
            (one) => one.closest("label")?.textContent?.trim() === "Project",
          );
          if ((project as HTMLInputElement | undefined)?.checked === true) return true;
          project?.click();
          return false;
        }, LEVEL),
      { timeout: 30_000, interval: 250, timeoutMsg: "Settings never came to the Project level" },
    );
  });

  after(async () => {
    await browser.execute((strip: string) => {
      for (const closer of document.querySelectorAll<HTMLElement>(
        `${strip} button[aria-label^="Close Settings"]`,
      ))
        closer.click();
    }, TABS);
    for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== first))
      await ask("close_plane", { plane });
    // The launch's project in front again, from its own tab.
    await browser.waitUntil(
      async () => {
        if ((await inFront()) === first) return true;
        await browser.execute(
          (strip: string, path: string) =>
            [...document.querySelectorAll<HTMLElement>(`${strip} [role="tab"]`)]
              .find((tab) => tab.getAttribute("data-directory") === path)
              ?.click(),
          PROJECTS,
          first,
        );
        return false;
      },
      { timeout: 20_000, interval: 250, timeoutMsg: "the launch's project never came back" },
    );
  });

  it("turns off General's rows, which only the shared file keeps, with the project's sentence", async () => {
    await openGroup("General");
    // The keyboard reaches the sentence: the off group's "Read-only" takes it, and its tooltip
    // says it (#2908).
    expect(await tooltipOf("fieldset.read-only-off > .read-only-why > span")).toContain(FEATURE);
    for (const label of SHARED_ONLY) {
      const row = await rowCalled(label);
      expect({ label, off: row.off }).toEqual({ label, off: true });
      expect(row.reason).toContain(FEATURE);
      if (row.controlOff !== null)
        expect({ label, controlOff: row.controlOff }).toEqual({ label, controlOff: true });
    }
  });

  it("keeps the Default profile and the environment passed to chats on, and the Default harness off", async () => {
    await openGroup("Harness & profiles");
    const harness = await rowCalled("Default harness");
    expect(harness.off).toBe(true);
    expect(harness.reason).toContain(FEATURE);

    for (const label of ["Default profile", "Environment passed to chats"]) {
      const row = await rowCalled(label);
      expect({ label, off: row.off }).toEqual({ label, off: false });
      if (row.controlOff !== null)
        expect({ label, controlOff: row.controlOff }).toEqual({ label, controlOff: false });
    }
  });

  it("keeps the Network page's rows on, since each says what it writes", async () => {
    await openGroup("Network");
    for (const label of NETWORK_ON) {
      const row = await rowCalled(label);
      expect({ label, off: row.off }).toEqual({ label, off: false });
    }
  });

  it("keeps the Dispatch page's grants and locks on, since each says what it writes", async () => {
    await openGroup("Dispatch");
    for (const label of DISPATCH_ON) {
      const row = await rowCalled(label);
      expect({ label, off: row.off }).toEqual({ label, off: false });
    }
  });

  it("keeps the Dispatch Limits table on, with your own row this machine's", async () => {
    // Its committed rows' boxes are off one by one; the row is not off whole (#1795's second line).
    await openGroup("Dispatch");
    const row = await rowCalled("Limits");
    expect(row.off).toBe(false);
  });

  it("writes no committed project file", async () => {
    // Long enough for Settings' reads after it opened to have landed, which is when a writer
    // that ignored the rule would have written.
    await browser.pause(3_000);
    expect(committedFiles(mine)).toEqual(found);
  });
});
