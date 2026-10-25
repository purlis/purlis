// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect, $$ } from "@wdio/globals";
import { READY } from "../harness.js";
import { pressAndStart } from "../opening.js";
import { chatsRunning, endTheChatsStartedSince, type Running } from "../chatsLeft.js";

/**
 * **A pane's warnings live inside the pane, and keep the tab joined** (#3142, the operator's
 * variant A of 2026-10-24), measured in the real engine against the built stylesheet.
 *
 * The operator's screenshots of 2026-10-24: a warning drawn between the chosen tab and its pane,
 * in an amber box that belonged to neither, and under it the pane's chips sitting on the
 * terminal's first lines, over the harness's own banner. Now a pane is three rows: the band of
 * warnings, the header with the chips and the pane's controls, then the terminal, which is
 * pinned to its bottom. Three scenarios: a pane with a warning, a split pane, a narrow pane.
 *
 * **What is raised is the shape, not the cause**, as `pane-notices.e2e.ts` raises it: a warning
 * is drawn into the pane's own stack with the elements and classes `Notice at="pane"` draws.
 * `PaneBand.test.tsx` holds the component to that shape.
 */

/** A long warning: a path and four hosts, longer than any pane is wide. */
const LONG =
  "The sandbox refused 4 connections while npm ci --prefer-offline --no-audit ran in /Users/aharon/Projects/purlis-plane/workspaces/smart-ide/clones/purlis/app/node_modules/.cache/purlis-e2e-fixtures/: registry.npmjs.org:443, objects.githubusercontent.com:443, codeload.github.com:443 and nodejs.org:443.";
const SHORT =
  "This chat started without 1 skill steward names, not installed on this machine: mattpocock-skills:grilling.";

type Box = {
  left: number;
  right: number;
  top: number;
  bottom: number;
  width: number;
  height: number;
};

async function tabNames(): Promise<string[]> {
  return browser.execute(() =>
    [
      ...(document.querySelector('[data-strip="Tabs"]')?.querySelectorAll('[role="tab"]') ?? []),
    ].map((tab) => tab.querySelector(".tab-name")?.textContent ?? ""),
  );
}

async function untilShows(index: number, text: string): Promise<void> {
  await browser.waitUntil(
    async () => {
      const panes = await $$('[data-testid="pane"]').getElements();
      const pane = panes[index];
      if (pane === undefined) return false;
      return (await pane.$(".xterm-rows").getText()).includes(text);
    },
    { timeout: 30_000, interval: 250, timeoutMsg: `pane ${index} never showed ${text}` },
  );
}

/** Draws a warning into pane `pane`'s band, as `Notice at="pane"` draws one, and waits until
 *  the band is drawn. */
async function raise(pane: number, sentence: string, ways: string[]): Promise<void> {
  const rowsBefore = (await measured(pane)).rowCount;
  const drawn = await browser.execute(
    (at: number, says: string, answers: string[]) => {
      const frames = [...document.querySelectorAll(".pane-frame")].filter((frame) =>
        frame.querySelector('[data-testid="pane"]'),
      );
      const stack = frames[at]?.querySelector(".pane-top .pane-notice-row > .pane-notices");
      if (!stack) return false;
      const el = [
        (tag: string, cls: string, text?: string): HTMLElement => {
          const made = document.createElement(tag);
          made.className = cls;
          if (text !== undefined) made.textContent = text;
          return made;
        },
      ][0];
      const box = el("div", "notice-pane-box ui-inline-surface ui-inline-surface-waiting");
      box.dataset.raised = "pane-warnings.e2e";
      box.setAttribute("data-asks", "");
      const line = el("div", "notice notice-pane");
      Object.assign(line.dataset, { scope: "button-row", size: "sm", wraps: "" });
      line.setAttribute("role", "status");
      line.append(el("div", "notice-says", says));
      for (const way of answers) {
        const button = el("button", way === "Dismiss" ? "notice-dismiss" : "notice-fix");
        Object.assign(button.dataset, {
          scope: "button",
          part: "root",
          variant: way === "Dismiss" ? "ghost" : "secondary",
          size: "sm",
        });
        const label = el("span", "", way);
        Object.assign(label.dataset, { scope: "button", part: "label" });
        button.append(label);
        button.setAttribute("type", "button");
        line.append(button);
      }
      box.append(line);
      stack.prepend(box);
      return true;
    },
    pane,
    sentence,
    ways,
  );
  expect(drawn).toBe(true);
  await browser.waitUntil(
    () =>
      browser.execute((at: number) => {
        const frames = [...document.querySelectorAll(".pane-frame")].filter((frame) =>
          frame.querySelector('[data-testid="pane"]'),
        );
        const row = frames[at]?.querySelector(".pane-notice-row");
        return row !== null && row !== undefined && getComputedStyle(row).display !== "none";
      }, pane),
    { timeout: 10_000, interval: 50, timeoutMsg: `pane ${pane} never drew its band` },
  );
  // The terminal is fitted to the room the band left before anything is measured.
  await untilRefitted(pane, rowsBefore);
}

async function lower(): Promise<void> {
  await browser.execute(() => {
    for (const one of document.querySelectorAll('[data-raised="pane-warnings.e2e"]')) one.remove();
  });
}

/** Every box this file needs of pane `pane`. */
async function measured(pane: number) {
  return browser.execute((at: number) => {
    const box = [
      (el: Element | null | undefined) => {
        if (!el) return null;
        const { left, right, top, bottom, width, height } = el.getBoundingClientRect();
        return { left, right, top, bottom, width, height };
      },
    ][0];
    const frames = [...document.querySelectorAll(".pane-frame")].filter((frame) =>
      frame.querySelector('[data-testid="pane"]'),
    );
    const frame = frames[at];
    const rows = frame?.querySelector(".xterm-rows");
    const tab = document.querySelector(
      '[data-scope="tabs"][data-part="tab"][data-variant="chat"][data-selected]',
    );
    const layout = document.querySelector('[data-scope="tabs"][data-part="layout"]');
    const raised = [...(frame?.querySelectorAll('[data-raised="pane-warnings.e2e"]') ?? [])];
    return {
      frame: box(frame),
      frameFill: frame ? getComputedStyle(frame).backgroundColor : "",
      band: box(frame?.querySelector(".pane-notice-row")),
      head: box(frame?.querySelector(".pane-head")),
      chips: [...(frame?.querySelectorAll(".pane-head > .pane-chips > *") ?? [])].map(box),
      screen: box(frame?.querySelector(".xterm-screen")),
      // What the terminal draws on: its pane, which clips the grid until it is fitted.
      terminal: box(frame?.querySelector('[data-testid="pane"]')),
      lastRow: box(rows?.lastElementChild),
      rowCount: rows?.children.length ?? 0,
      tab: box(tab),
      tabFill: tab ? getComputedStyle(tab).backgroundColor : "",
      joined: layout ? getComputedStyle(layout).getPropertyValue("--tabs-joined").trim() : "",
      warnings: raised.map((one) => ({
        box: box(one),
        says: box(one.querySelector(".notice-says")),
        buttons: [...one.querySelectorAll("button")].map(box),
      })),
    };
  }, pane);
}

function check(what: string, holds: boolean, seen: unknown) {
  if (!holds) throw new Error(`${what}: ${JSON.stringify(seen)}`);
}

function inside(inner: Box | null, outer: Box | null, what: string) {
  check(`${what}: nothing drawn`, inner !== null && outer !== null, { inner, outer });
  const [a, b] = [inner as Box, outer as Box];
  check(
    `${what} is not inside`,
    a.left >= b.left - 1 &&
      a.right <= b.right + 1 &&
      a.top >= b.top - 1 &&
      a.bottom <= b.bottom + 1,
    { a, b },
  );
}

/** Nothing of the band or the header is drawn over the terminal (rule 3): the box the terminal
 *  draws in, which clips its grid while a refit is on its way. */
function offTheTerminal(seen: Awaited<ReturnType<typeof measured>>) {
  const screen = seen.terminal as Box;
  check("the band runs onto the terminal", (seen.band?.bottom ?? 0) <= screen.top + 1, seen);
  check("the header runs onto the terminal", (seen.head as Box).bottom <= screen.top + 1, seen);
  for (const chip of seen.chips)
    check("a chip is over the terminal", (chip as Box).bottom <= screen.top + 1, { chip, screen });
}

/** Waits until the terminal of pane `pane` is fitted to the room it has: its rows changed from
 *  `was`, or a second went by with no change to wait for, and then held still for three reads
 *  in a row. A band that comes or goes can lay out in more than one frame (its last Notice
 *  gone, then the row's own room), and a fit read between the two is not the room it ends with. */
async function untilRefitted(pane: number, was: number): Promise<void> {
  await browser
    .waitUntil(async () => (await measured(pane)).rowCount !== was, {
      timeout: 2_000,
      interval: 100,
    })
    .catch(() => undefined);
  let last = (await measured(pane)).rowCount;
  let still = 0;
  await browser
    .waitUntil(
      async () => {
        const now = (await measured(pane)).rowCount;
        still = now === last ? still + 1 : 0;
        last = now;
        return still >= 3;
      },
      { timeout: 3_000, interval: 100 },
    )
    .catch(() => undefined);
}

describe("a pane's warnings, inside the pane", () => {
  let wereAlreadyOpen: string[] = [];
  let chatsBefore: Running = {};
  let was = { width: 1280, height: 800 };

  before(async () => {
    wereAlreadyOpen = await tabNames();
    chatsBefore = await chatsRunning();
    was = await browser.getWindowSize();
    await pressAndStart("New tab");
    await untilShows(0, READY);
  });

  afterEach(lower);

  after(async () => {
    await browser.setWindowSize(was.width, was.height);
    await endTheChatsStartedSince(chatsBefore);
    await browser.waitUntil(
      async () => (await tabNames()).every((tab) => wereAlreadyOpen.includes(tab)),
      { timeout: 20_000, timeoutMsg: "the pane-warnings spec left a chat open behind it" },
    );
  });

  async function windowIs(width: number, height: number) {
    await browser.setWindowSize(width, height);
    let last = -1;
    await browser.waitUntil(
      async () => {
        const now = await browser.execute(() => window.innerWidth);
        const settled = now === last;
        last = now;
        return settled;
      },
      { timeout: 20_000, interval: 250, timeoutMsg: "the window never stopped resizing" },
    );
  }

  it("keeps the chosen tab joined to its pane while a warning shows (rules 1 and 2)", async () => {
    await windowIs(1280, 800);
    await raise(0, SHORT, ["Open in Skill studio", "Dismiss"]);
    const seen = await measured(0);
    const frame = seen.frame as Box;

    // The band, the header, then the terminal: all inside the pane.
    inside(seen.band, frame, "the band");
    inside(seen.head, frame, "the header");
    inside(seen.warnings[0].box, frame, "the warning");
    check(
      "the header is not under the band",
      (seen.head as Box).top >= (seen.band as Box).bottom - 1,
      seen,
    );
    offTheTerminal(seen);

    // The runway: the warning starts below the pane's top, on the pane's own fill.
    check(
      "the warning touches the pane's top",
      (seen.warnings[0].box as Box).top >= frame.top + 6,
      seen,
    );

    // Docked (Purlis): the chosen tab's bottom is the pane's top, in the same fill.
    if (seen.joined === "1") {
      check(
        "the tab does not meet its pane",
        Math.abs((seen.tab as Box).bottom - frame.top) <= 1,
        seen,
      );
      expect(seen.tabFill).toBe(seen.frameFill);
    }
  });

  it("keeps the terminal's last line where it was as a warning comes and goes (rule 4)", async () => {
    await windowIs(1280, 800);
    const before = await measured(0);
    check("no terminal grid", before.lastRow !== null && before.screen !== null, before);

    await raise(0, SHORT, ["Open in Skill studio", "Dismiss"]);
    await untilRefitted(0, before.rowCount);
    const shown = await measured(0);
    // Fitted to the room left: fewer rows, the grid under the header, its bottom where it was.
    check("the terminal kept every row", shown.rowCount < before.rowCount, {
      before: before.rowCount,
      shown: shown.rowCount,
    });
    offTheTerminal(shown);
    check(
      "the last line moved when the warning came",
      Math.abs((shown.lastRow as Box).bottom - (before.lastRow as Box).bottom) <= 1,
      { before: before.lastRow, shown: shown.lastRow },
    );

    await lower();
    await untilRefitted(0, shown.rowCount);
    const gone = await measured(0);
    check("the room was not given back", gone.rowCount === before.rowCount, {
      before: before.rowCount,
      gone: gone.rowCount,
    });
    check(
      "the last line moved when the warning went",
      Math.abs((gone.lastRow as Box).bottom - (before.lastRow as Box).bottom) <= 1,
      { before: before.lastRow, gone: gone.lastRow },
    );
  });

  it("gives each pane of a split its own band and header, lined up (split pane)", async () => {
    await windowIs(1600, 1000);
    if ((await $$('[data-testid="pane"]').getElements()).length < 2) {
      await pressAndStart("Split right");
      await untilShows(1, READY);
    }
    await raise(0, SHORT, ["Open in Skill studio", "Dismiss"]);
    const [left, right] = [await measured(0), await measured(1)];

    // The warning is in its own pane only, and the other pane's header is at the top of its pane.
    inside(left.warnings[0].box, left.frame, "the warning in its pane");
    expect(right.warnings).toHaveLength(0);
    check(
      "the panes are not side by side",
      (left.frame as Box).right <= (right.frame as Box).left + 1,
      { left: left.frame, right: right.frame },
    );
    inside(right.head, right.frame, "the other pane's header");
    check(
      "the other pane's header is not at its top",
      (right.head as Box).top - (right.frame as Box).top <= 12,
      right,
    );
    offTheTerminal(left);
    offTheTerminal(right);
  });

  it("wraps a long warning in place in a narrow pane, its ways out under its words (narrow pane)", async () => {
    await windowIs(1024, 768);
    if ((await $$('[data-testid="pane"]').getElements()).length < 2) {
      await pressAndStart("Split right");
      await untilShows(1, READY);
    }
    await raise(1, LONG, ["Show what you can do", "Dismiss"]);
    const seen = await measured(1);
    const frame = seen.frame as Box;
    const [warning] = seen.warnings;

    inside(warning.box, frame, "the long warning in its narrow pane");
    for (const button of warning.buttons) {
      inside(button, warning.box, "a way out in its warning");
      check(
        "a way out is beside the words",
        (button as Box).top >= (warning.says as Box).bottom - 1,
        { button, says: warning.says },
      );
    }
    offTheTerminal(seen);
    // Nothing was scrolled sideways by what the warning holds.
    const sideways = await browser.execute(() => {
      const all: number[] = [];
      for (
        let el: Element | null = document.querySelector(".pane-notice-row");
        el;
        el = el.parentElement
      )
        all.push(Math.round(el.scrollLeft));
      all.push(Math.round(window.scrollX));
      return all;
    });
    expect(sideways.filter((by) => by !== 0)).toEqual([]);
  });
});
