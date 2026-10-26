// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, readFileSync, realpathSync, renameSync, writeFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { openMenuOn } from "../acting.js";
import { ALL_WORKSPACES, goToAWorkspace, goToWorkspace } from "../titleBarWorkspaces.js";
import { anEmptyRecord, copyFixturePlane } from "../harness.js";
import { closeProject, openThroughTheTrustQuestion } from "../opening.js";

/**
 * **The quiet gears, and ⌘, at the focused level**, in the built app (SE-23, #1173; V89g and
 * V89i on #558): the gear on the project in front opens Settings at the Project level, a value
 * changed there lands in `charter.toml` on disk, and the app menu's Settings… opens the focused
 * workspace's level.
 *
 * `src/settings/SettingsGears.test.tsx` owns which tab carries a gear, its name, its Tab stop and
 * the stylesheet's quiet; what only a scenario can say is that the path holds with the real core
 * behind it and the real file under it.
 *
 * **⌘, is sent as the event the menu sends** (`settings-asked`, `lifecycle.rs`). The accelerator
 * is a native menu's, which a WebDriver keystroke never reaches — it is synthesised inside the
 * page. `lifecycle.rs`'s tests hold the accelerator to `⌘,` and `Ctrl+,`; this holds everything
 * after it.
 *
 * **Its own project, copied into the run's tree** (charter-app#129's fence): it writes a setting,
 * so it may only ever be pointed at a plane the run itself made. **It closes it through the
 * window**, by the tab's own ×, as `saving.e2e.ts` does, and asserts the tab is gone: one app
 * process serves the whole run, so a project left in front here, with its Settings tab, is the
 * strip every later spec reads. `close_plane` asked directly takes the tab out too
 * (`plane-closed`, #1242); the × is what an operator presses.
 */

const PROJECTS = '[data-strip="Projects"]';
const LEVEL = '[role="radiogroup"][aria-label="Level"]';

const mine = (() => {
  const copied = copyFixturePlane();
  const renamed = join(dirname(copied), "gears");
  renameSync(copied, renamed);
  const root = realpathSync(renamed);
  // Nothing to put back, so opening it starts no chat for the next spec to inherit.
  anEmptyRecord(root);
  return root;
})();

/** The Sign commits row, by its control's id: what the control is, whether it is on, and where
 *  it and the row's Reset are drawn (`null` where there is no Reset). */
async function signRow(id: string) {
  return browser.execute((control: string) => {
    const box = (el: Element) => {
      const { top, bottom, left, right } = el.getBoundingClientRect();
      return { top, bottom, left, right };
    };
    const it = document.getElementById(control);
    const reset = [
      ...(it?.closest(".ui-setting-row")?.querySelectorAll<HTMLElement>("button") ?? []),
    ].find((one) => one.textContent?.trim() === "Reset");
    return {
      role: it?.getAttribute("role") ?? null,
      // A native switch in its own label (#2907): its state is the input's, its box the label's.
      checked: it instanceof HTMLInputElement ? String(it.checked) : null,
      box: it ? box(it.closest("label") ?? it) : null,
      reset: reset ? box(reset) : null,
    };
  }, id);
}

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

/** The level the Settings tab on screen is at, by its switcher. */
async function levelBecomes(want: string): Promise<void> {
  let saw = "";
  await browser.waitUntil(
    async () => {
      saw = await browser.execute(
        (selector: string) =>
          document
            .querySelector(`${selector} input[type="radio"]:checked`)
            ?.closest("label")
            ?.textContent?.trim() ?? "",
        LEVEL,
      );
      return saw === want;
    },
    { timeout: 30_000, timeoutMsg: `Settings stayed at ${JSON.stringify(saw)}, not ${want}` },
  );
}

const sharedFile = () => join(mine, "charter.toml");
/** The committed file as the copy had it, put back when the spec is done. */
const sharedBefore = readFileSync(join(mine, "charter.toml"), "utf8");
const localFile = () => join(mine, "charter.local.toml");

describe("the settings gears, and ⌘, at the focused level", function () {
  this.timeout(180_000);

  /** The project the launch opened, which this spec never touches. */
  let first = "";

  before(async () => {
    first = (await ask<string[]>("open_planes"))[0];
    // Through the opener, as a person opens one (`workspace-lifecycle.e2e.ts` says why).
    await $(`${PROJECTS} button[aria-label="Open a project…"]`).click();
    const box = await $('[data-setting="open-by-path"] input');
    await box.waitForDisplayed({ timeout: 20_000 });
    await box.addValue(mine);
    // The path box's own submit, found by its form, and the trust question's act found
    // in the question: the two are both named Open project.
    await $('form.by-path button[type="submit"]').click();
    await openThroughTheTrustQuestion();
    await browser.waitUntil(async () => (await ask<string[]>("open_planes")).includes(mine), {
      timeout: 30_000,
      timeoutMsg: "this spec's project never opened",
    });
    await $(ALL_WORKSPACES).waitForExist({ timeout: 30_000 });
  });

  after(async () => {
    // The window as it was found: this spec's project closed from its own tab, which takes its
    // Settings tab with it and brings the launch's project back to the front.
    const closer = `${PROJECTS} [role="tab"][aria-label="${basename(mine)}"]`;
    if (await $(closer).isExisting()) {
      await closeProject(closer);
      await browser.waitUntil(async () => !(await $(closer).isExisting()), {
        timeout: 20_000,
        timeoutMsg: "this spec's project stayed on the strip",
      });
    }
    // Anything the window could not close is let go of in the core all the same.
    for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== first)) {
      await ask("close_plane", { plane });
    }
    writeFileSync(sharedFile(), sharedBefore);
  });

  it("opens Project settings from the gear on the project in front, and writes charter.toml", async () => {
    // The plane root is not a workspace, so nothing narrower than the project is focused.
    await goToWorkspace("Plane root");

    // Quiet until its tab is under the pointer or the keyboard, and neither can be made to show
    // it from here: a WebDriver pointer move does not make WebKit match `:hover` (macOS and
    // WebKitGTK, run 37260967190), a WebDriver Tab is synthesised inside the page and moves no
    // focus, and whether a scripted focus matches `:focus-visible` depends on what the specs
    // before this one did with the pointer. So the gear is pressed as it is, and its quiet is
    // `SettingsGears.test.tsx`'s to hold: the stylesheet's rules, and that a Tab from the strip
    // reaches it (D-SE23f). What this holds is the path from that press to the file.
    // The rail has no gear (#2915): its project's menu has the same row.
    await openMenuOn(`${PROJECTS} [role="tab"][aria-selected="true"]`);
    await $('[role="menu"]').$('[role="menuitem"]*=Settings').click();

    await levelBecomes("Project");

    await $('nav[aria-label="Groups"]').$("button=Saving").click();
    // An on/off key of the files is a switch at the value in force (#1865), off by default and
    // not set here, so with no Reset beside it yet. Found by its `<label for>`, which wdio's
    // `aria/` selector does not resolve here.
    const signId = await browser.execute(
      () =>
        [...document.querySelectorAll("label")].find(
          (one) => one.textContent?.trim() === "Sign commits",
        )?.htmlFor ?? "",
    );
    expect(signId).not.toBe("");
    // A select of on, off and not set until the core has said what is in force.
    await browser.waitUntil(async () => (await signRow(signId)).role === "switch", {
      timeout: 20_000,
      timeoutMsg: "Sign commits never became a switch",
    });
    const before = await signRow(signId);
    expect(before).toMatchObject({ role: "switch", checked: "false", reset: null });
    // A click on the switch's input itself: the WebView's driver clicks without a pointer, which
    // React Aria reads as a virtual press, the input's to take and not its label's (#2907).
    await browser.execute((id: string) => document.getElementById(id)?.click(), signId);

    // Shared by default, so it lands in the committed file and not in this machine's.
    await browser.waitUntil(
      async () => /sign\s*=\s*true/.test(readFileSync(sharedFile(), "utf8")),
      { timeout: 30_000, timeoutMsg: "charter.toml never said sign = true" },
    );
    expect(existsSync(localFile()) ? readFileSync(localFile(), "utf8") : "").not.toMatch(/sign/);

    // Set here now: on, with its Reset on the switch's line, after it (#1865).
    let after = await signRow(signId);
    await browser.waitUntil(
      async () => {
        after = await signRow(signId);
        return after.reset !== null;
      },
      { timeout: 20_000, timeoutMsg: "Sign commits, set here, never offered Reset" },
    );
    const { box, reset } = after;
    if (box === null || reset === null) throw new Error("the switch or its Reset is not drawn");
    expect(after.checked).toBe("true");
    expect(
      Math.abs((box.top + box.bottom) / 2 - (reset.top + reset.bottom) / 2),
    ).toBeLessThanOrEqual(2);
    expect(reset.left).toBeGreaterThanOrEqual(box.right - 1);
  });

  it("opens the focused workspace's level on the app menu's Settings…", async () => {
    const focused = await goToAWorkspace();
    expect(focused).not.toBe("");

    await browser.executeAsync((done: () => void) => {
      void window.__TAURI__.event.emit("settings-asked").then(done, done);
    });

    await levelBecomes("Workspace");
    await expect($('[data-strip="Tabs"] [role="tab"][aria-selected="true"]')).toHaveText(
      `Settings · ${focused}`,
      { containing: true },
    );
  });
});
