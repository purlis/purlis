// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";

/**
 * **Settings' editors, in the real WebView** (DS-10, #1842; spec #1832's seam 3):
 *
 * - **Edit as TOML** is CodeMirror: the file's text with numbered lines and the grammar's
 *   colours, as wide as the group it replaces and at least eight controls tall;
 * - **a list field** (Harness & profiles' "Environment passed to chats") is drawn under its
 *   name at the row's width, and as tall as its lines: nothing in it scrolls.
 *
 * What jsdom cannot say: how wide and tall WebKit lays these out. Ranges, never exact pixels.
 * `TextEditor.test.tsx` and `ListField.test.tsx` hold the behaviour. **Nothing is written**: the
 * spec only opens and measures, and closes the Settings tab it opened.
 */

const TABS = '[data-strip="Tabs"]';
const PALETTE = '[role="dialog"][aria-label="Command palette"]';
const GROUPS = 'nav[aria-label="Groups"]';
const SETTINGS = "Settings";
const LIST = "Environment passed to chats";

type Box = {
  top: number;
  bottom: number;
  left: number;
  right: number;
  width: number;
  height: number;
};

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

/** The editor of charter.toml, measured against the section it is drawn in. */
async function editorMeasured() {
  return browser.execute(() => {
    const box = (el: Element | null | undefined): Box | null => {
      if (!el) return null;
      const { top, bottom, left, right, width, height } = el.getBoundingClientRect();
      return { top, bottom, left, right, width, height };
    };
    const text = document.querySelector('[role="textbox"][aria-label="charter.toml, as TOML"]');
    const editor = text?.closest(".cm-editor");
    const section = editor?.closest("section");
    // The section's help line spans its content box: the width the editor should take.
    const help = section?.querySelector("p.ui-setting-help");
    const control = document.querySelector(".ui-raw-actions .ui-button");
    return {
      editor: box(editor),
      content: box(help),
      control: box(control),
      numbers: [...(editor?.querySelectorAll(".cm-lineNumbers .cm-gutterElement") ?? [])]
        .map((one) => one.textContent ?? "")
        .filter((one) => /^\d+$/.test(one)).length,
      coloured: editor?.querySelectorAll(".cm-line span").length ?? 0,
      textarea: Boolean(section?.querySelector("textarea")),
    };
  });
}

/** The list field, its label and its row, measured; and whether any of its lines is hidden. */
async function listMeasured() {
  return browser.execute((label: string) => {
    const box = (el: Element | null | undefined): Box | null => {
      if (!el) return null;
      const { top, bottom, left, right, width, height } = el.getBoundingClientRect();
      return { top, bottom, left, right, width, height };
    };
    const name = [...document.querySelectorAll<HTMLLabelElement>("label.ui-setting-label")].find(
      (one) => one.textContent?.trim() === label,
    );
    const field = name
      ? (document.getElementById(name.htmlFor) as HTMLTextAreaElement | null)
      : null;
    const row = name?.closest(".ui-setting-row");
    return {
      label: box(name),
      field: box(field),
      row: box(row),
      tag: field?.tagName ?? "",
      scrolls: field ? field.scrollHeight > field.clientHeight + 1 : true,
    };
  }, LIST);
}

describe("Settings' editors, in the real WebView", () => {
  let tabWasOpen = false;

  before(async () => {
    tabWasOpen = (await tabNames()).some((name) => name.includes(SETTINGS));
  });

  after(async () => {
    if (!tabWasOpen) {
      const closer = await $(`${TABS} button[aria-label="Close ${SETTINGS}"]`);
      if (await closer.isExisting()) await closer.click();
    }
  });

  it("draws Edit as TOML as CodeMirror: numbered, coloured, the group's width, eight controls tall", async () => {
    await openProjectSettings();
    await $('[role="group"][aria-label="Edit as TOML"]').$("button=charter.toml").click();
    await browser.waitUntil(async () => (await editorMeasured()).coloured > 0, {
      timeout: 20_000,
      timeoutMsg: "Edit as TOML never drew charter.toml in the editor, coloured",
    });
    const { editor, content, control, numbers, textarea } = await editorMeasured();
    if (!editor || !content || !control) throw new Error("the editor was not drawn whole");

    expect(textarea).toBe(false);
    expect(numbers).toBeGreaterThanOrEqual(1);
    // As wide as the section's text, give or take a pixel of rounding.
    expect(Math.abs(editor.left - content.left)).toBeLessThanOrEqual(1);
    expect(Math.abs(editor.right - content.right)).toBeLessThanOrEqual(1);
    // At least eight controls tall, whatever the control height is at this text size.
    expect(editor.height).toBeGreaterThanOrEqual(control.height * 8 - 2);
  });

  it("draws a list field under its name, at the row's width, with every line shown", async () => {
    await $(GROUPS).$("button=Harness & profiles").click();
    await browser.waitUntil(async () => (await listMeasured()).field !== null, {
      timeout: 20_000,
      timeoutMsg: `Harness & profiles never drew ${LIST}`,
    });
    const { label, field, row, tag, scrolls } = await listMeasured();
    if (!label || !field || !row) throw new Error("the row was not drawn whole");

    expect(tag).toBe("TEXTAREA");
    // Under its name: the box starts below the label's line.
    expect(field.top).toBeGreaterThanOrEqual(label.bottom - 1);
    // The row's width: from its left edge to its right, give or take a pixel of rounding.
    expect(Math.abs(field.left - row.left)).toBeLessThanOrEqual(1);
    expect(Math.abs(field.right - row.right)).toBeLessThanOrEqual(1);
    // Every line shown: the box is as tall as its text.
    expect(scrolls).toBe(false);
  });
});
