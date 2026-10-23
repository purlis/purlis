// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import { READY } from "../harness.js";
import { endChatsStartedHere, startAChat, type Started } from "../started.js";

/**
 * **The token and time limits' lines, and the settings set's native controls, in the real
 * WebView** (#1545 line 2; #626's train 27 follow-up 2).
 *
 * What jsdom cannot say: how the note under the limits table wraps in the page at a wide and a
 * narrow window, how the table stacks into a form at the narrowest Settings (#1859), how a task's
 * row reads "stopped at its time limit" in its tab's menu, and whether the engine's own keys
 * reach a native range, and the select's arrow its list, with nothing of the window's in the
 * way. `DispatchLimits.test.tsx`, `collections.test.ts`, `shownState.test.ts`,
 * `TabChip.window.test.tsx` and `gallery.test.tsx` hold the rest.
 *
 * **The task is pretended**, as `tab-chip.e2e.ts` pretends its tasks (`src/e2eTasks.ts`): a
 * task purlis stopped at its time limit needs a persona chat, a core that dispatched it and a
 * limit that ran out, none of which this suite's fake harness makes. The row, its word and the
 * menu it is in are the app's own from there.
 *
 * **The session's token line is not here**: the footer says it only once the session's own
 * limit numbers reach the window, and the pretend seam carries tasks alone (listed on #1545).
 *
 * **It leaves the window as it found it**: one app process serves the whole run. Nothing here
 * writes a setting (an override picked is not written until a limit is typed), the text size it
 * steps is stepped back, and the chat it opens is ended.
 */

const STRIP = '[data-strip="Tabs"]';
const PALETTE = '[role="dialog"][aria-label="Command palette"]';
const NOTE = '[data-testid="dispatch-tokens-time-note"]';

type Box = { left: number; right: number; top: number; bottom: number };

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
  await $('nav[aria-label="Groups"]').waitForDisplayed({ timeout: 20_000 });
}

/** Presses the level called `level` on the switcher, then the nav's group called `group`. */
async function goTo(level: string, group: string): Promise<void> {
  await browser.execute((called: string) => {
    const radio = [
      ...document.querySelectorAll<HTMLElement>(
        '[role="radiogroup"][aria-label="Level"] [role="radio"]',
      ),
    ].find((one) => one.textContent?.trim() === called);
    if (radio?.getAttribute("aria-checked") !== "true") radio?.click();
  }, level);
  await browser.waitUntil(
    async () =>
      browser.execute((called: string) => {
        const button = [
          ...document.querySelectorAll<HTMLElement>('nav[aria-label="Groups"] button'),
        ].find((one) => one.textContent?.trim() === called);
        button?.click();
        return button?.getAttribute("aria-current") === "true";
      }, group),
    { timeout: 20_000, interval: 250, timeoutMsg: `Settings never showed ${level} › ${group}` },
  );
}

async function windowIs(width: number, height: number): Promise<void> {
  await browser.setWindowSize(width, height);
  let last = -1;
  await browser.waitUntil(
    async () => {
      const now = await browser.execute(() => window.innerWidth);
      const settled = now === last;
      last = now;
      return settled;
    },
    { timeout: 10_000, interval: 200, timeoutMsg: "the window never settled on a width" },
  );
}

/** The note under the limits table, measured against the column it is drawn in. */
async function theNote() {
  return browser.execute((note: string) => {
    const box = (el: Element | null): Box | null => {
      if (!el) return null;
      const { left, right, top, bottom } = el.getBoundingClientRect();
      return { left, right, top, bottom };
    };
    const said = document.querySelector<HTMLElement>(note);
    return {
      text: said?.textContent ?? "",
      note: box(said),
      body: box(said?.closest(".ui-settings-body") ?? null),
      cut: said ? said.scrollWidth > said.clientWidth + 1 : true,
      pageScrolls: document.documentElement.scrollWidth > window.innerWidth + 1,
    };
  }, NOTE);
}

/** The two sidebars: each one's activity bar, and the slot it opens. */
const SIDES = [
  ["Navigation", "region-left"],
  ["Attention", "region-right"],
] as const;

/**
 * Opens each sidebar that is put away, by a press of a view's icon on its activity bar, so the
 * narrowest case does not depend on what a spec run before it left (as `tab-strip-centre.e2e.ts`
 * does). Returns each icon it pressed, for {@link putAwayAgain}.
 */
async function bothSidesOpen(): Promise<string[]> {
  const pressed: string[] = [];
  for (const [bar, slot] of SIDES) {
    const side = await $(`[data-panel][id="${slot}"]`);
    if (((await side.getSize("width")) as number) > 0) continue;
    const icons = `[role="tablist"][aria-label="${bar}"] [role="tab"]`;
    const open = await $(`${icons}[aria-selected="true"]`);
    const icon = (await open.isExisting()) ? open : await $(icons);
    const name = (await icon.getAttribute("aria-label")) ?? "";
    await icon.click();
    await browser.waitUntil(async () => ((await side.getSize("width")) as number) > 0, {
      timeout: 20_000,
      timeoutMsg: `the ${bar} side did not open when its view was pressed`,
    });
    pressed.push(`${bar}\u0000${name}`);
  }
  return pressed;
}

/** Puts away again each sidebar {@link bothSidesOpen} opened: a press of its open view. */
async function putAwayAgain(pressed: string[]): Promise<void> {
  for (const one of pressed) {
    const [bar, name] = one.split("\u0000");
    await (
      await $(`[role="tablist"][aria-label="${bar}"] [role="tab"][aria-label="${name}"]`)
    ).click();
  }
}

/** The limits table where it stacks (#1859): its room, and each row's heading, boxes and words,
 *  measured; `null` where it is not drawn. */
async function theLimitsStacked() {
  return browser.execute(() => {
    const box = (el: Element) => {
      const { left, right, top, bottom } = el.getBoundingClientRect();
      return { left, right, top, bottom };
    };
    const table = document.querySelector('table[aria-label="Dispatch limits"]');
    const room = table?.closest(".ui-table-room");
    const head = table?.querySelector("thead");
    if (!table || !room || !head) return null;
    return {
      room: { ...box(room), width: room.clientWidth },
      scrolls: room.scrollWidth > room.clientWidth + 1,
      headings: getComputedStyle(head).display,
      rows: [...table.querySelectorAll("tbody tr")].map((tr) => {
        const cells = [...tr.querySelectorAll<HTMLElement>("td[data-heading]")].filter(
          (cell) => cell.childElementCount > 0,
        );
        const label = tr.querySelector("th");
        return {
          label: label ? box(label) : null,
          fields: cells.flatMap((cell) => [...cell.querySelectorAll("input")].map(box)),
          // What each cell says above its box, against the words it carries.
          words: cells.map((cell) => [
            getComputedStyle(cell, "::before").content,
            JSON.stringify(cell.dataset.heading ?? ""),
          ]),
          cut: [...tr.querySelectorAll<HTMLElement>("th, td")].some(
            (cell) => cell.scrollWidth > cell.clientWidth + 1,
          ),
        };
      }),
    };
  });
}

describe("the limits' lines and the set's native controls, in the real WebView", () => {
  let was = { width: 1280, height: 800 };
  /** The chat the time-limit case starts, by the number its start answered. */
  let mine: Started | undefined;

  before(async () => {
    was = await browser.getWindowSize();
    await windowIs(1280, 800);
  });

  after(async () => {
    const tried = async (step: () => Promise<unknown>) => {
      try {
        await step();
      } catch {
        // The next step still runs.
      }
    };
    await tried(() =>
      browser.execute(() =>
        window.dispatchEvent(new CustomEvent("purlis-e2e-tasks", { detail: [] })),
      ),
    );
    await tried(() => browser.keys("Escape"));
    await tried(() => browser.setWindowSize(was.width, was.height));
    // The chat this file started, ended by its number: the Settings tab it opened ends nothing.
    await tried(() => endChatsStartedHere([mine]));
  });

  it("draws the note under the limits table whole, inside its column, wide and narrow", async () => {
    await openProjectSettings();
    await goTo("Project", "Dispatch");
    await $(NOTE).waitForDisplayed({ timeout: 20_000 });

    for (const [width, height] of [
      [1280, 800],
      [900, 700],
    ] as const) {
      await windowIs(width, height);
      const seen = await theNote();
      expect(seen.text).toContain("At tokens per session, a session starts no new task");
      expect(seen.text).toContain("a harness that reports no tokens is not counted");
      const { note, body } = seen;
      if (note === null || body === null)
        throw new Error(`the note or its column is not drawn at ${width} px`);
      // Wrapped inside the column, never past it, and the page does not scroll sideways.
      expect(note.left).toBeGreaterThanOrEqual(body.left - 1);
      expect(note.right).toBeLessThanOrEqual(body.right + 1);
      expect(seen.cut).toBe(false);
      expect(seen.pageScrolls).toBe(false);
    }
    await windowIs(1280, 800);
  });

  /**
   * **The engine's own keys reach the native range, and nothing of ours eats them** (#626). What
   * can be checked through this suite's driver, and what cannot:
   *
   * - The embedded WebDriver (`tauri-plugin-wdio-webdriver` 1.4.0, `platform/executor.rs`)
   *   presses a key by dispatching a script-made `KeyboardEvent` at `document.activeElement`.
   *   Such an event is untrusted, and an engine runs no default action for one: a range never
   *   steps on it, on macOS and Linux alike (train 59's runs). The select is Radix's (#1860),
   *   whose keys are the app's own, so the driver's key does act there.
   *   The driver acts out only a radio's arrows and a text box's typing itself.
   * - So the spec checks what the app decides: the control holds the keyboard, the key arrives
   *   at it, and no handler of the window's prevents it or stops it on its way up. What the
   *   engine then does with a person's real key is the engine's own control.
   */
  async function aKeyReaches(control: string, key: string) {
    const before = await browser.execute((which: string) => {
      const el = document.querySelector<HTMLElement>(which);
      el?.focus();
      const seen = { atControl: false, atWindow: false, prevented: false };
      (window as unknown as Record<string, unknown>).__keySeen = seen;
      el?.addEventListener("keydown", () => (seen.atControl = true), { once: true });
      window.addEventListener(
        "keydown",
        (event) => {
          seen.atWindow = true;
          seen.prevented = event.defaultPrevented;
        },
        { once: true },
      );
      return { found: el !== null, focused: el !== null && document.activeElement === el };
    }, control);
    expect(before.found).toBe(true);
    expect(before.focused).toBe(true);
    await browser.keys([key]);
    return browser.execute(
      (which: string) => ({
        ...((window as unknown as Record<string, unknown>).__keySeen as {
          atControl: boolean;
          atWindow: boolean;
          prevented: boolean;
        }),
        stillFocused: document.activeElement === document.querySelector(which),
      }),
      control,
    );
  }

  it("stacks the limits into a form at the narrowest Settings: each row's level above its boxes, none cut (#1859)", async () => {
    // The narrowest Settings by default: the narrowest window (1024 px, ADR 0054) with both
    // sidebars open, whatever the specs before this one left, and put back after.
    const opened = await bothSidesOpen();
    try {
      await windowIs(1024, 768);
      await goTo("Project", "Dispatch");
      await $('table[aria-label="Dispatch limits"]').waitForDisplayed({ timeout: 20_000 });
      const seen = await theLimitsStacked();
      if (seen === null) throw new Error("the limits table is not drawn");
      // The room is under the width the table stacks at (29rem, at the default text size).
      expect(seen.room.width).toBeLessThanOrEqual(464);
      expect(seen.headings).toBe("none");
      expect(seen.scrolls).toBe(false);
      expect(seen.rows.length).toBeGreaterThanOrEqual(1);
      for (const row of seen.rows) {
        if (row.label === null) throw new Error("a row has no heading");
        expect(row.fields.length).toBeGreaterThanOrEqual(1);
        for (const field of row.fields) {
          // Under the row's level, and inside the room, give or take a pixel of rounding.
          expect(field.top).toBeGreaterThanOrEqual(row.label.bottom - 1);
          expect(field.left).toBeGreaterThanOrEqual(seen.room.left - 1);
          expect(field.right).toBeLessThanOrEqual(seen.room.right + 1);
        }
        // Each box under its limit's whole words, and nothing cut.
        for (const [said, carried] of row.words) expect(said).toBe(carried);
        expect(row.cut).toBe(false);
      }
    } finally {
      await putAwayAgain(opened);
      await windowIs(1280, 800);
    }
  });

  it("lets an arrow reach the select of Add an override, which opens its list on it (#626, #1860)", async () => {
    await goTo("Project", "Dispatch");
    await $(NOTE).waitForDisplayed({ timeout: 20_000 });
    // The override pick: the one select in the limits (`Choice kind="select"`), Radix's since
    // #1860. Its keys are the app's own (Radix's), not the engine's, so the driver's arrow acts
    // them out: the key reaches the select, travels on to the window, and opens the list.
    const select = '.dispatch-limits [role="combobox"]';
    const seen = await aKeyReaches(select, "ArrowDown");
    expect({ atControl: seen.atControl, atWindow: seen.atWindow }).toEqual({
      atControl: true,
      atWindow: true,
    });
    const list = await $('[role="listbox"]');
    await list.waitForDisplayed({ timeout: 10_000 });
    // The fixture project has two workspaces and two personas to offer.
    expect((await list.$$('[role="option"]')).length).toBeGreaterThanOrEqual(2);
    // Escape puts the list away, picks nothing, and gives the keyboard back to the select. A
    // pick writes nothing anyway: Add an override only drafts a row, and nothing is written
    // until a limit is typed into it.
    await browser.keys(["Escape"]);
    await list.waitForExist({ reverse: true, timeout: 10_000 });
    await browser.waitUntil(
      async () =>
        browser.execute(
          (which: string) => document.activeElement === document.querySelector(which),
          select,
        ),
      { timeout: 10_000, timeoutMsg: "the select did not take the keyboard back" },
    );
  });

  it("lets the arrow keys reach the native range of the text size (#626)", async () => {
    await goTo("You", "Text");
    const range = 'input[type="range"]';
    await $(range).waitForExist({ timeout: 20_000 });
    for (const key of ["ArrowRight", "ArrowLeft"]) {
      const seen = await aKeyReaches(range, key);
      expect({ key, ...seen }).toEqual({
        key,
        atControl: true,
        atWindow: true,
        prevented: false,
        stillFocused: true,
      });
    }
  });

  it("says a task purlis stopped at its time limit in its tab's menu, whole", async () => {
    mine = await startAChat();
    const asker = mine.session;
    await browser.waitUntil(
      async () =>
        browser.execute(
          (n: number, ready: string) =>
            (
              document.querySelector(`[data-testid="pane"][data-session="${n}"] .xterm-rows`)
                ?.textContent ?? ""
            ).includes(ready),
          asker,
          READY,
        ),
      { timeout: 30_000, interval: 250, timeoutMsg: `chat ${asker}'s pane never showed ${READY}` },
    );
    expect(asker).toBeGreaterThan(0);

    await browser.execute((of: number) => {
      window.dispatchEvent(
        new CustomEvent("purlis-e2e-tasks", {
          detail: [
            {
              workspace: "another-workspace",
              chat: {
                session: 9100,
                name: "9100",
                cwd: null,
                harness: "claude",
                in_front: false,
                resumed: null,
                fresh: null,
                profile: null,
                persona: null,
                unreported: null,
                card: null,
                guessed: null,
                pinned: false,
                label: "a task past its time",
                from: {
                  chat: of,
                  name: "its session",
                  workspace: "another-workspace",
                  task: true,
                  tab: false,
                  reported: true,
                  unreported: false,
                  outcome: "stopped_at_limit",
                },
              },
            },
          ],
        }),
      );
    }, asker);

    // The pointer rests on the chip, as a person's would (dispatched: see `tab-chip.e2e.ts`).
    await browser.waitUntil(
      async () =>
        browser.execute(
          (strip: string) => document.querySelector(`${strip} .tab-tasks-counts`) !== null,
          STRIP,
        ),
      { timeout: 10_000, timeoutMsg: "the tab never wore a chip for the pretended task" },
    );
    await browser.execute((strip: string) => {
      const counts = document.querySelector<HTMLElement>(`${strip} .tab-tasks-counts`);
      if (!counts) return;
      const at = counts.getBoundingClientRect();
      const send = (type: string, dx = 0) =>
        counts.dispatchEvent(
          new PointerEvent(type, {
            bubbles: true,
            cancelable: true,
            pointerId: 1,
            pointerType: "mouse",
            isPrimary: true,
            clientX: at.left + at.width / 2 + dx,
            clientY: at.top + at.height / 2,
          }),
        );
      send("pointerover");
      send("pointermove", 2);
    }, STRIP);
    await browser.waitUntil(
      async () =>
        browser.execute(() => document.querySelector('.tasks-menu[data-part="root"]') !== null),
      { timeout: 5_000, timeoutMsg: "resting on the chip opened no menu" },
    );
    // The finished task is in the fold: opened in place, as a click on it does.
    await browser.execute(() =>
      document
        .querySelector<HTMLElement>('.tasks-menu[data-part="root"] .tasks-menu-fold')
        ?.click(),
    );
    let seen: { words: string[]; cut: string[]; inside: boolean } = {
      words: [],
      cut: [],
      inside: false,
    };
    await browser.waitUntil(
      async () => {
        seen = await browser.execute(() => {
          const menu = document.querySelector<HTMLElement>('.tasks-menu[data-part="root"]');
          const words = [...(menu?.querySelectorAll<HTMLElement>(".shown-state .word") ?? [])];
          const at = menu?.getBoundingClientRect();
          return {
            words: words.map((word) => word.textContent ?? ""),
            cut: words
              .filter((word) => word.scrollWidth > word.clientWidth + 1)
              .map((word) => word.textContent ?? ""),
            inside:
              at !== undefined &&
              at.left >= 0 &&
              at.right <= window.innerWidth &&
              at.bottom <= window.innerHeight,
          };
        });
        return seen.words.includes("stopped at its time limit");
      },
      {
        timeout: 5_000,
        timeoutMsg: `the menu never said "stopped at its time limit": ${JSON.stringify(seen)}`,
      },
    );
    expect(seen.cut).toEqual([]);
    expect(seen.inside).toBe(true);
  });
});
