// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { browser, $ } from "@wdio/globals";
import { READY } from "../harness.js";
import { pressAndStart } from "../opening.js";
import { chatsRunning, endTheChatsStartedSince, type Running } from "../chatsLeft.js";
import { theWindowHoldsTheRail, WINDOW_1280, windowAt } from "../rail.js";

/**
 * **The Chats list in a narrow sidebar, measured** (#1499, V100-50, #1675).
 *
 * The operator's screenshot: rows cut off at the sidebar's edge, "cancel mid-turn smart-ide ●
 * no…", with the state the first thing lost. A row is now one line as wide as the list: its
 * state's mark first, then the name, which gives way, then the persona's badge at its end
 * (DS-11). The mark is whole, a finished task's as a chat's (#1687), at the least width the
 * left region can be dragged to, five levels down, at the largest text.
 *
 * And the second screenshot (DS-11, #1843): a name about 53px right of its own filter, under an
 * empty line. A top-level chat's name now starts where the filter's text does, every row is the
 * shared row's height, and the first row comes right under the filter.
 *
 * That is layout, and jsdom lays nothing out, so it is measured here: in the real engine,
 * against the built stylesheet.
 *
 * **What is measured is the component's own markup.** This suite's fake harness cannot make
 * the chats these cases need (a task five levels down that ended without a report, a session
 * folded over five finished tasks). So `src/ChatsList.fixture.test.tsx` draws the real
 * `ChatsSection` with them and keeps what it drew in `e2e/fixtures/`, and fails when the
 * component stops drawing that. This file puts that markup into the window's left region, in
 * the real section's place, and measures it. Nothing here builds a row by hand.
 */

/** The least the left region can be dragged to (`SLOTS.left.floor` in `regions.ts`).
 *  `ChatsList.fixture.test.tsx` fails when this line and that one differ. */
const FLOOR = "11rem";
/** The largest text the window can be set to (`textSize.ts`), held the same way. */
const MOST_TEXT = 24;

const fixture = (name: string) =>
  readFileSync(fileURLToPath(new URL(`../fixtures/${name}`, import.meta.url)), "utf8");
const ONE_LINE = fixture("chats-list.one-line.html");

type Box = { left: number; right: number; top: number; bottom: number; width: number };

type Word = {
  /** The row it is on, by its name. */
  row: string;
  text: string;
  box: Box;
  /** How much of the word its own box does not show: 0 when it is whole. */
  cut: number;
  /** Whether any box above it, up to the section, clips it and is narrower than it. */
  clipped: boolean;
};

type Measured = {
  section: Box;
  /** How far the section scrolls sideways past what it shows: 0 when nothing overflows it. */
  overflow: number;
  /** Every finished task's state mark, measured as a word is: a finished row is one line too,
   *  its word heard and in its tooltip (#1687). */
  words: Word[];
  /** Every chat row's state mark, measured as a word is. */
  marks: Word[];
  /** Every row's own box: a chat's item, a finished task's line. */
  rows: { name: string; box: Box; nameCut: boolean }[];
};

/**
 * Puts the component's markup in the real section's place, held to `width` (any CSS length),
 * with the window's text at `text` px where one is given. Answers whether there was a left
 * region to put it in.
 */
async function draw(html: string, width: string, text?: number): Promise<boolean> {
  return browser.execute(
    (markup: string, wide: string, px: number | null) => {
      const real = document.querySelector<HTMLElement>(
        '.region-view > [data-testid="chats-section"]',
      );
      if (!real) return false;
      for (const one of document.querySelectorAll('[data-raised="chats-list.e2e"]')) one.remove();
      if (px !== null) document.documentElement.style.fontSize = `${px}px`;
      const holder = document.createElement("div");
      holder.innerHTML = markup;
      const drawn = holder.firstElementChild as HTMLElement | null;
      if (!drawn) return false;
      drawn.dataset.raised = "chats-list.e2e";
      drawn.removeAttribute("data-testid");
      drawn.style.width = wide;
      // All of it, not two fifths of the region: every row is measured.
      drawn.style.maxHeight = "none";
      drawn.style.flex = "none";
      real.style.display = "none";
      real.before(drawn);
      return true;
    },
    html,
    width,
    text ?? null,
  );
}

async function lower(): Promise<void> {
  await browser.execute(() => {
    for (const one of document.querySelectorAll('[data-raised="chats-list.e2e"]')) one.remove();
    document.documentElement.style.fontSize = "";
    const real = document.querySelector<HTMLElement>(
      '.region-view > [data-testid="chats-section"]',
    );
    if (real) real.style.display = "";
  });
}

async function measured(): Promise<Measured | null> {
  return browser.execute(() => {
    const box = (el: Element) => {
      const { left, right, top, bottom, width } = el.getBoundingClientRect();
      return { left, right, top, bottom, width };
    };
    const section = document.querySelector<HTMLElement>('[data-raised="chats-list.e2e"]');
    if (!section) return null;
    const nameOf = (el: Element) =>
      el.closest("li, .finished-task")?.querySelector(".session")?.textContent ?? "";
    /** How whole `el` is drawn, and inside what. */
    const seen = (el: HTMLElement, text: string) => {
      const mine = el.getBoundingClientRect();
      let clipped = false;
      for (let up = el.parentElement; up && up !== section; up = up.parentElement) {
        const style = getComputedStyle(up);
        if (style.overflowX === "visible") continue;
        const theirs = up.getBoundingClientRect();
        if (mine.left < theirs.left - 1 || mine.right > theirs.right + 1) clipped = true;
      }
      return {
        row: nameOf(el),
        text,
        box: box(el),
        cut: el.scrollWidth - el.clientWidth,
        clipped,
      };
    };
    return {
      section: box(section),
      overflow: section.scrollWidth - section.clientWidth,
      words: [...section.querySelectorAll<HTMLElement>(".finished-name .shown-state .shape")].map(
        (mark) => seen(mark, mark.closest(".shown-state")?.getAttribute("data-state") ?? ""),
      ),
      // A chat's row leads with its persona's mark, its state on it (#3342): the mark is what
      // must be whole, and a done chat's wears no dot.
      // The persona's mark itself, and not the box around it with the dot that stands past its
      // corner.
      marks: [...section.querySelectorAll<HTMLElement>(".chat .chat-persona > :first-child")].map(
        (mark) => seen(mark, mark.parentElement?.getAttribute("data-state") ?? ""),
      ),
      rows: [
        ...section.querySelectorAll<HTMLElement>(
          'li[role="none"]:not(.finished-tasks), .finished-line',
        ),
      ].map((row) => {
        const session = row.querySelector<HTMLElement>(".session");
        return {
          name: session?.textContent ?? "",
          box: box(row),
          nameCut: session !== null && session.scrollWidth > session.clientWidth,
        };
      }),
    };
  });
}

/** The top of every row and finished line, top to bottom. */
async function tops(): Promise<number[]> {
  return browser.execute(() =>
    [
      ...document.querySelectorAll(
        '[data-raised="chats-list.e2e"] li[role="none"], [data-raised="chats-list.e2e"] .finished-line',
      ),
    ].map((row) => Math.round(row.getBoundingClientRect().top)),
  );
}

function check(what: string, holds: boolean, saw: unknown) {
  if (!holds) throw new Error(`${what}: ${JSON.stringify(saw)}`);
}

/** Every state mark and word is whole and inside the list, no row runs past it, and it does
 *  not scroll sideways. */
function whole(seen: Measured | null, least: number) {
  check("nothing was measured", seen !== null, seen);
  const { section, overflow, words, marks, rows } = seen as Measured;
  check("no state mark was drawn", marks.length >= least, marks.length);
  check("no finished task's mark was drawn", words.length >= 1, words.length);
  check("the list scrolls sideways", overflow <= 1, overflow);
  for (const one of [...words, ...marks]) {
    check(`${one.row}: its state is not a word`, one.text !== "", one);
    check(`${one.row}: its state's word has no width`, one.box.width > 0, one);
    check(`${one.row}: its state's word is cut short by its own box`, one.cut <= 1, one);
    check(`${one.row}: its state's word is cut short by a box around it`, !one.clipped, one);
    check(
      `${one.row}: its state's word runs past the list`,
      one.box.left >= section.left - 1 && one.box.right <= section.right + 1,
      { one, section },
    );
  }
  for (const one of rows)
    check(`${one.name}: its row runs past the list`, one.box.right <= section.right + 1, {
      one,
      section,
    });
  return seen as Measured;
}

describe("the Chats list in a narrow sidebar", () => {
  // At 1280×800, where this was held to fit before the default window grew (`rail.ts`,
  // #3077); the project rail hides itself there beside both sidebars.
  before(() => windowAt(WINDOW_1280));
  after(() => theWindowHoldsTheRail());

  /** The chats running when the file began, so only the ones it started are ended again. */
  let chatsBefore: Running = {};

  before(async () => {
    chatsBefore = await chatsRunning();
    // A chat, so the left region draws its Chats section with a list in it.
    await pressAndStart("New tab");
    await browser.waitUntil(
      async () => (await $('[data-testid="pane"] .xterm-rows').getText()).includes(READY),
      { timeout: 30_000, interval: 250, timeoutMsg: "the chat never started" },
    );
    await $('.region-view > [data-testid="chats-section"]').waitForExist({ timeout: 10_000 });
  });

  afterEach(lower);

  after(async () => {
    // Each chat this file started, by the number the core gave it (#1796): a tab's name may be
    // one an older chat has too.
    await endTheChatsStartedSince(chatsBefore);
  });

  it("is never narrower than its floor: the left region's own least width", async () => {
    // What the panel is told, read back off it: the slot cannot be dragged under this.
    const least = await browser.execute(() => {
      const slot = document.querySelector<HTMLElement>(".slot-left");
      const root = Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
      return { width: slot?.getBoundingClientRect().width ?? 0, root };
    });
    const floor = Number.parseFloat(FLOOR) * least.root;

    check("the left region is narrower than its floor", least.width >= floor - 1, {
      least,
      floor,
    });
  });

  it("keeps every state's mark whole at the region's least width", async () => {
    check("there was no left region to draw in", await draw(ONE_LINE, FLOOR), FLOOR);
    const seen = whole(await measured(), 10);

    // A finished task that ended without a report, five levels down, keeps its mark whole.
    const unreported = seen.words.filter((word) => word.text === "unreported");
    check("the unreported end was not drawn", unreported.length >= 1, seen.words);
    // What gave way is the name: the long ones are cut short, with an ellipsis.
    for (const one of seen.rows.filter((row) => row.name.length > 40))
      check(`${one.name}: its name was not what gave way`, one.nameCut, one);
  });

  it("keeps every state's mark whole at the least width and the largest text", async () => {
    check("there was no left region to draw in", await draw(ONE_LINE, FLOOR, MOST_TEXT), FLOOR);

    whole(await measured(), 10);
  });

  it("holds below its floor too, where a window was left narrower by an older layout", async () => {
    check("there was no left region to draw in", await draw(ONE_LINE, "150px"), 150);

    whole(await measured(), 10);
  });

  it("moves no row when a row or the notes beside the title come to say something", async () => {
    check("there was no left region to draw in", await draw(ONE_LINE, "16rem"), "16rem");
    const before = await tops();
    check("no row was drawn", before.length >= 8, before);

    // What a row says changes while the row stands: a folded row's count comes or goes, and a
    // chat is being stopped. Here every other row gains a count and the rest lose theirs; and
    // the notes beside the title say why a key did nothing, and what the filter hides.
    const changed = await browser.execute(() => {
      const section = document.querySelector('[data-raised="chats-list.e2e"]');
      const lines = [...(section?.querySelectorAll(".chat .ui-row-trail") ?? [])];
      lines.forEach((line, at) => {
        const had = line.querySelector(".task-count");
        if (at % 2 === 1) {
          had?.remove();
          return;
        }
        if (had) return;
        const count = document.createElement("span");
        count.className = "task-count ui-text-meta ui-text-strong";
        count.textContent = "12";
        const stopping = document.createElement("span");
        stopping.className = "stopping ui-text-meta";
        stopping.textContent = "Stopping…";
        line.prepend(count, stopping);
      });
      const said = section?.querySelector(".chats-said");
      if (said)
        said.textContent =
          "purlis cannot open a chat beside another yet. Press Enter to open it in front.";
      const hides = section?.querySelector(".chats-hidden");
      if (hides) hides.textContent = "The filter hides 3 of 12 chats.";
      return lines.length;
    });
    check("no row had a line to change", changed >= 2, changed);

    const after = await tops();
    check("a row moved", JSON.stringify(after) === JSON.stringify(before), { before, after });
  });

  it("indents every row under another by one step when narrow, and a step per level when wide (#1682)", async () => {
    // The narrow list's one-step cap is an `@container chats` rule, which the window tests'
    // cascade does not evaluate (`cascade.testkit.ts`): it is held here, in the real engine.
    // A row stands one step in further than its depth, where a parent's twist hangs (DS-3).
    const indents = async () =>
      browser.execute(() => {
        const section = document.querySelector('[data-raised="chats-list.e2e"]');
        return [...(section?.querySelectorAll<HTMLElement>('[role="treeitem"]') ?? [])].map(
          (row) => ({
            level: Number(row.getAttribute("aria-level")),
            px: Number.parseFloat(getComputedStyle(row).paddingInlineStart),
          }),
        );
      });

    check("there was no left region to draw in", await draw(ONE_LINE, FLOOR), FLOOR);
    const narrow = await indents();
    check(
      "no row under another was drawn",
      narrow.some((one) => one.level >= 4),
      narrow,
    );
    const step = narrow.find((one) => one.level === 1)?.px ?? 0;
    check("a top-level row stands no step in", step > 0, narrow);
    for (const one of narrow)
      check(
        `a level-${one.level} row is not ${one.level === 1 ? "one step" : "two steps"} in`,
        Math.abs(one.px - (one.level === 1 ? step : 2 * step)) < 0.5,
        narrow,
      );

    await lower();
    check("there was no left region to draw in", await draw(ONE_LINE, "24rem"), "24rem");
    const wide = await indents();
    const at = (level: number) => wide.find((one) => one.level === level)?.px ?? 0;
    check(
      "a wide list does not step in per level",
      at(4) > at(3) && at(3) > at(2) && at(2) > at(1) && Math.abs(at(2) - 2 * at(1)) < 0.5,
      wide,
    );
  });

  it("starts a top-level chat's name where the filter's text starts, right under it (DS-11)", async () => {
    // At the region's least width, a common one, and the largest text: the two are lined up
    // by the same tokens, so they stay lined up whatever the room and the text size.
    for (const [width, text] of [
      [FLOOR, undefined],
      ["20rem", undefined],
      ["20rem", MOST_TEXT],
    ] as const) {
      await lower();
      check("there was no left region to draw in", await draw(ONE_LINE, width, text), width);
      const seen = await browser.execute(() => {
        const section = document.querySelector('[data-raised="chats-list.e2e"]');
        // The element typed in, inside the field family's box (#2905).
        const box = section?.querySelector<HTMLInputElement>(".chats-filter-box input");
        if (!section || !box) return null;
        const style = getComputedStyle(box);
        const outer = box.getBoundingClientRect();
        const rows = [...section.querySelectorAll<HTMLElement>('[role="treeitem"]')];
        // A sidebar view's row height as the theme in force sets it (#3342): the control's
        // height and a step.
        const probe = document.createElement("div");
        probe.style.height = "calc(var(--size-control) + var(--space-1))";
        document.body.append(probe);
        const chrome = probe.getBoundingClientRect().height;
        probe.remove();
        return {
          root: Number.parseFloat(getComputedStyle(document.documentElement).fontSize),
          chrome,
          // Where a typed letter starts: past the box's border and its padding.
          text:
            outer.left +
            Number.parseFloat(style.borderInlineStartWidth) +
            Number.parseFloat(style.paddingInlineStart),
          // The view's top: the title's line, the filter, the scope and the chips (#3342).
          filterBottom:
            section.querySelector('[data-part="top"]')?.getBoundingClientRect().bottom ?? 0,
          rows: rows.map((row) => ({
            level: Number(row.getAttribute("aria-level")),
            parent: row.hasAttribute("aria-expanded"),
            name: row.querySelector(".session")?.textContent ?? "",
            nameLeft: row.querySelector(".session")?.getBoundingClientRect().left ?? 0,
            top: row.getBoundingClientRect().top,
            height: row.getBoundingClientRect().height,
          })),
        };
      });
      check("nothing was measured", seen !== null, width);
      const { root, chrome, text: at, filterBottom, rows } = seen as NonNullable<typeof seen>;
      const top = rows.filter((one) => one.level === 1);
      check("no top-level row was drawn", top.length >= 3, rows);
      check(
        "no top-level row with chats under it, and none without, was drawn",
        top.some((one) => one.parent) && top.some((one) => !one.parent),
        top,
      );
      // A range, never an exact pixel: within a pixel and a half of the filter's text.
      for (const one of top)
        check(
          `${one.name}: its name does not start where the filter's text does (${width})`,
          Math.abs(one.nameLeft - at) <= 1.5,
          { at, one },
        );
      // Every chat's row is a sidebar view's row (30px at 14px, #3342), give or take a pixel,
      // and the first comes right under the view's top: no empty line between them.
      for (const one of rows)
        check(
          `${one.name}: its row is not the list row's height (${width})`,
          one.height >= chrome - 1 && one.height <= chrome + 1,
          { chrome, one },
        );
      const gap = rows[0].top - filterBottom;
      check(
        `the first row is not right under the filter (${width})`,
        gap >= 0 && gap <= 0.5 * root,
        {
          gap,
          root,
        },
      );
    }
  });

  it("draws every chat's row one line high, its mark first and its name cut short (#1675)", async () => {
    check("there was no left region to draw in", await draw(ONE_LINE, FLOOR), FLOOR);
    whole(await measured(), 10);

    const rows = await browser.execute(() => {
      const section = document.querySelector('[data-raised="chats-list.e2e"]');
      return [...(section?.querySelectorAll<HTMLElement>(".chat") ?? [])].map((chat) => {
        const line = chat.querySelector(".ui-row-label")?.getBoundingClientRect();
        const mark = chat.querySelector(".chat-persona")?.getBoundingClientRect();
        const name = chat.querySelector(".session")?.getBoundingClientRect();
        return {
          name: chat.querySelector(".session")?.textContent ?? "",
          height: Math.round(chat.getBoundingClientRect().height),
          lineHeight: Math.round(line?.height ?? 0),
          markLeft: mark?.left ?? 0,
          nameLeft: name?.left ?? 0,
          markTop: mark?.top ?? 0,
          nameBottom: name?.bottom ?? 0,
        };
      });
    });
    check("no chat row was drawn", rows.length >= 8, rows);
    // One line each, and all the same height, however long the name.
    check(
      "a chat's row is not one line high",
      new Set(rows.map((one) => one.height)).size === 1 && rows[0].height > 0,
      rows,
    );
    for (const one of rows) {
      check(`${one.name}: its mark is not before its name`, one.markLeft < one.nameLeft, one);
      check(`${one.name}: its mark is not on its name's line`, one.markTop < one.nameBottom, one);
    }
  });
});
