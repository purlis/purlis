// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { theWindowHoldsTheRail, WINDOW_1280, windowAt } from "../rail.js";

/**
 * **The merge view's keyboard list of differences, in the real WebView** (#1875, after #1862):
 * Edit as TOML on `charter.toml`, a line typed into the edit, and the file written on disk under
 * it. The editor becomes the merge view, the file as it now is beside the edit, and under it a
 * button per difference, named by the lines it takes.
 *
 * What jsdom cannot say, and this measures: that WebKit draws the list under the merge view, at
 * the section's width, each button a control's height and none clipped; that every difference is
 * listed, the one scrolled out of sight too; that the gutter's arrows are out of the Tab order
 * while the list's buttons are in it; and that taking one moves the keyboard to the next, then
 * back into the edit once nothing differs. A button is pressed by its own click from the page:
 * WebDriver's Enter activates no button (`palette.e2e.ts` says why), and `TextEditor.test.tsx`
 * holds Enter and Space.
 *
 * **It leaves the window and the plane as it found them**: the edit is discarded, `charter.toml`
 * is put back byte for byte whatever happened (one app process serves the whole run), and a
 * Settings tab this spec opened is closed.
 */

const TABS = '[data-strip="Tabs"]';
const PALETTE = '[role="dialog"][aria-label="Command palette"]';
const GROUPS = 'nav[aria-label="Groups"]';
const SETTINGS = "Settings";
const EDIT = "charter.toml, as TOML";
const DISK = "charter.toml, on disk now";
const MERGE = '[data-testid="conflict-merge"]';
const LIST = `[role="group"][aria-label="Differences from ${DISK}"]`;
const TYPED = "# typed in the window, #1875\n";
const WRITTEN = "# written on disk under the edit, #1875\n";

type Box = { top: number; bottom: number; left: number; right: number; height: number };

/** The plane the app says it is acting on — the copy this run was given, never the repo's. */
async function planeRoot(): Promise<string> {
  // The status line says the project by its name; its directory is on the item (qw357).
  const said = await $('[data-testid="status-project"]');
  await said.waitForDisplayed({ timeout: 30_000 });
  return ((await said.getAttribute("data-directory")) ?? "").trim();
}

/** The names on the tab strip. */
async function tabNames(): Promise<string[]> {
  return browser.execute(() =>
    [
      ...(document.querySelector('[data-strip="Tabs"]')?.querySelectorAll('[role="tab"]') ?? []),
    ].map((tab) => tab.querySelector(".tab-name")?.textContent ?? tab.textContent ?? ""),
  );
}

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

/** Opens `file` from the Edit as TOML links at the nav's foot, found by their words. */
async function openRawFile(file: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute((wanted: string) => {
        const group = [...document.querySelectorAll<HTMLElement>('[role="group"]')].find(
          (one) =>
            document.getElementById(one.getAttribute("aria-labelledby") ?? "")?.textContent ===
            "Edit as TOML",
        );
        const link = [...(group?.querySelectorAll<HTMLElement>("button") ?? [])].find(
          (one) => one.textContent?.trim() === wanted,
        );
        if (link === undefined) return false;
        link.click();
        return true;
      }, file),
    { timeout: 20_000, interval: 250, timeoutMsg: `Edit as TOML offered no ${file}` },
  );
}

/** The merge view and its list, measured in one pass. */
async function measured() {
  return browser.execute(
    (merge: string, list: string) => {
      const box = (el: Element | null | undefined): Box | null => {
        if (!el) return null;
        const { top, bottom, left, right, height } = el.getBoundingClientRect();
        return { top, bottom, left, right, height };
      };
      const view = document.querySelector(merge);
      const takes = document.querySelector(list);
      const section = view?.closest("section");
      const buttons = [...(takes?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
      const arrows = [...(view?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
      return {
        merge: box(view),
        list: box(takes),
        // The section's help line spans its content box: the width the list may take.
        content: box(section?.querySelector("p.ui-setting-help")),
        // The section's own Save: a control at its full height.
        save: box(section?.querySelector('.ui-raw-actions [data-scope="button"]')),
        names: buttons.map((one) => one.textContent?.trim() ?? ""),
        buttons: buttons.map((one) => ({
          ...box(one),
          tabIndex: one.tabIndex,
          clipped: one.scrollWidth > one.clientWidth + 1,
        })),
        arrows: arrows.map((one) => ({
          tabIndex: one.tabIndex,
          hidden: one.getAttribute("aria-hidden"),
        })),
        focused: document.activeElement?.textContent?.trim() ?? "",
        focusedLabel: document.activeElement?.getAttribute("aria-label") ?? "",
      };
    },
    MERGE,
    LIST,
  );
}

describe("the merge view's keyboard list of differences", function () {
  // At 1280×800, both sidebars open, where the project rail hides itself to leave the work its
  // room (`rail.ts`, #3077): measured with the rail as the window draws it.
  before(() => windowAt(WINDOW_1280));
  after(() => theWindowHoldsTheRail());

  this.timeout(180_000);

  let file = "";
  let original: string | null = null;
  let tabWasOpen = false;

  before(async () => {
    file = join(await planeRoot(), "charter.toml");
    original = existsSync(file) ? readFileSync(file, "utf8") : null;
    tabWasOpen = (await tabNames()).some((name) => name.includes(SETTINGS));
  });

  after(async () => {
    // The edit is discarded where it is still drawn, so the tab holds no draft of it.
    await browser.execute(() => {
      const discard = [...document.querySelectorAll<HTMLElement>("section.ui-raw button")].find(
        (one) => one.textContent?.trim() === "Discard",
      );
      discard?.click();
    });
    // Whatever happened, the file is what it was before this spec. A `before` that failed read
    // nothing, so touches nothing.
    if (file !== "") {
      const now = existsSync(file) ? readFileSync(file, "utf8") : null;
      if (original === null) {
        if (now !== null) rmSync(file);
      } else if (now !== original) writeFileSync(file, original);
    }
    if (!tabWasOpen) {
      const closer = await $(`${TABS} button[aria-label="Close ${SETTINGS}"]`);
      if (await closer.isExisting()) await closer.click();
    }
  });

  it("lists every difference under the view, at its width, and the keyboard goes from one to the next", async () => {
    if (original === null) throw new Error("the plane this run was given has no charter.toml");
    await openProjectSettings();
    await openRawFile("charter.toml");
    const edit = `[role="textbox"][aria-label="${EDIT}"]`;
    await $(edit).waitForExist({ timeout: 20_000 });

    // Typed at the top through the editing command a person's typing runs, which CodeMirror
    // reads from the page (`view-tabs.e2e.ts` says why a driver's key is not).
    await browser.execute(
      (css: string, typed: string) => {
        document.querySelector<HTMLElement>(css)?.focus();
        document.execCommand("insertText", false, typed);
      },
      edit,
      TYPED,
    );
    await browser.waitUntil(async () => (await $(edit).getText()).includes(TYPED.trim()), {
      timeout: 10_000,
      timeoutMsg: "the edit never held what was typed",
    });

    // The file moves under the edit, at its other end: far from the typed line, so the merge
    // view folds the lines between and the second difference starts out of sight.
    writeFileSync(file, original + WRITTEN);
    await $(LIST).waitForExist({
      timeout: 30_000,
      timeoutMsg: "the editor never became the merge view with its list of differences",
    });
    await browser.waitUntil(async () => (await measured()).names.length === 2, {
      timeout: 10_000,
      timeoutMsg: "the list never named both differences",
    });

    const seen = await measured();
    const { merge, list, content, save } = seen;
    if (!merge || !list || !content || !save) throw new Error("the merge view was not drawn whole");

    // Named by the lines each takes: the typed line the file does not have, and the file's new
    // last line.
    expect(seen.names[0]).toMatch(/^Take out line 1, which charter\.toml, on disk now/);
    expect(seen.names[1]).toMatch(/^Take line \d+ from charter\.toml, on disk now$/);
    // Under the view, inside the section's width.
    expect(list.top).toBeGreaterThanOrEqual(merge.bottom - 1);
    expect(list.left).toBeGreaterThanOrEqual(content.left - 1);
    expect(list.right).toBeLessThanOrEqual(content.right + 1);
    for (const button of seen.buttons) {
      // A small button: no taller than the section's Save, none clipped, and in the Tab order.
      expect(button.height ?? 0).toBeGreaterThan(0);
      expect(button.height ?? 0).toBeLessThanOrEqual(save.height + 1);
      expect(button.left ?? 0).toBeGreaterThanOrEqual(content.left - 1);
      expect(button.right ?? 0).toBeLessThanOrEqual(content.right + 1);
      expect(button.clipped).toBe(false);
      expect(button.tabIndex).toBe(0);
    }
    // The gutter's arrows are the pointer's: out of the Tab order, and silent.
    for (const arrow of seen.arrows) {
      expect(arrow.tabIndex).toBe(-1);
      expect(arrow.hidden).toBe("true");
    }

    // Taken, the keyboard goes on to the difference now in its place...
    await browser.execute((css: string) => {
      const first = document.querySelector<HTMLElement>(`${css} button`);
      first?.focus();
      first?.click();
    }, LIST);
    await browser.waitUntil(async () => (await measured()).names.length === 1, {
      timeout: 10_000,
      timeoutMsg: "taking the first difference did not take it",
    });
    await browser.waitUntil(async () => (await measured()).focused.startsWith("Take line"), {
      timeout: 10_000,
      timeoutMsg: "the keyboard did not go on to the next difference",
    });

    // ...and back into the edit once nothing differs.
    await browser.execute((css: string) => {
      document.querySelector<HTMLElement>(`${css} button`)?.click();
    }, LIST);
    await browser.waitUntil(async () => !(await $(LIST).isExisting()), {
      timeout: 10_000,
      timeoutMsg: "the list stayed after every difference was taken",
    });
    await browser.waitUntil(async () => (await measured()).focusedLabel === EDIT, {
      timeout: 10_000,
      timeoutMsg: "the keyboard did not come back into the edit",
    });
  });
});
