// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect, $$ } from "@wdio/globals";
import { READY } from "../harness.js";
import { endChatsStartedHere, startAChat, type Started } from "../started.js";
import { noDialogIsLeftUp } from "../opening.js";

/**
 * **Focus mode on the real window** (#2921, spec #2897 N32): ⌘⇧↩ (Ctrl+Shift+Enter off a Mac)
 * hides the project rail, the activity columns, the side panels, the Panel and the status bar,
 * and the work takes their room. The same key brings every one of them back at the size it had.
 * The terminal in front is not drawn again: the same element with the same text.
 *
 * `keys/focusMode.test.tsx` and `RegionFrame.drawn.test.tsx` hold the toggle and F6 against
 * jsdom; only a window that lays out can say what is drawn and where.
 *
 * **The key is dispatched from the page**, as `panel-toggle.e2e.ts` does a chord, since the
 * driver does not hold a modifier across `browser.keys`; the keyboard is taken off any chat
 * first. **It leaves the window as it found it**: focus mode off, whatever an assertion did.
 */

const MAC = process.platform === "darwin";
const FRAME = ".region-frame[data-cycle]";
const WINDOW = "main.window";
/** A modal dialog as the key registry finds one (`keys/contexts.ts`): read here, never imported. */
const MODAL = '[data-scope="dialog"][data-part="root"], [role="alertdialog"]';

/** What focus mode hides, as `keys/focusMode.ts` names it: read here, never imported. */
const HIDDEN = {
  rail: '[data-scope="rail"][data-part="slot"]',
  columns: ".region-frame > .activity-column",
  left: ".region-slot.slot-left",
  right: ".region-slot.slot-right",
  panel: ".region-slot.slot-panel",
  status: 'footer[data-scope="status-bar"]',
};

/** Each part's box as drawn: its width and height, or `null` where it is not drawn. */
type Boxes = Record<keyof typeof HIDDEN, { width: number; height: number } | null>;

async function boxes(): Promise<Boxes> {
  return browser.execute((parts: Record<string, string>) => {
    const out: Record<string, { width: number; height: number } | null> = {};
    for (const [name, selector] of Object.entries(parts)) {
      const all = [...document.querySelectorAll<HTMLElement>(selector)];
      const drawn = all.filter((one) => one.getClientRects().length > 0);
      out[name] =
        drawn.length === 0
          ? null
          : drawn
              .map((one) => one.getBoundingClientRect())
              .reduce(
                (sum, box) => ({
                  width: Math.round(sum.width + box.width),
                  height: Math.round(sum.height + box.height),
                }),
                { width: 0, height: 0 },
              );
    }
    return out;
  }, HIDDEN) as Promise<Boxes>;
}

/** A part's box as one word, whatever order its sizes came in. */
const said = (box: { width: number; height: number } | null) =>
  box === null ? "none" : `${box.width}x${box.height}`;

/** Whether two readings draw every part at one size. */
const alike = (a: Boxes, b: Boxes) =>
  (Object.keys(HIDDEN) as (keyof typeof HIDDEN)[]).every((name) => said(a[name]) === said(b[name]));

/** The parts' boxes once two readings half a second apart agree: the window at rest. */
async function boxesAtRest(): Promise<Boxes> {
  let last = await boxes();
  for (let look = 0; look < 20; look++) {
    await browser.pause(500);
    const now = await boxes();
    if (alike(now, last)) return now;
    last = now;
  }
  return last;
}

/** Waits for each part to be at the size it had, and says which is not when one stays apart. */
async function untilTheSame(before: Boxes, failure: string): Promise<void> {
  let now = before;
  const same = await browser
    .waitUntil(
      async () => {
        now = await boxes();
        return alike(now, before);
      },
      { timeout: 10_000, interval: 250 },
    )
    .then(() => true)
    .catch(() => false);
  if (same) return;
  const apart = (Object.keys(HIDDEN) as (keyof typeof HIDDEN)[])
    .filter((name) => said(now[name]) !== said(before[name]))
    .map((name) => `${name} ${said(before[name])} → ${said(now[name])}`);
  throw new Error(`${failure}: ${apart.join("; ")}`);
}

/** Whether the window is in focus mode. */
async function inFocusMode(): Promise<boolean> {
  return browser.execute(
    (root: string) => document.querySelector(root)?.hasAttribute("data-focus-mode") === true,
    WINDOW,
  );
}

/** The editor region's width. */
async function workWidth(): Promise<number> {
  return browser.execute(
    () => document.querySelector(".region-centre")?.getBoundingClientRect().width ?? 0,
  );
}

/** Focus mode's key, from the page, until the window is `want`. */
async function focusModeTo(want: boolean): Promise<void> {
  // A modal keeps the window's keys (the operator's ruling): none is left up for this, and one
  // still leaving (its exit drawn) is waited out.
  await noDialogIsLeftUp();
  await browser.waitUntil(
    () => browser.execute((modal: string) => document.querySelector(modal) === null, MODAL),
    { timeout: 10_000, interval: 150, timeoutMsg: "a modal stayed up before focus mode's key" },
  );
  // One press: a press that lands late must never meet a second, which would turn it back.
  // What the page said as it was pressed, for the failure: whether a handler claimed the key
  // (`defaultPrevented`), whether a modal was up at that moment, where the keyboard was.
  const pressed = await browser.execute(
    (mac: boolean, modal: string) => {
      const on = document.activeElement;
      if (on instanceof HTMLElement) on.blur();
      const at = document.activeElement?.tagName ?? "none";
      const modalUp = document.querySelector(modal) !== null;
      // Every change of the window's focus-mode marks after the press, for the failure: a
      // toggle that went on and back off reads apart from one that never went on.
      const root = document.querySelector("main.window");
      const seen: string[] = [];
      (window as unknown as { focusModeSeen: string[] }).focusModeSeen = seen;
      if (root !== null)
        new MutationObserver((changes) => {
          for (const one of changes)
            seen.push(`${one.attributeName} was ${one.oldValue === null ? "absent" : "set"}`);
        }).observe(root, {
          attributes: true,
          attributeOldValue: true,
          attributeFilter: ["data-focus-mode", "data-focus-back"],
        });
      const already = root?.hasAttribute("data-focus-mode") === true;
      const claimed = !document.body.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Enter",
          code: "Enter",
          shiftKey: true,
          metaKey: mac,
          ctrlKey: !mac,
          bubbles: true,
          cancelable: true,
        }),
      );
      const windows = document.querySelectorAll("main.window").length;
      return `in focus mode ${already} before, claimed ${claimed}, a modal ${modalUp ? "up" : "not up"} as it was pressed, the keyboard on ${at}, ${windows} window(s)`;
    },
    MAC,
    MODAL,
  );
  const landed = await browser
    .waitUntil(async () => (await inFocusMode()) === want, { timeout: 10_000, interval: 150 })
    .then(() => true)
    .catch(() => false);
  if (landed) return;
  // What held the key back, said in the failure: the modal up, if one is.
  const held = await browser.execute((modal: string) => {
    const up = document.querySelector<HTMLElement>(modal);
    const seen = (window as unknown as { focusModeSeen?: string[] }).focusModeSeen ?? [];
    const marks = `the marks changed: ${seen.length === 0 ? "never" : seen.join(" ")}`;
    return up === null
      ? `no modal up now; ${marks}`
      : `a modal up now: ${up.getAttribute("aria-label") ?? up.dataset.testid ?? up.className}; ${marks}`;
  }, MODAL);
  throw new Error(
    `${MAC ? "⌘⇧↩" : "Ctrl+Shift+Enter"} did not turn focus mode ${want ? "on" : "off"} (${pressed}; ${held})`,
  );
}

/** The first pane's terminal: marked, so the same element can be found again; and its text. */
async function terminalNow(): Promise<{ marked: boolean; text: string }> {
  return browser.execute(() => {
    const rows = document.querySelector<HTMLElement>('[data-testid="pane"] .xterm-rows');
    if (rows === null) return { marked: false, text: "" };
    const marked = rows.dataset.focusModeProbe === "kept";
    rows.dataset.focusModeProbe = "kept";
    return { marked, text: rows.textContent ?? "" };
  });
}

describe("focus mode", function () {
  this.timeout(180_000);

  /** The chat this spec started, where the window had none in front with a terminal. */
  let started: Started | undefined;

  /** Whether the pane in front shows the harness's ready line. */
  const aPaneIsReady = async () => {
    const panes = await $$('[data-testid="pane"]').getElements();
    if (panes.length === 0) return false;
    return (await panes[0].$(".xterm-rows").getText()).includes(READY);
  };

  before(async () => {
    await browser.$(FRAME).waitForExist({ timeout: 30_000 });
    const ready = await browser
      .waitUntil(aPaneIsReady, { timeout: 5_000, interval: 250 })
      .then(() => true)
      .catch(() => false);
    // A terminal to watch: the window's own, else a chat of this spec's, ended after.
    if (!ready) started = await startAChat();
    await browser.waitUntil(aPaneIsReady, {
      timeout: 60_000,
      interval: 250,
      timeoutMsg: `no pane showed ${READY}`,
    });
    if (await inFocusMode()) await focusModeTo(false);
  });

  after(async () => {
    if (await inFocusMode()) await focusModeTo(false);
    await endChatsStartedHere([started]);
  });

  it("hides the rail, the bars and the panels, and brings each back at its size", async () => {
    const before = await boxesAtRest();
    const widthBefore = await workWidth();
    const terminal = await terminalNow();
    expect(terminal.text).toContain(READY);

    await focusModeTo(true);
    const hidden = await boxes();
    for (const name of Object.keys(HIDDEN) as (keyof typeof HIDDEN)[])
      expect({ [name]: hidden[name] }).toEqual({ [name]: null });
    // The work has the room they left.
    expect(await workWidth()).toBeGreaterThan(widthBefore);
    // The terminal is the same element, still showing what it showed: wider, its lines may wrap
    // differently, so its words are compared once it is back at its width.
    const during = await terminalNow();
    expect(during.marked).toBe(true);
    expect(during.text).toContain(READY);

    await focusModeTo(false);
    // Each part is back at the size it had: nothing was written or resized.
    await untilTheSame(before, "the window's parts did not come back at the sizes they had");
    expect(Math.abs((await workWidth()) - widthBefore)).toBeLessThanOrEqual(1);
    await browser.waitUntil(async () => (await terminalNow()).text === terminal.text, {
      timeout: 10_000,
      timeoutMsg: "the terminal's text is not what it was before focus mode",
    });
    expect((await terminalNow()).marked).toBe(true);
  });

  it("leaves focus mode on a region's key, and changes no region out of sight", async () => {
    const before = await boxesAtRest();
    await focusModeTo(true);
    // ⌘B (Ctrl+B off a Mac) in focus mode only leaves it (D-2921-19).
    await browser.execute((mac: boolean) => {
      document.body.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "b",
          code: "KeyB",
          metaKey: mac,
          ctrlKey: !mac,
          bubbles: true,
          cancelable: true,
        }),
      );
    }, MAC);
    await browser.waitUntil(async () => !(await inFocusMode()), {
      timeout: 10_000,
      timeoutMsg: "the left sidebar's key did not leave focus mode",
    });
    await untilTheSame(before, "a region changed while focus mode hid it");
  });
});
