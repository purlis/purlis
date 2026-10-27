// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect, $, $$ } from "@wdio/globals";
import { READY } from "../harness.js";
import { pressAndStart, pressOnly } from "../opening.js";
import { chatsRunning, endTheChatsStartedSince, type Running } from "../chatsLeft.js";

/**
 * **A Notice in a pane fits its pane, measured** (#1481), **in a row of its own above the
 * terminal, never over it, two at a time** (#1647).
 *
 * The operator, from the dev build with two panes side by side: *"ui is broken for questions
 * modals"*. In a narrow pane a Notice's sentence was one word wide and fourteen lines tall, its
 * buttons each wrapped to five lines beside it, the block a button opened was drawn to the
 * right of the Notice and off the window, the Notice was drawn over an open dialog, and the
 * window itself was scrolled sideways by what overflowed it, so the band and the sidebar were
 * cut at the left.
 *
 * All of that is layout, and jsdom lays nothing out, so it is measured here: in the real
 * engine, against the built stylesheet, in real panes of a real split.
 *
 * Then the operator's screenshot of 2026-10-10: three Notices stacked in the pane's corner, over
 * the terminal, and hid the conversation (#1647). The Notices have the pane's top row now, the
 * terminal is under it, and past two the rest are behind "+N more". `NoticePaneRow` hides what
 * is past two whoever drew it, so the Notices this file raises are stacked as real ones are.
 *
 * **What is raised is the shape, not the cause.** A vault refusal and a held dispatch need a
 * persona chat and a core holding them, which this suite's fake harness has no way to make. So
 * a Notice is drawn into the pane's own stack (`[data-scope=notice-row][data-part=stack]`, which `PaneFrame` draws for
 * every pane) with the elements and classes `Notice at="pane"` draws, and the dialog with the
 * ones `AskPersona` draws. `Notice.pane.test.tsx` and `AskPersona.window.test.tsx` hold the
 * components to those shapes; this file holds the stylesheet to what they must look like. A
 * class renamed in one place and not the other fails one of the two.
 */

/** The sentence and the ways out of the Notice in the operator's screenshot. */
const SENTENCE =
  "This chat runs as steward, and vault devops is tagged for devops, so purlis did not open it. This chat has already asked devops: answer that above.";
const WAYS = ["Allow steward to use this vault", "Dispatch to devops…", "Keep blocked"];
/** The dispatch grant Notice's own sentence and its five answers (#1503), in the order drawn. */
const ASKS =
  "This chat runs as steward and wants to dispatch to devops. Nothing starts until you answer. Allowing it lets steward chats ask devops for anything devops can do, without asking you again.";
const FIVE_WAYS = [
  "Allow for this chat",
  "Allow for me on this machine",
  "Allow for everyone in this project",
  "Keep blocked",
  "Never for this pair",
];
/**
 * What a dispatch's question says under its answers (#1502): a box for each persona the asking
 * one wants, as many as purlis ever offers, each with what it works with, and what the target
 * works with. The hosts are the long unbroken words a real project declares.
 */
const ALSO = {
  label: "Also let steward dispatch to:",
  boxes: ["qa", "docs", "legal", "release-engineering", "ops", "observability"].map((name) => ({
    name,
    says: `vault ${name}; hosts ${name}.a-rather-long-internal-zone.corp.example:6443, *.${name}.internal.example and 14 more`,
  })),
  access:
    "devops works with its own access: vaults deploy, prod-cluster, registry and 2 more; hosts 10.100.39.145:6443, *.a-rather-long-internal-zone.corp.example, registry.internal.example:5000 and 37 more.",
  secret: "Allowing a dispatch does not allow the use of a secret: that is asked as before.",
};
type Also = typeof ALSO;

/** A brief of 46 lines, some of them far longer than any pane is wide. */
const BRIEF = Array.from({ length: 46 }, (_, line) =>
  line % 5 === 0
    ? `${line} ${"an_unbroken_token_that_is_wider_than_a_pane_".repeat(8)}`
    : `${line} **Goal.** A READ-ONLY verification of prod release v2.48.0, with numbers.`,
).join("\n");

type Box = {
  left: number;
  right: number;
  top: number;
  bottom: number;
  width: number;
  height: number;
};

/** The tabs on the strip, left to right, as `pane-fill.e2e.ts` reads them. */
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

/**
 * Draws a Notice into pane `pane`'s stack, as `Notice at="pane"` draws one: the box, the line
 * (a `status`) with the sentence and one button per way out, and, where `opened`, what a way
 * out opened under it. Answers whether the pane had a stack to draw it into.
 *
 * **Then waits for the pane's row to be drawn** (#1647): a row with nothing in it is not drawn,
 * and the row learns it has a Notice from its stack (`NoticePaneRow`'s observer) and draws
 * itself on React's next render, not in the same task as the append.
 */
async function raise(
  pane: number,
  notice: { sentence: string; ways: string[]; opened?: string; also?: Also },
): Promise<boolean> {
  const drawn = await browser.execute(
    (at: number, sentence: string, ways: string[], opened: string | null, also: Also | null) => {
      const frames = [...document.querySelectorAll(".pane-frame")].filter((frame) =>
        frame.querySelector('[data-testid="pane"]'),
      );
      const stack = frames[at]?.querySelector(
        "[data-scope=notice-row][data-part=root] > [data-scope=notice-row][data-part=stack]",
      );
      if (!stack) return false;
      // Held in a list, not bound by name: the spec's bundler wraps a named function in a
      // `__name` call the page does not have (`gauge.e2e.ts`, `view-tabs.e2e.ts`).
      const el = [
        (tag: string, cls: string, text?: string): HTMLElement => {
          const made = document.createElement(tag);
          made.className = cls;
          if (text !== undefined) made.textContent = text;
          return made;
        },
      ][0];
      // The notice family's box in a pane (`ui/notice`, #2912), and a line that is the shared
      // row of buttons that wraps (#1855), as `Notice` draws them.
      const box = el("div", "");
      Object.assign(box.dataset, {
        scope: "notice",
        part: "root",
        variant: "trouble",
        placement: "pane",
      });
      if (opened !== null) box.setAttribute("data-opened", "");
      box.dataset.raised = "pane-notices.e2e";
      // As `Notice` marks one with an answer besides Dismiss: what the pane's row stands first
      // (#1647), so a Notice this file raises is one of the two that stand.
      if (ways.some((way) => way !== "Dismiss")) box.setAttribute("data-asks", "");
      // The line is a small `ButtonRow` of the button family's buttons (#2904).
      const line = el("div", "");
      Object.assign(line.dataset, { scope: "button-row", size: "sm", wraps: "" });
      line.setAttribute("role", "status");
      const says = el("div", "", sentence);
      Object.assign(says.dataset, { scope: "notice", part: "message" });
      line.append(says);
      for (const way of ways) {
        const button = el("button", "");
        button.dataset.way = way === "Dismiss" ? "dismiss" : "fix";
        Object.assign(button.dataset, {
          scope: "button",
          part: "root",
          variant: "secondary",
          size: "sm",
        });
        const label = el("span", "", way);
        Object.assign(label.dataset, { scope: "button", part: "label" });
        button.append(label);
        button.setAttribute("type", "button");
        button.tabIndex = 0;
        line.append(button);
      }
      box.append(line);
      if (opened !== null) {
        const under = el("div", "");
        Object.assign(under.dataset, { scope: "notice", part: "under" });
        if (also !== null) {
          // As `DispatchGrantNotice` draws it, above the brief: the house `Choice` of checks,
          // each the checkbox family's box in its own label (#2907), then the access line and
          // the line about secrets.
          const besides = el("div", "dispatch-also");
          const checks = el("div", "");
          checks.setAttribute("role", "group");
          checks.dataset.scope = "checkbox-group";
          for (const [index, one] of also.boxes.entries()) {
            const option = el("label", "");
            Object.assign(option.dataset, { scope: "checkbox", part: "root", variant: "box" });
            const input = document.createElement("input");
            input.type = "checkbox";
            input.id = `pane-notices-e2e-also-${index}`;
            const hidden = el("span", "sr-only");
            hidden.append(input);
            const mark = el("span", "");
            Object.assign(mark.dataset, { scope: "checkbox", part: "box" });
            const name = el("span", "", one.name);
            Object.assign(name.dataset, { scope: "checkbox", part: "label" });
            const says = el("span", "", one.says);
            Object.assign(says.dataset, { scope: "checkbox", part: "description" });
            option.append(hidden, mark, name, says);
            checks.append(option);
          }
          besides.append(
            el("p", "", also.label),
            checks,
            el("p", "dispatch-works-with", also.access),
            el("p", "", also.secret),
          );
          under.append(besides);
        }
        const report = el("div", "block-report");
        report.append(
          el("p", "", "The brief, as the chat wrote it. purlis did not write it."),
          el("pre", "block-report-draft block-report-brief", opened),
          el("p", "", "The brief is 46 lines. Scroll its box to read all of it before you answer."),
        );
        under.append(report);
        box.append(under);
      }
      // Before any Notice the app itself has in this pane (a chat on Linux can have its own),
      // after the ones this file raised: the pane's order is this file's, and what it measures
      // stands rather than waiting behind "+N more".
      const theApps = [...stack.children].find(
        (one) => (one as HTMLElement).dataset.raised !== "pane-notices.e2e",
      );
      stack.insertBefore(box, theApps ?? null);
      return true;
    },
    pane,
    notice.sentence,
    notice.ways,
    notice.opened ?? null,
    notice.also ?? null,
  );
  if (!drawn) return false;
  await browser.waitUntil(
    () =>
      browser.execute((at: number) => {
        const frames = [...document.querySelectorAll(".pane-frame")].filter((frame) =>
          frame.querySelector('[data-testid="pane"]'),
        );
        const row = frames[at]?.querySelector("[data-scope=notice-row][data-part=root]");
        return row !== null && row !== undefined && getComputedStyle(row).display !== "none";
      }, pane),
    { timeout: 10_000, interval: 50, timeoutMsg: `pane ${pane} never drew its row of Notices` },
  );
  return true;
}

/** Takes away everything this file drew. */
async function lower(): Promise<void> {
  await browser.execute(() => {
    for (const one of document.querySelectorAll('[data-raised="pane-notices.e2e"]')) one.remove();
  });
}

/** Every box this file needs of pane `pane` and the Notices raised in it. */
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
    const stack = frame?.querySelector<HTMLElement>("[data-scope=notice-row][data-part=stack]");
    const notices = [...(frame?.querySelectorAll('[data-raised="pane-notices.e2e"]') ?? [])].map(
      (raised) => {
        const line = raised.querySelector("[data-placement=pane] > [data-scope=button-row]");
        const pre = raised.querySelector<HTMLElement>("pre");
        return {
          box: box(raised),
          line: box(line),
          says: box(raised.querySelector("[data-scope=notice][data-part=message]")),
          buttons: [...(line?.querySelectorAll("button") ?? [])].map((button) => {
            const css = getComputedStyle(button);
            const lineHeight = Number.parseFloat(css.lineHeight);
            return {
              label: button.textContent ?? "",
              box: box(button),
              // Words cut off inside the button rather than wrapped (#1855).
              clipped:
                button.scrollWidth > button.clientWidth + 1 ||
                button.scrollHeight > button.clientHeight + 1,
              // One line of the button's text, as the explorer's rows are measured.
              line: Number.isFinite(lineHeight)
                ? lineHeight
                : Number.parseFloat(css.fontSize) * 1.5,
              chrome:
                Number.parseFloat(css.paddingTop) +
                Number.parseFloat(css.paddingBottom) +
                Number.parseFloat(css.borderTopWidth) +
                Number.parseFloat(css.borderBottomWidth),
            };
          }),
          under: box(raised.querySelector("[data-scope=notice][data-part=under]")),
          also: box(raised.querySelector(".dispatch-also")),
          options: [
            ...raised.querySelectorAll('.dispatch-also [data-scope="checkbox"][data-part="root"]'),
          ].map((option) => ({
            option: box(option),
            box: box(option.querySelector('[data-part="box"]')),
            label: box(option.querySelector('[data-part="label"]')),
            says: box(option.querySelector('[data-part="description"]')),
          })),
          access: box(raised.querySelector(".dispatch-works-with")),
          report: box(raised.querySelector(".block-report")),
          pre: box(pre),
          preScrolls: pre ? pre.scrollHeight > pre.clientHeight + 1 : false,
          background: getComputedStyle(raised).backgroundColor,
          shown: (raised as HTMLElement).offsetParent !== null,
        };
      },
    );
    return {
      window: { width: window.innerWidth, height: window.innerHeight },
      rem: Number.parseFloat(getComputedStyle(document.documentElement).fontSize),
      frame: box(frame),
      neighbours: frames.filter((one) => one !== frame).map((one) => box(one)),
      row: box(frame?.querySelector("[data-scope=notice-row][data-part=root]")),
      terminal: box(frame?.querySelector('[data-testid="pane"]')),
      more: box(frame?.querySelector("[data-scope=notice-row][data-part=root] > [data-way=more]")),
      // What it says: of its two labels, the one not hidden (#3142).
      moreSays:
        frame?.querySelector(
          "[data-scope=notice-row][data-part=root] > [data-way=more] [data-scope=notice-row][data-part=says] > :not([data-off])",
        )?.textContent ?? null,
      stack: box(stack),
      // The stack's width inside its scrollbar, which a card takes whole.
      stackInside: stack ? stack.clientWidth : 0,
      stackScrolls: stack ? stack.scrollHeight > stack.clientHeight + 1 : false,
      notices,
    };
  }, pane);
}

/**
 * One measured claim, failing with what was measured: this suite's `expect` takes no message,
 * and a bare "expected 412 to be at most 380" names neither the box nor the edge.
 */
function check(
  what: string,
  value: number | boolean,
  is: "atLeast" | "atMost" | "below" | "above" | "is",
  other: number | boolean,
) {
  const holds =
    is === "is"
      ? value === other
      : is === "atLeast"
        ? value >= other
        : is === "atMost"
          ? value <= other
          : is === "below"
            ? value < other
            : value > other;
  if (!holds) throw new Error(`${what}: ${String(value)} is not ${is} ${String(other)}`);
}

/** `inner` is inside `outer` on all four sides, to within a pixel of rounding. */
function inside(inner: Box | null, outer: Box | null, what: string) {
  check(`${what}: nothing was drawn`, inner !== null, "is", true);
  check(`${what}: nothing to be inside`, outer !== null, "is", true);
  const [a, b] = [inner as Box, outer as Box];
  check(`${what} has no width`, a.width, "above", 0);
  check(`${what} has no height`, a.height, "above", 0);
  check(`${what} starts left of it`, a.left, "atLeast", b.left - 1);
  check(`${what} runs past its right edge`, a.right, "atMost", b.right + 1);
  check(`${what} starts above it`, a.top, "atLeast", b.top - 1);
  check(`${what} runs past its bottom edge`, a.bottom, "atMost", b.bottom + 1);
}

/** Whether two boxes share any area. */
const overlap = (a: Box, b: Box) =>
  a.left < b.right - 1 && b.left < a.right - 1 && a.top < b.bottom - 1 && b.top < a.bottom - 1;

/**
 * **No Notice is over the terminal** (#1647): the row's stack ends above the terminal's top
 * edge, and what each Notice draws shares no area with it.
 *
 * What a Notice draws is its box **as far as the stack shows it**: once "+N more" is open the
 * stack scrolls, and a Notice scrolled out of it still has a box below the row, clipped and
 * drawn nowhere. So each box is cut to the stack's before it is compared.
 */
function offTheTerminal(seen: {
  terminal: Box | null;
  stack: Box | null;
  notices: { box: Box | null; shown: boolean }[];
}) {
  check("the pane has no terminal", seen.terminal !== null, "is", true);
  check("the pane has no stack of Notices", seen.stack !== null, "is", true);
  const [terminal, stack] = [seen.terminal as Box, seen.stack as Box];
  check("the stack of Notices runs onto the terminal", stack.bottom, "atMost", terminal.top + 1);
  for (const [index, notice] of seen.notices.entries()) {
    if (!notice.shown) continue;
    const box = notice.box as Box;
    const top = Math.max(box.top, stack.top);
    const bottom = Math.min(box.bottom, stack.bottom);
    const left = Math.max(box.left, stack.left);
    const right = Math.min(box.right, stack.right);
    // Scrolled wholly out of the stack: drawn nowhere.
    if (bottom - top <= 0 || right - left <= 0) continue;
    const drawn = { left, right, top, bottom, width: right - left, height: bottom - top };
    check(`Notice ${index + 1} is over the terminal`, overlap(drawn, terminal), "is", false);
    check(
      `Notice ${index + 1} is not above the terminal`,
      drawn.bottom,
      "atMost",
      terminal.top + 1,
    );
  }
}

/**
 * **The brief, scrolled to in the box under the line** (#1647): the pane's row is at most three
 * fifths of the pane, so what a way out opened gives way first and scrolls in its own box
 * (#1481). The box is scrolled to the brief's top, and the brief is held there: at the box's
 * width, its top shown, and the rest of it shown too or scrolled to in the same box.
 */
async function briefInTheBoxUnder(pane: number) {
  const seen = await browser.execute((at: number) => {
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
    const raised = frames[at]?.querySelector('[data-raised="pane-notices.e2e"]');
    const under = raised?.querySelector<HTMLElement>("[data-scope=notice][data-part=under]");
    const pre = raised?.querySelector<HTMLElement>("pre");
    if (under && pre)
      under.scrollTop += pre.getBoundingClientRect().top - under.getBoundingClientRect().top;
    return {
      under: box(under),
      pre: box(pre),
      underScrolls: under ? under.scrollHeight > under.clientHeight + 1 : false,
    };
  }, pane);
  check("the brief's box was not drawn", seen.pre !== null, "is", true);
  check("there is no box under the line", seen.under !== null, "is", true);
  const [pre, under] = [seen.pre as Box, seen.under as Box];
  check("the brief's box has no width", pre.width, "above", 0);
  check("the brief's box has no height", pre.height, "above", 0);
  check(
    "the brief's box starts left of the box under the line",
    pre.left,
    "atLeast",
    under.left - 1,
  );
  check("the brief's box runs past the box under the line", pre.right, "atMost", under.right + 1);
  check("the brief's box starts above the box under the line", pre.top, "atLeast", under.top - 1);
  check(
    "the brief's box runs past the box under the line, which does not scroll",
    pre.bottom <= under.bottom + 1 || seen.underScrolls,
    "is",
    true,
  );
}

/**
 * How far anything is scrolled sideways, from the pane's frame up to the page. A focus scrolls
 * a `hidden` box to show what overflowed it, which is how the band and the sidebar came to be
 * cut at the left: every number here is 0 when nothing overflows.
 */
async function scrolledSideways(pane: number): Promise<number[]> {
  return browser.execute((at: number) => {
    const frames = [...document.querySelectorAll(".pane-frame")].filter((frame) =>
      frame.querySelector('[data-testid="pane"]'),
    );
    const all: number[] = [];
    for (let el: Element | null = frames[at] ?? null; el; el = el.parentElement)
      all.push(Math.round(el.scrollLeft));
    all.push(Math.round(window.scrollX));
    return all;
  }, pane);
}

describe("a Notice in a pane", () => {
  /** The tabs that were already there: the strip is back to them once this file's chats end. */
  let wereAlreadyOpen: string[] = [];
  /** The chats running when the file began, so only the ones it started are ended again. */
  let chatsBefore: Running = {};
  let was = { width: 1280, height: 800 };

  before(async () => {
    wereAlreadyOpen = await tabNames();
    chatsBefore = await chatsRunning();
    was = await browser.getWindowSize();
    await pressAndStart("New tab");
    await untilShows(0, READY);
    await pressAndStart("Split right");
    await untilShows(1, READY);
    // What an earlier action answered stands at the top of the content until it is closed
    // (#3344), and takes the panes' height with it: closed, as a person reading a pane would,
    // so each case measures the pane the window gives.
    await browser.execute(() => {
      for (const close of document.querySelectorAll<HTMLElement>(
        '.window-work > .action-result button[aria-label="Dismiss"]',
      ))
        close.click();
    });
  });

  afterEach(async () => {
    await lower();
    await browser.execute(() => {
      for (const one of document.querySelectorAll('[data-raised="pane-notices.e2e:dialog"]'))
        one.remove();
    });
    const picker = await $('[role="dialog"][aria-labelledby="start-chat"]');
    if (await picker.isDisplayed().catch(() => false)) await browser.keys(["Escape"]);
  });

  // One app process serves the whole run: the window goes back to the size it had, and every
  // chat this file opened is ended (as `pane-fill.e2e.ts` ends its own).
  after(async () => {
    await browser.setWindowSize(was.width, was.height);
    // Each chat this file started, by the number the core gave it (#1796): a tab's name may be
    // one an older chat has too.
    await endTheChatsStartedSince(chatsBefore);
    await browser.waitUntil(
      async () => (await tabNames()).every((tab) => wereAlreadyOpen.includes(tab)),
      { timeout: 20_000, timeoutMsg: "the pane-notices spec left a chat open behind it" },
    );
  });

  /**
   * Asks for a window of this size and waits until the page has stopped changing width. What
   * the window became is the runner's to decide (a small screen gives less than it was asked
   * for), so each case below asserts the pane's width it needs and never the window's.
   */
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

  for (const pane of [0, 1]) {
    it(`fits pane ${pane} of a split in the narrowest window, sentence first and what it opened under it`, async () => {
      // 1024 px is the narrowest window purlis supports (ADR 0054), and with two panes side by
      // side each is a few hundred pixels: the operator's screenshot.
      await windowIs(1024, 768);
      expect(await raise(pane, { sentence: SENTENCE, ways: WAYS, opened: BRIEF })).toBe(true);

      const seen = await measured(pane);
      const [notice] = seen.notices;
      const frame = seen.frame as Box;
      expect(seen.notices).toHaveLength(1);
      expect(seen.neighbours).toHaveLength(1);
      // The case this is about: a pane narrower than a Notice would like to be.
      check("the pane is not a narrow one", frame.width, "below", 40 * seen.rem);

      // Inside its pane on all four sides, and so off its neighbour.
      inside(notice.box, frame, "the Notice in its pane");
      for (const neighbour of seen.neighbours)
        check(
          "it is over the pane beside it",
          overlap(notice.box as Box, neighbour as Box),
          "is",
          false,
        );
      expect((notice.box as Box).right).toBeLessThanOrEqual(seen.window.width);

      // The sentence has the row: as wide as the Notice, less its padding and its mark's
      // column (#3142). It was one word wide, and taller than it was wide.
      const says = notice.says as Box;
      expect(says.width).toBeGreaterThanOrEqual((notice.box as Box).width - 4 * seen.rem);
      expect(says.width).toBeGreaterThan(says.height);

      // Every way out is inside the Notice, under the sentence, and on one line or, where it
      // alone is nearly as wide as the pane, two. They were five lines each.
      expect(notice.buttons.map((one) => one.label)).toEqual(WAYS);
      for (const button of notice.buttons) {
        inside(button.box, notice.box, `"${button.label}" in the Notice`);
        check(
          `"${button.label}" is beside the sentence`,
          (button.box as Box).top,
          "atLeast",
          says.bottom - 1,
        );
        check(
          `"${button.label}" is broken over more than two lines`,
          (button.box as Box).height,
          "atMost",
          button.line * 2 + button.chrome + 4,
        );
        // A real target: about a line of text tall, or more.
        expect((button.box as Box).height).toBeGreaterThanOrEqual(button.line - 2);
        // The shared button in its wrapping row: none of its words cut off (#1855).
        check(`"${button.label}" is clipped`, button.clipped, "is", false);
      }
      // No two of them on top of each other.
      for (const [index, one] of notice.buttons.entries())
        for (const other of notice.buttons.slice(index + 1))
          check(
            `${one.label} / ${other.label}`,
            overlap(one.box as Box, other.box as Box),
            "is",
            false,
          );

      // What a way out opened is under the line, at the Notice's width: never beside it. Its
      // draft scrolls in its own box and widens nothing, however long its lines are.
      const [under, line, box] = [notice.under as Box, notice.line as Box, notice.box as Box];
      expect(under.top).toBeGreaterThanOrEqual(line.bottom - 1);
      expect(under.left).toBeGreaterThanOrEqual(box.left - 1);
      expect(under.right).toBeLessThanOrEqual(box.right + 1);
      expect(notice.preScrolls).toBe(true);
      await briefInTheBoxUnder(pane);

      // A wash, drawn: on the pane's own fill, never over the terminal (#3142, rule 7).
      expect(notice.background).not.toMatch(/rgba\(.*,\s*0\)|transparent/);

      // Above the terminal, never over it, in a narrow pane (#1647).
      offTheTerminal(seen);
    });
  }

  it("fits the dispatch question's five answers in a narrow pane, each whole and none over another", async () => {
    await windowIs(1024, 768);
    expect(await raise(0, { sentence: ASKS, ways: FIVE_WAYS, opened: BRIEF })).toBe(true);

    const seen = await measured(0);
    const [notice] = seen.notices;
    const frame = seen.frame as Box;
    expect(seen.notices).toHaveLength(1);
    check("the pane is not a narrow one", frame.width, "below", 40 * seen.rem);
    inside(notice.box, frame, "the Notice in its pane");

    // All five, in the order they are answered, under the sentence and inside the Notice.
    const says = notice.says as Box;
    expect(notice.buttons.map((one) => one.label)).toEqual(FIVE_WAYS);
    for (const button of notice.buttons) {
      inside(button.box, notice.box, `"${button.label}" in the Notice`);
      check(
        `"${button.label}" is beside the sentence`,
        (button.box as Box).top,
        "atLeast",
        says.bottom - 1,
      );
      check(
        `"${button.label}" is broken over more than two lines`,
        (button.box as Box).height,
        "atMost",
        button.line * 2 + button.chrome + 4,
      );
      expect((button.box as Box).height).toBeGreaterThanOrEqual(button.line - 2);
      check(`"${button.label}" is clipped`, button.clipped, "is", false);
    }
    for (const [index, one] of notice.buttons.entries())
      for (const other of notice.buttons.slice(index + 1))
        check(
          `${one.label} / ${other.label}`,
          overlap(one.box as Box, other.box as Box),
          "is",
          false,
        );

    // The brief is still under all five, never pushed beside them or out of the pane.
    const [under, line] = [notice.under as Box, notice.line as Box];
    expect(under.top).toBeGreaterThanOrEqual(line.bottom - 1);
    offTheTerminal(seen);
    await briefInTheBoxUnder(0);
  });

  for (const pane of [0, 1]) {
    it(`keeps a dispatch's boxes and access line in pane ${pane} of a narrow split: under the answers, above the brief, no wider than the Notice`, async () => {
      // #1502: the question grew a list of boxes and a sentence of hosts. In the operator's
      // narrow split they must wrap inside the Notice, and reach nothing sideways.
      await windowIs(1024, 768);
      expect(await raise(pane, { sentence: SENTENCE, ways: WAYS, opened: BRIEF, also: ALSO })).toBe(
        true,
      );

      const seen = await measured(pane);
      const [notice] = seen.notices;
      const frame = seen.frame as Box;
      check("the pane is not a narrow one", frame.width, "below", 40 * seen.rem);
      const [box, line, also, report] = [
        notice.box as Box,
        notice.line as Box,
        notice.also as Box,
        notice.report as Box,
      ];

      // The Notice is as wide as it was without them: a host is never what widens it.
      check("the Notice runs past its pane", box.right, "atMost", frame.right + 1);
      check("the Notice starts left of its pane", box.left, "atLeast", frame.left - 1);
      for (const neighbour of seen.neighbours)
        check("it is over the pane beside it", overlap(box, neighbour as Box), "is", false);

      // The order it is answered in: the sentence and the answers, then the boxes and the
      // access line, then the brief.
      for (const button of notice.buttons)
        check(
          `"${button.label}" is not above the boxes`,
          (button.box as Box).bottom,
          "atMost",
          also.top + 1,
        );
      check("the boxes are beside the line", also.top, "atLeast", line.bottom - 1);
      check("the brief is above the boxes", report.top, "atLeast", also.bottom - 1);
      check("the boxes are wider than the Notice", also.right, "atMost", box.right + 1);
      check("the boxes start left of the Notice", also.left, "atLeast", box.left - 1);

      // Every box: its mark, its name and what it works with, inside the Notice's width, one
      // under another and never on top of each other.
      expect(notice.options).toHaveLength(ALSO.boxes.length);
      for (const [index, one] of notice.options.entries()) {
        const name = ALSO.boxes[index].name;
        for (const [what, part] of [
          ["its box", one.box],
          ["its name", one.label],
          ["what it works with", one.says],
        ] as const) {
          check(`${name}: ${what} was not drawn`, part !== null, "is", true);
          check(`${name}: ${what} has no width`, (part as Box).width, "above", 0);
          check(
            `${name}: ${what} runs past the Notice`,
            (part as Box).right,
            "atMost",
            box.right + 1,
          );
          check(`${name}: ${what} starts left of it`, (part as Box).left, "atLeast", box.left - 1);
        }
        // The mark is a target, and the name is beside it, not under it.
        check(`${name}: its box is too small to press`, (one.box as Box).width, "atLeast", 10);
        check(
          `${name}: its name is not beside its box`,
          (one.label as Box).left,
          "atLeast",
          (one.box as Box).right - 1,
        );
        const next = notice.options[index + 1];
        if (next !== undefined)
          check(
            `${name} is on top of the next box`,
            (next.option as Box).top,
            "atLeast",
            (one.option as Box).bottom - 1,
          );
      }

      // The access line wraps at the Notice's width, under the boxes.
      const access = notice.access as Box;
      check("the access line runs past the Notice", access.right, "atMost", box.right + 1);
      check("the access line starts left of it", access.left, "atLeast", box.left - 1);
      check(
        "the access line is above the boxes",
        access.top,
        "atLeast",
        (notice.options[notice.options.length - 1].option as Box).bottom - 1,
      );
      check("the access line did not wrap", access.height, "above", notice.buttons[0].line * 1.5);

      // Taller than the pane, it scrolls inside the pane: the last box and the brief are
      // reached, and nothing is scrolled sideways to show a host.
      inside(seen.stack, frame, "the stack of Notices");
      const reached = await browser.execute(() => {
        // Each box is the checkbox family's (#2907): its input takes the focus, its box is drawn.
        const rows = document.querySelectorAll<HTMLElement>(
          '[data-raised="pane-notices.e2e"] .dispatch-also [data-scope="checkbox"][data-part="root"]',
        );
        const row = rows[rows.length - 1];
        row?.querySelector("input")?.focus();
        row?.scrollIntoView({ block: "nearest", inline: "nearest" });
        const last = row?.querySelector<HTMLElement>('[data-part="box"]') ?? row;
        const { left, right, top, bottom, width, height } = last.getBoundingClientRect();
        return { left, right, top, bottom, width, height };
      });
      inside(reached, frame, "the last box, scrolled to");
      expect((await scrolledSideways(pane)).filter((by) => by !== 0)).toEqual([]);
    });
  }

  it("shows two in a pane and keeps the rest behind +N more, above the terminal", async () => {
    // The operator's screenshot (#1647): three cards stacked over the terminal. Six of the
    // tallest here, far more than any pane of this window holds.
    await windowIs(1024, 768);
    for (let one = 0; one < 6; one++)
      expect(await raise(0, { sentence: SENTENCE, ways: WAYS, opened: BRIEF })).toBe(true);
    await browser.waitUntil(async () => (await measured(0)).moreSays === "+4 more", {
      timeout: 10_000,
      interval: 100,
      timeoutMsg: "the pane never said +4 more",
    });

    const seen = await measured(0);
    const frame = seen.frame as Box;
    expect(seen.notices).toHaveLength(6);
    expect(seen.notices.map((one) => one.shown)).toEqual([true, true, false, false, false, false]);

    // The row is the top of the pane, inside it, and the terminal starts under it; the pane is
    // as tall as its neighbour, so it did not grow to hold what purlis has to say.
    inside(seen.row, frame, "the pane's row of Notices");
    check(
      "the terminal starts above the row's bottom",
      (seen.terminal as Box).top,
      "atLeast",
      (seen.row as Box).bottom - 1,
    );
    check(
      "the row is more than three fifths of the pane",
      (seen.row as Box).height,
      "atMost",
      frame.height * 0.6 + 1,
    );
    expect(Math.abs(frame.height - (seen.neighbours[0] as Box).height)).toBeLessThanOrEqual(1);
    offTheTerminal(seen);
    // "+N more" is in the band, on a row of its own under the warnings (#3142, rule 5).
    inside(seen.more, seen.row, "+4 more in the row");
    check(
      "+4 more is not under the warnings",
      (seen.more as Box).top,
      "atLeast",
      (seen.stack as Box).bottom - 1,
    );

    // Opened, every one is in the row, one under another in the order raised, the row scrolls
    // inside the pane, and the last one's last button is reached there.
    await $("button[data-way=more]").click();
    await browser.waitUntil(async () => (await measured(0)).notices.every((one) => one.shown), {
      timeout: 10_000,
      interval: 100,
      timeoutMsg: "+4 more never opened",
    });
    const open = await measured(0);
    inside(open.row, open.frame, "the opened row");
    expect(open.stackScrolls).toBe(true);
    offTheTerminal(open);
    for (const [index, one] of open.notices.entries()) {
      const next = open.notices[index + 1];
      if (next === undefined) continue;
      expect((next.box as Box).top).toBeGreaterThan((one.box as Box).bottom);
      expect((next.box as Box).left).toBeCloseTo((one.box as Box).left, 0);
    }
    const reached = await browser.execute(() => {
      const buttons = document.querySelectorAll<HTMLElement>(
        '[data-raised="pane-notices.e2e"][data-placement=pane] > [data-scope=button-row] > button',
      );
      const last = buttons[buttons.length - 1];
      last?.focus();
      last?.scrollIntoView({ block: "nearest" });
      const { left, right, top, bottom, width, height } = last.getBoundingClientRect();
      return { left, right, top, bottom, width, height };
    });
    inside(reached, open.row, "the last Notice's last button, scrolled to");

    // Escape closes it, and the keyboard is back on "+4 more".
    await browser.keys(["Escape"]);
    await browser.waitUntil(
      async () => (await measured(0)).notices.filter((one) => one.shown).length === 2,
      { timeout: 10_000, interval: 100, timeoutMsg: "Escape never closed +4 more" },
    );
  });

  for (const width of [1024, 1600]) {
    it(`never draws a Notice over the terminal in a ${width === 1024 ? "narrow" : "wide"} pane`, async () => {
      // #1647: one Notice and two, in a narrow pane of a split and a wide one.
      await windowIs(width, width === 1024 ? 768 : 1000);
      for (const count of [1, 2]) {
        expect(await raise(0, { sentence: SENTENCE, ways: WAYS })).toBe(true);
        const seen = await measured(0);
        expect(seen.notices.filter((one) => one.shown)).toHaveLength(count);
        inside(seen.row, seen.frame, "the pane's row of Notices");
        offTheTerminal(seen);
        check(
          "the terminal starts above the row's bottom",
          (seen.terminal as Box).top,
          "atLeast",
          (seen.row as Box).bottom - 1,
        );
      }
    });
  }

  it("scrolls nothing sideways when the keyboard goes through its buttons", async () => {
    // What cut the band and the sidebar at the left: a button that overflowed the pane was
    // focused, and every box up to the page was scrolled to show it.
    await windowIs(1024, 768);
    for (const pane of [0, 1])
      expect(await raise(pane, { sentence: SENTENCE, ways: WAYS, opened: BRIEF })).toBe(true);

    await browser.execute(() => {
      for (const button of document.querySelectorAll<HTMLElement>(
        '[data-raised="pane-notices.e2e"] button',
      )) {
        button.focus();
        button.scrollIntoView({ block: "nearest", inline: "nearest" });
      }
    });

    for (const pane of [0, 1]) {
      const scrolled = await scrolledSideways(pane);
      expect(scrolled.length).toBeGreaterThan(3);
      expect(scrolled.filter((by) => by !== 0)).toEqual([]);
    }
    // And the band and the sidebar start where the window does.
    const starts = await browser.execute(() =>
      [".title-bar", 'nav[aria-label="Explorer"]', '[data-testid="status-line"]'].map(
        // A region that is put away is not cut: only one that is drawn is measured.
        (where) => document.querySelector(where)?.getBoundingClientRect().left ?? 0,
      ),
    );
    for (const left of starts) expect(left).toBeGreaterThanOrEqual(0);
  });

  it("draws a warning as a card the band's width, its words above its ways out (#3142)", async () => {
    await windowIs(1600, 1000);
    // The one-liner the corner was made for (ADR 0062), then the long one.
    const short = { sentence: "Restarting this chat…", ways: ["Dismiss"] };
    expect(await raise(0, short)).toBe(true);
    expect(await raise(0, { sentence: SENTENCE, ways: WAYS, opened: BRIEF })).toBe(true);

    const seen = await measured(0);
    const frame = seen.frame as Box;
    const [oneLine, long] = seen.notices;

    // Variant A: the words on their own row, every way out under them, side by side while
    // they fit; the card as wide as the band, whatever it says.
    inside(oneLine.box, frame, "the one-line Notice in its pane");
    const says = oneLine.says as Box;
    for (const button of oneLine.buttons) {
      const at = button.box as Box;
      check(`"${button.label}" is not under the words`, at.top, "atLeast", says.bottom - 1);
      expect(at.height).toBeLessThanOrEqual(button.line * 1.5 + button.chrome + 4);
    }
    expect(says.height).toBeLessThanOrEqual(oneLine.buttons[0].line * 1.5);
    expect(Math.abs((oneLine.box as Box).width - seen.stackInside)).toBeLessThanOrEqual(1);

    // The long one: inside the pane, the band's width, and what a way out opened under its line.
    inside(long.box, frame, "the long Notice in its pane");
    expect(Math.abs((long.box as Box).width - seen.stackInside)).toBeLessThanOrEqual(1);
    expect((long.under as Box).top).toBeGreaterThanOrEqual((long.line as Box).bottom - 1);
    expect((long.under as Box).right).toBeLessThanOrEqual((long.box as Box).right + 1);
    for (const neighbour of seen.neighbours)
      expect(overlap(long.box as Box, neighbour as Box)).toBe(false);
    offTheTerminal(seen);
  });

  it("keeps each Notice's drawn box inside a pane a standing result squeezes (#3344)", async () => {
    await windowIs(1600, 1000);
    // What an action answered stands at the top of the content until it is closed, and takes the
    // panes' height: one as tall as a result card, raised as this file raises its Notices.
    const stood = await browser.execute(() => {
      const work = document.querySelector(".window-work");
      if (!work) return false;
      const result = document.createElement("div");
      result.dataset.raised = "pane-notices.e2e";
      result.style.blockSize = "120px";
      result.style.flex = "none";
      work.prepend(result);
      return true;
    });
    expect(stood).toBe(true);
    expect(await raise(0, { sentence: "Restarting this chat…", ways: ["Dismiss"] })).toBe(true);
    expect(await raise(0, { sentence: SENTENCE, ways: WAYS, opened: BRIEF })).toBe(true);

    const seen = await measured(0);
    const [frame, stack] = [seen.frame as Box, seen.stack as Box];
    // The band keeps to its share of the pane, and scrolls what it cannot show.
    check(
      "the band runs past its share of the pane",
      stack.height,
      "atMost",
      frame.height * 0.46 + 1,
    );
    // What each Notice draws, its box clipped to the stack, stays inside the pane and off the
    // terminal, however short the pane is.
    for (const [index, notice] of seen.notices.entries()) {
      if (!notice.shown) continue;
      const box = notice.box as Box;
      const top = Math.max(box.top, stack.top);
      const bottom = Math.min(box.bottom, stack.bottom);
      if (bottom - top <= 0) continue;
      const left = Math.max(box.left, stack.left);
      const right = Math.min(box.right, stack.right);
      inside(
        { left, right, top, bottom, width: right - left, height: bottom - top },
        frame,
        `Notice ${index + 1}, as drawn`,
      );
    }
    offTheTerminal(seen);
  });

  it("is above its terminal and never over it, and under a dialog and its scrim", async () => {
    await windowIs(1024, 768);
    expect(await raise(1, { sentence: SENTENCE, ways: WAYS, opened: BRIEF })).toBe(true);

    /** What is drawn at the middle of the Notice's sentence: the Notice, or something over it. */
    const atTheNotice = () =>
      browser.execute(() => {
        const says = document.querySelector(
          '[data-raised="pane-notices.e2e"] [data-scope=notice][data-part=message]',
        );
        if (!says) return "(no Notice)";
        const box = says.getBoundingClientRect();
        const hit = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);
        if (hit?.closest('[data-raised="pane-notices.e2e"]')) return "the Notice";
        if (hit?.closest('[role="dialog"]')) return "the dialog";
        if (hit?.closest('[data-scope="dialog"][data-part="scrim"]')) return "the scrim";
        if (hit?.closest('[data-testid="pane"]')) return "the terminal";
        return hit?.className || hit?.tagName || "(nothing)";
      });

    // Drawn, in its row above the terminal (#1647): nothing is over it, and it over nothing.
    expect(await atTheNotice()).toBe("the Notice");
    offTheTerminal(await measured(1));

    // A real dialog, portaled to the body as every dialog is: the picker. It was drawn UNDER
    // the pane's corner, so a Notice sat on the dialog's top edge.
    await pressOnly("New tab");
    const picker = await $('[role="dialog"][aria-labelledby="start-chat"]');
    await picker.waitForDisplayed({ timeout: 20_000 });
    expect(["the scrim", "the dialog"]).toContain(await atTheNotice());
    // And the dialog's own top edge is the dialog.
    const top = await browser.execute(() => {
      const dialog = document.querySelector('[role="dialog"][aria-labelledby="start-chat"]');
      if (!dialog) return false;
      const box = dialog.getBoundingClientRect();
      const hit = document.elementFromPoint(box.left + box.width / 2, box.top + 2);
      return hit !== null && dialog.contains(hit);
    });
    expect(top).toBe(true);
    await browser.keys(["Escape"]);
    await expect(picker).not.toBeDisplayed();
    expect(await atTheNotice()).toBe("the Notice");
  });

  it("keeps the dispatch question's workspace choice under its answers and inside a narrow pane", async () => {
    // #1505. Where an Allow holds is chosen in the box under the line, above the brief. Drawn
    // here by hand, as every Notice in this file is, with a workspace named far longer than
    // the pane is wide. **This holds the classes, not the component**: since train 67 the
    // choice's group is a `div` with the utility classes `DispatchGrantNotice` gives it, and
    // `.dispatch-within` here is only a name to find it by (App.css has no rule for it). That markup, and its place under the answers and above the brief, is
    // `DispatchGrantNotice.test.tsx`'s ("draws the choice under the answers and above the
    // brief") and `Notice.pane.test.tsx`'s; change the three together.
    await windowIs(1024, 768);
    expect(await raise(0, { sentence: ASKS, ways: FIVE_WAYS, opened: BRIEF })).toBe(true);
    const drawn = await browser.execute((workspace: string) => {
      const under = document.querySelector(
        '[data-raised="pane-notices.e2e"] [data-scope=notice][data-part=under]',
      );
      if (!under) return false;
      const choice = document.createElement("div");
      choice.className = "dispatch-within my-1 min-w-0 max-w-full text-xs wrap-anywhere";
      choice.setAttribute("role", "radiogroup");
      const says = document.createElement("p");
      says.className = "m-0 text-text-secondary";
      says.textContent = "Where an Allow for you or for the project holds";
      choice.append(says);
      for (const [index, text] of [`In ${workspace} only`, "In any workspace"].entries()) {
        const label = document.createElement("label");
        label.className = "flex min-w-0 max-w-full";
        const radio = document.createElement("input");
        radio.type = "radio";
        radio.name = "pane-notices-within";
        radio.checked = index === 0;
        label.append(radio, ` ${text}`);
        choice.append(label);
      }
      under.prepend(choice);
      return true;
    }, "a_workspace_named_wider_than_a_pane_".repeat(6));
    expect(drawn).toBe(true);

    const seen = await measured(0);
    const [notice] = seen.notices;
    const frame = seen.frame as Box;
    check("the pane is not a narrow one", frame.width, "below", 40 * seen.rem);
    // **Scrolled to, in the box under the line** (#1647): the row is at most three fifths of the
    // pane, so what a way out opened gives way first and scrolls in its own box (#1481). The
    // choice is measured where that box shows it, and the brief with it, in one frame.
    const parts = await browser.execute(() => {
      const box = [
        (el: Element | null | undefined) => {
          if (!el) return null;
          const { left, right, top, bottom, width, height } = el.getBoundingClientRect();
          return { left, right, top, bottom, width, height };
        },
      ][0];
      const under = document.querySelector<HTMLElement>(
        '[data-raised="pane-notices.e2e"] [data-scope=notice][data-part=under]',
      );
      const choice = document.querySelector<HTMLElement>(
        '[data-raised="pane-notices.e2e"] .dispatch-within',
      );
      if (under && choice)
        under.scrollTop += choice.getBoundingClientRect().top - under.getBoundingClientRect().top;
      return {
        under: box(under),
        underScrolls: under ? under.scrollHeight > under.clientHeight + 1 : false,
        line: box(
          document.querySelector(
            '[data-raised="pane-notices.e2e"][data-placement=pane] > [data-scope=button-row]',
          ),
        ),
        pre: box(document.querySelector('[data-raised="pane-notices.e2e"] pre')),
        choice: box(choice),
        says: box(choice?.querySelector("p")),
        labels: [...(choice?.querySelectorAll("label") ?? [])].map((label) => box(label)),
      };
    });

    // The Notice still fits its pane, and the choice is in the box under the line: at its
    // width, its top shown there, and the rest of it either shown too or scrolled to there.
    inside(notice.box, frame, "the Notice in its pane");
    const [under, choice] = [parts.under as Box, parts.choice as Box];
    check("the workspace choice was not drawn", parts.choice !== null, "is", true);
    check(
      "the workspace choice starts left of the box under the line",
      choice.left,
      "atLeast",
      under.left - 1,
    );
    check(
      "the workspace choice runs past the box under the line",
      choice.right,
      "atMost",
      under.right + 1,
    );
    check(
      "the workspace choice starts above the box under the line",
      choice.top,
      "atLeast",
      under.top - 1,
    );
    check(
      "the workspace choice runs past the box under the line, which does not scroll",
      choice.bottom <= under.bottom + 1 || parts.underScrolls,
      "is",
      true,
    );
    inside(parts.says, parts.choice, "what the choice is about");
    expect(parts.labels).toHaveLength(2);
    for (const [index, label] of parts.labels.entries())
      inside(label, parts.choice, `choice ${index + 1}`);
    // The long name broke: the two choices are one under the other, never one over the other.
    const [narrow, wide] = parts.labels as Box[];
    check("the two choices overlap", overlap(narrow, wide), "is", false);
    // Under every answer, and above the brief.
    check(
      "the choice is beside the answers",
      choice.top,
      "atLeast",
      (parts.line as Box).bottom - 1,
    );
    check("the brief is above the choice", (parts.pre as Box).top, "atLeast", choice.bottom - 1);
  });
});

/**
 * **Ask {persona}, measured** (#1481): the dialog the operator met from "Dispatch to devops…".
 * Its "What to ask" box was a third of the dialog's width under a full-width name, and its
 * buttons were below the bottom of the window.
 *
 * Drawn as `AskPersona` draws it (see the note at the top of this file), where a dialog is
 * portaled: the end of the body.
 */
describe("the Ask persona dialog", () => {
  let was = { width: 1280, height: 800 };

  before(async () => {
    was = await browser.getWindowSize();
  });

  afterEach(async () => {
    await browser.execute(() => {
      for (const one of document.querySelectorAll('[data-raised="pane-notices.e2e:dialog"]'))
        one.remove();
    });
  });

  after(async () => {
    await browser.setWindowSize(was.width, was.height);
  });

  it("gives the box you write in the row's width, fits the window, and keeps its buttons in reach", async () => {
    await browser.setWindowSize(1024, 768);
    await browser.pause(500);

    const seen = await browser.execute(() => {
      const el = [
        (tag: string, cls: string, text?: string) => {
          const made = document.createElement(tag);
          made.className = cls;
          if (text !== undefined) made.textContent = text;
          made.dataset.raised = "pane-notices.e2e:dialog";
          return made;
        },
      ][0];
      const row = [
        (control: HTMLElement, help: string) => {
          const made = el("div", "ui-setting-row");
          const holder = el("div", "ui-setting-control");
          holder.append(control);
          made.append(
            el("label", "ui-setting-label", "A row"),
            holder,
            el("p", "ui-setting-help", help),
          );
          return made;
        },
      ][0];
      // The dialog family's anatomy (`ui/dialog/Dialog.tsx`), as the Ask a persona dialog
      // draws it: the scrim, the root, its header, its body and its footer with the answers.
      const part = (tag: string, name: string, text?: string) => {
        const made = el(tag, "", text);
        Object.assign(made.dataset, { scope: "dialog", part: name });
        return made;
      };
      const scrim = part("div", "scrim");
      const modal = part("div", "modal");
      const dialog = part("section", "root");
      Object.assign(dialog.dataset, { variant: "dialog", size: "standard" });
      dialog.classList.add("ask-persona");
      dialog.setAttribute("role", "dialog");
      const header = part("header", "header");
      header.append(part("h2", "title", "Ask devops"));
      // The field family's boxes (#2905), as `TextField` and `TextArea` draw them: the field's
      // root, the box, and the element typed in.
      const field = (scope: "text-input" | "textarea", place: string, typed: HTMLElement) => {
        const root = el("div", place);
        Object.assign(root.dataset, { scope: "field", part: "root" });
        const frame = el("div", "");
        Object.assign(frame.dataset, { scope, part: "root" });
        if (scope === "text-input") frame.dataset.size = "md";
        else frame.dataset.grows = "";
        frame.append(typed);
        root.append(frame);
        return root;
      };
      const name = field("text-input", "ui-setting-box", el("input", ""));
      // The shared list field (DS-10): the row's width, growing with its lines.
      const typed = el("textarea", "") as HTMLTextAreaElement;
      typed.rows = 4;
      const ask = field("textarea", "ui-setting-box ui-setting-list", typed);
      const form = part("form", "form");
      const body = part("div", "body");
      body.append(
        row(name, "What the task is called, on its tab and under this chat."),
        row(ask, "A task starts as devops on these words, with its own sandbox, hosts and vaults."),
      );
      // Where it works, and then some: more than a window of this height shows at once.
      for (let more = 0; more < 24; more++)
        body.append(
          row(
            el("span", "", "A branch of its own"),
            "purlis cuts a new branch, in a folder of its own.",
          ),
        );
      const footer = part("footer", "footer");
      const answers = part("div", "actions");
      // The button family's buttons (#2904), as `Button` draws them: Cancel leads.
      const family = (variant: string, words: string) => {
        const button = el("button", "", words);
        Object.assign(button.dataset, { scope: "button", part: "root", variant, size: "md" });
        return button;
      };
      answers.append(family("secondary", "Cancel"), family("primary", "Ask devops"));
      footer.append(part("div", "summary"), answers);
      form.append(body, part("div", "separator"), footer);
      dialog.append(header, form);
      modal.append(dialog);
      scrim.append(modal);
      document.body.append(scrim);

      const box = [
        (of: Element) => {
          const { left, right, top, bottom, width, height } = of.getBoundingClientRect();
          return { left, right, top, bottom, width, height };
        },
      ][0];
      const style = getComputedStyle(typed);
      return {
        window: {
          left: 0,
          top: 0,
          right: window.innerWidth,
          bottom: window.innerHeight,
          width: window.innerWidth,
          height: window.innerHeight,
        },
        dialog: box(dialog),
        name: box(name),
        ask: box(ask),
        resize: style.resize,
        answers: box(answers),
        scrolls: body.scrollHeight > body.clientHeight + 1,
        scrolledTo: body.scrollTop,
      };
    });

    // The box you write in is at least as wide as the name's, and grows with its lines rather
    // than being dragged (DS-10's list field).
    expect(seen.ask.width).toBeGreaterThanOrEqual(seen.name.width - 1);
    expect(Math.abs(seen.ask.left - seen.name.left)).toBeLessThanOrEqual(1);
    expect(seen.ask.width).toBeGreaterThan(seen.dialog.width * 0.8);
    expect(seen.ask.height).toBeGreaterThan(seen.name.height * 2);
    expect(seen.resize).toBe("none");

    // The dialog is inside the window and its body scrolls inside it; before any scrolling, its
    // answers are at its bottom edge, in view.
    inside(seen.dialog, seen.window, "the dialog in the window");
    expect(seen.scrolls).toBe(true);
    expect(seen.scrolledTo).toBe(0);
    inside(seen.answers, seen.dialog, "the dialog's answers, before it is scrolled");
    inside(seen.answers, seen.window, "the dialog's answers in the window");
  });
});
