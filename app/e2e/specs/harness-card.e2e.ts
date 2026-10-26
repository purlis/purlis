// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { endEveryChat, harnessRowsDrawn, pressAndStart, pressOnly } from "../opening.js";

/**
 * **The harness capability card** (HP-19, W10), in the built app: what each harness can do,
 * read by the core off the harness's declaration and the adapter charter ships for it
 * (`purlis_core::harness_card`).
 *
 * - A chat's header names its harness, and opens the harness's card as a view tab — a tab, not
 *   a dialog (the operator's 2026-09-23 ruling: new surfaces are view tabs).
 * - A control that is off because the harness lacks a capability says so in the card's line for
 *   it, followed by the card's label (ADR 0072 §3). The control is a curation row: a project
 *   whose default profile runs opencode cannot have a curation prompt typed into it
 *   (`[terminal] ready_to_type = "never"` in its declaration), so every Curate row is drawn and
 *   cannot run, and its reason is the card's.
 *
 * **It leaves the window as it found it**: the tab it opens it closes, the chat it starts it
 * ends, and the profile file it rewrites it puts back.
 */

const TABS = '[data-strip="Tabs"]';
const PALETTE = '[role="dialog"][aria-label="Command palette"]';
/** The line opencode's card says for what it lacks, and the card's label after it. */
const TYPED_INTO =
  "opencode cannot have a prompt typed in for you, because purlis cannot tell when it has finished starting. See What opencode can do here.";
/** What the Claude Code card's row for it says. */
const TYPED_INTO_ROW = "Can have a prompt typed in for you when it starts";

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

/** The names on the chat strip, left to right. */
async function tabNames(): Promise<string[]> {
  return browser.execute(() =>
    [
      ...(document.querySelector('[data-strip="Tabs"]')?.querySelectorAll('[role="tab"]') ?? []),
    ].map((tab) => tab.textContent ?? ""),
  );
}

/** The label of opencode's card, which an off control's reason ends with (ADR 0072 §3). */
const OPENCODE_CARD = "What opencode can do here";
/** Where the window keeps the harnesses whose one-time note it has said (`cardLink.tsx`). */
const NOTED_KEY = "purlis.harness-noted";
/** The one-time note's opening words (#1134, W10). */
const FIRST_ON = "You have not started a chat on";

/**
 * Opens the menu of the strip tab `tab` and its *Curate* submenu, and answers what the submenu
 * holds: each row's name, and whether it can run. The right-click is the window's own
 * `contextmenu` event at the tab's centre, and the submenu is opened by its row's own click, both
 * from the page: a scenario cannot right-click (`docs/ui-primitives.md`). Nothing where the tab
 * or its Curate row is not there.
 */
async function curateRowsOf(tab: string): Promise<{ name: string; off: boolean; why: string }[]> {
  const opened = await browser.execute((css: string) => {
    const el = document.querySelector(css);
    if (!(el instanceof HTMLElement)) return false;
    const box = el.getBoundingClientRect();
    el.dispatchEvent(
      new MouseEvent("contextmenu", {
        bubbles: true,
        cancelable: true,
        clientX: Math.round(box.left + box.width / 2),
        clientY: Math.round(box.top + box.height / 2),
      }),
    );
    return true;
  }, tab);
  if (!opened) return [];
  await $('[role="menu"]').waitForDisplayed({ timeout: 20_000 });
  const sub = await browser.execute(() => {
    const curate = [
      ...document.querySelectorAll<HTMLElement>(
        '[role="menu"] [role="menuitem"][aria-haspopup="menu"]',
      ),
    ].find((row) => (row.textContent ?? "").includes("Curate"));
    curate?.click();
    return curate !== undefined;
  });
  if (!sub) return [];
  await browser
    .waitUntil(() => browser.execute(() => document.querySelectorAll('[role="menu"]').length > 1), {
      timeout: 10_000,
    })
    .catch(() => undefined);
  return browser.execute(() => {
    const menus = [...document.querySelectorAll('[role="menu"]')];
    const submenu = menus.at(-1);
    if (menus.length < 2 || submenu === undefined) return [];
    return [...submenu.querySelectorAll('[role="menuitem"]')].map((row) => ({
      name: row.getAttribute("aria-label") ?? row.textContent ?? "",
      off: row.getAttribute("aria-disabled") === "true" || row.hasAttribute("data-disabled"),
      // Why it cannot run: its description, which its words also say under them.
      why: document.getElementById(row.getAttribute("aria-describedby") ?? "")?.textContent ?? "",
    }));
  });
}

/** Closes every menu open, from the keyboard: the driver's Escape, else the page's. */
async function closeMenus(): Promise<void> {
  for (let left = 0; left < 3; left++) {
    if (!(await $('[role="menu"]').isExisting())) return;
    await browser.keys(["Escape"]);
    const gone = await $('[role="menu"]')
      .waitForExist({ timeout: 2_000, reverse: true })
      .then(() => true)
      .catch(() => false);
    if (gone) return;
    await browser.execute(() =>
      (document.activeElement ?? document.body).dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
      ),
    );
  }
}

/** Closes the card tab `name`, if it is on the strip, and waits until it is gone. */
async function closeCardTab(name: string): Promise<void> {
  const closer = await $(`${TABS} button[aria-label="Close ${name}"]`);
  if (!(await closer.isExisting())) return;
  await browser.execute((el: HTMLElement) => el.click(), closer as unknown as HTMLElement);
  await browser.waitUntil(async () => !(await tabNames()).includes(name), {
    timeout: 20_000,
    timeoutMsg: `the ${name} tab did not close`,
  });
}

/** What the window holds of the harnesses whose one-time note it has said, or null. */
const notedNow = () =>
  browser.execute((key: string) => {
    try {
      return window.localStorage.getItem(key);
    } catch {
      return null;
    }
  }, NOTED_KEY);

/** Puts back what the window held of the notes said, or takes it away where it held none. */
const noted = (held: string | null) =>
  browser.execute(
    (key: string, value: string | null) => {
      try {
        if (value === null) window.localStorage.removeItem(key);
        else window.localStorage.setItem(key, value);
      } catch {
        // Nothing kept: the note is said again, which the spec's first read checks for.
      }
    },
    NOTED_KEY,
    held,
  );

/** The one-time note the open picker says, or "" where it says none. */
const pickerNote = () =>
  browser.execute((opening: string) => {
    const note = [...document.querySelectorAll<HTMLElement>('[role="dialog"] [role="note"]')].find(
      (one) => one.innerText.includes(opening),
    );
    return note?.innerText ?? "";
  }, FIRST_ON);

describe("a harness's capability card", function () {
  this.timeout(180_000);

  // The window has drawn its bar before anything here presses it: a spec run on its own is the
  // first in its app process, and the first frame can come late on a loaded machine.
  before(async () => {
    await $('button[aria-label="New tab"]').waitForClickable({ timeout: 60_000 });
  });

  it("is named in a chat's header, and opens the harness's card in a tab", async () => {
    // The plane's `scenario` profile is of kind claude: its chat runs Claude Code's card.
    await pressAndStart("New tab");
    const chip = await $(`button[aria-label="What Claude Code can do here"]`);
    await chip.waitForExist({ timeout: 30_000 });
    expect(await chip.getText()).toBe("Claude Code");

    await chip.click();

    const card = await $('[data-testid="view-pane-charter-harness-claude"]');
    await card.waitForExist({ timeout: 20_000 });
    await browser.waitUntil(async () => (await card.getText()).includes(TYPED_INTO_ROW), {
      timeout: 20_000,
      timeoutMsg: "the card tab never listed what charter can do with Claude Code",
    });
    const said = await card.getText();
    expect(said).toContain("What Claude Code can do here");
    expect(said).toContain("Tells purlis when it is waiting for you");
    expect(await tabNames()).toContain("What Claude Code can do here");

    await $(`${TABS} button[aria-label="Close What Claude Code can do here"]`).click();
    await browser.waitUntil(
      async () => !(await tabNames()).includes("What Claude Code can do here"),
      {
        timeout: 20_000,
        timeoutMsg: "the card tab did not close",
      },
    );
    await endEveryChat();
  });

  describe("on a project whose default harness lacks a capability", () => {
    let local = "";
    let was = "";

    before(async () => {
      local = join((await ask<string[]>("open_planes"))[0], "charter.local.toml");
      was = readFileSync(local, "utf8");
      // The same stand-in program, declared as opencode and made the default. The program's
      // path is the scenario profile's own, read back out of the file it is in.
      const command = /command = (\[.*\])/.exec(was)?.[1];
      if (!command) throw new Error(`no command in ${local}: ${was}`);
      writeFileSync(
        local,
        `${was.replace('default = "scenario"', 'default = "typing-off"')}\n` +
          `[harness.typing-off]\nkind = "opencode"\ncommand = ${command}\n`,
      );
      await browser.execute(() => window.dispatchEvent(new Event("focus")));
    });

    after(async () => {
      if (await $(PALETTE).isExisting()) await browser.keys(["Escape"]);
      writeFileSync(local, was);
      await browser.execute(() => window.dispatchEvent(new Event("focus")));
    });

    it("draws a Curate row that cannot run, saying the card's line and label", async () => {
      let reasons: string[] = [];
      await browser
        .waitUntil(
          async () => {
            await browser.keys(["F2"]);
            const palette = await $(PALETTE);
            await palette.waitForDisplayed({ timeout: 20_000 });
            await (await palette.$("input")).addValue("Curate");
            reasons = await browser.execute(() =>
              [
                ...document.querySelectorAll(
                  '[role="dialog"][aria-label="Command palette"] [role="option"][aria-disabled="true"] .palette-why',
                ),
              ].map((why) => why.textContent ?? ""),
            );
            const found = reasons.some((why) => why.includes(TYPED_INTO));
            await browser.keys(["Escape"]);
            await expect($(PALETTE)).not.toBeDisplayed();
            return found;
          },
          { timeout: 60_000, interval: 1_000 },
        )
        .catch((why: unknown) => {
          throw new Error(
            `${String(why)} — the off Curate rows said: ${JSON.stringify(reasons.slice(0, 3))}`,
          );
        });
    });

    /**
     * **The card's label, as a row of its own under the Curate rows that cannot run** (#1134's
     * second line, D-1134-1): a disabled menu row is skipped by the keyboard, so the link is a
     * row under them, and pressing it opens the card's tab, as the palette's row does. Tried on
     * a workspace's tab, then on the project's own: whichever holds the off rows.
     */
    it("puts the card's label under the Curate rows that cannot run, and opens its tab from there", async () => {
      // A workspace's chip in the title bar (#2916), then the project's own button on the rail.
      const tabs = [
        '[data-testid="title-bar"] [data-scope="titlebar"][data-part="workspace"]',
        '[data-strip="Projects"] [role="tab"][aria-selected="true"]',
      ];
      let seen: string[] = [];
      let last = "no attempt finished";
      // Each subject's tab in turn, again until the rows are there: the window reads the
      // harnesses' cards and the curation rows again once it has focus after the profile file
      // changed, each in its own time.
      const tryEach = async () => {
        seen = [];
        for (const tab of tabs) {
          const rows = await curateRowsOf(tab);
          seen.push(
            `${tab}: ${JSON.stringify(rows.map((row) => `${row.name}${row.off ? ` (off: ${row.why})` : ""}`))}`,
          );
          const lacking = rows.filter(
            (row) => row.off && row.why.endsWith(`See ${OPENCODE_CARD}.`),
          );
          const card = rows.find((row) => row.name === OPENCODE_CARD);
          if (lacking.length === 0 || card === undefined) {
            await closeMenus();
            continue;
          }
          expect(card.off).toBe(false);
          // Pressed by its own click, from the page: the submenu can sit past the window's edge
          // where the driver counts a row as not on screen.
          await browser.execute((name: string) => {
            const menus = [...document.querySelectorAll('[role="menu"]')];
            [...(menus.at(-1)?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])]
              .find((row) => row.getAttribute("aria-label") === name)
              ?.click();
          }, OPENCODE_CARD);
          return true;
        }
        last = seen.join("; ");
        return false;
      };
      try {
        await browser.execute(() => window.dispatchEvent(new Event("focus")));
        const opened = await browser
          .waitUntil(tryEach, { timeout: 30_000, interval: 1_000 })
          .then(() => true)
          .catch(() => false);
        if (!opened) {
          await closeMenus();
          // What the palette lists of the cards, to tell a card row missing from the catalogue
          // from one the menu did not draw.
          await browser.keys(["F2"]);
          await $(PALETTE).waitForDisplayed({ timeout: 20_000 });
          await (await $(PALETTE).$("input")).addValue("can do here");
          const cards = await browser.execute(
            (css: string) =>
              [...document.querySelectorAll(`${css} [role="option"]`)].map(
                (one) => one.textContent ?? "",
              ),
            PALETTE,
          );
          await browser.keys(["Escape"]);
          throw new Error(
            `no Curate submenu put ${OPENCODE_CARD} under rows that cannot run; it held ${last}; the palette's cards: ${JSON.stringify(cards)}`,
          );
        }
        const pane = await $('[data-testid^="view-pane-charter-harness-"]');
        await browser.waitUntil(async () => (await tabNames()).includes(OPENCODE_CARD), {
          timeout: 20_000,
          timeoutMsg: `the card's row opened no ${OPENCODE_CARD} tab; the strip holds ${JSON.stringify(await tabNames())}`,
        });
        await pane.waitForExist({ timeout: 20_000 });
        await browser.waitUntil(async () => (await pane.getText()).includes(OPENCODE_CARD), {
          timeout: 20_000,
          timeoutMsg: "the card tab never drew opencode's card",
        });
      } finally {
        await closeMenus();
        await closeCardTab(OPENCODE_CARD);
      }
    });

    /**
     * **The card's lines, said once, by Start** (#1134's first line, W10): the first picker on a
     * harness that lacks something says the card's lines; once a chat is started on it, the next
     * picker on it says nothing more. "Once" is kept in the window's web storage, which this
     * test empties first and puts back after, and the chat it starts it ends. A start the stand-in
     * cannot make still counts: the note is kept at the press, not when the chat is up. The
     * project's `opencode.json` is put back as it was too, whatever an opencode chat's start did
     * to it: the specs after this one read the fixture's.
     */
    it("says the card's lines once by Start, and not again after a chat started on it", async () => {
      const held = await notedNow();
      const config = join((await ask<string[]>("open_planes"))[0], "opencode.json");
      const configWas = existsSync(config) ? readFileSync(config, "utf8") : undefined;
      await noted(null);
      try {
        await pressOnly("New tab");
        await harnessRowsDrawn();
        await browser
          .waitUntil(async () => (await pickerNote()) !== "", { timeout: 20_000 })
          .catch(async () => {
            const said = await browser.execute(
              () =>
                (document.querySelector('[role="dialog"]') as HTMLElement | null)?.innerText ?? "",
            );
            throw new Error(`the first picker on opencode said no note; it said: ${said}`);
          });
        expect(await pickerNote()).toContain("opencode");

        // Start, as `pickAndStart` presses it: Approve and start on the profile's first use.
        // Pressed from the document: the note and the card make the picker taller than
        // WebDriver counts as on screen.
        const dialog = await $('[role="dialog"]');
        await browser.waitUntil(
          () =>
            browser.execute(() => {
              const start = [
                ...document.querySelectorAll<HTMLButtonElement>('[role="dialog"] button'),
              ]
                .filter(
                  (button) => !button.disabled && button.getAttribute("aria-disabled") !== "true",
                )
                .find((button) =>
                  ["Approve and start", "Start chat"].includes(button.textContent ?? ""),
                );
              start?.click();
              return start !== undefined;
            }),
          { timeout: 20_000, timeoutMsg: "the picker offered no Start" },
        );
        const closed = await dialog
          .waitForDisplayed({ timeout: 20_000, reverse: true })
          .then(() => true)
          .catch(() => false);
        if (!closed) await browser.keys(["Escape"]);
        expect(await notedNow()).toContain("opencode");
        await endEveryChat();

        await pressOnly("New tab");
        await harnessRowsDrawn();
        // The picker is drawn whole by now; a note would be in it with the rows.
        await browser.pause(1_000);
        expect(await pickerNote()).toBe("");
        await browser.keys(["Escape"]);
        await $('[role="dialog"]').waitForDisplayed({ timeout: 20_000, reverse: true });
      } finally {
        if (await $('[role="dialog"]').isExisting()) await browser.keys(["Escape"]);
        await endEveryChat();
        await noted(held);
        if (configWas === undefined) rmSync(config, { force: true });
        else if (!existsSync(config) || readFileSync(config, "utf8") !== configWas)
          writeFileSync(config, configWas);
      }
    });
  });
});
