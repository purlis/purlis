// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect, $$ } from "@wdio/globals";
import { READY } from "../harness.js";
import { bringSideBack, putSideAway, sideIsAway, type SideRegion } from "../sides.js";
import {
  SELECTED_TAB,
  bringChatForward,
  endChatsStartedHere,
  startAChat,
  type Started,
} from "../started.js";

/**
 * **A session's tab wears a chip of its tasks, measured and driven in the real WebView**
 * (#1487).
 *
 * Three things here are the engine's and not jsdom's: how wide a tab is with and without a
 * chip, what gives way on a crowded strip, and what real timers do with a pointer that crosses
 * the strip against one that rests on a chip. `TabChip.window.test.tsx` holds everything else,
 * on fake clocks.
 *
 * **The tasks are pretended.** A task needs a persona chat and a core that dispatched it, which
 * this suite's fake harness cannot make (`pane-crumbs.e2e.ts` says the same). A chip is what it
 * does under a pointer, so it cannot be drawn by hand as that spec draws its breadcrumb: the
 * `e2e` build listens for the chats to list as tasks (`src/e2eTasks.ts`), and the chip, its
 * menu and their timers are the app's own from there.
 *
 * **The pointer is dispatched, not performed**, for the reason `workspace-explorer.e2e.ts`
 * gives: a WebDriver pointer action on the embedded driver raises no pointer events the page
 * sees. What is sent is what the platform sends as a pointer goes from one element to the
 * next: it leaves the one for the other, and moves.
 *
 * **So the dispatched pointer is the only one on the strip** (#3521). The runner's own cursor
 * is a real one, wherever the machine left it, and no WebDriver call moves it: the embedded
 * driver's pointer actions are page `MouseEvent`s (`tauri-plugin-wdio-webdriver`'s desktop
 * executor). When a chip is drawn under that cursor the engine sends its own pointer events at
 * the cursor's point, which is not the dispatched one, and the chip reads the two as a hand that
 * moved and rests (the menu that opened under a parked pointer, once). {@link fenceRealPointer}
 * stops the engine's own pointer events for this file, and keeps what it stopped for a failure
 * to say.
 */

type Box = { left: number; right: number; top: number; bottom: number; width: number };

/** Where the pretended tasks are said to work. Not the session's workspace, so every task's
 *  row also says where: the widest a row gets. */
const ELSEWHERE = "another-workspace";

const STRIP = '[data-strip="Tabs"]';

async function tabNames(): Promise<string[]> {
  return browser.execute(
    (strip: string) =>
      [...(document.querySelector(strip)?.querySelectorAll('[role="tab"]') ?? [])].map(
        (tab) => tab.querySelector(".tab-name")?.textContent ?? "",
      ),
    STRIP,
  );
}

/** The strip as it is drawn: every tab's name, the one in front, and the chats on screen. */
async function strip(): Promise<{ tabs: string[]; front: string | null; panes: number[] }> {
  return browser.execute((at: string) => {
    const tabs = [...(document.querySelector(at)?.querySelectorAll('[role="tab"]') ?? [])];
    const name = (tab: Element) => tab.querySelector(".tab-name")?.textContent ?? "";
    const front = tabs.find((tab) => tab.getAttribute("aria-selected") === "true");
    return {
      tabs: tabs.map(name),
      front: front ? name(front) : null,
      panes: [...document.querySelectorAll('[data-testid="pane"]')].map((pane) =>
        Number(pane.getAttribute("data-session") ?? -1),
      ),
    };
  }, STRIP);
}

async function untilShows(text: string): Promise<void> {
  await browser.waitUntil(
    async () => {
      const panes = await $$('[data-testid="pane"]').getElements();
      if (panes.length === 0) return false;
      return (await panes[0].$(".xterm-rows").getText()).includes(text);
    },
    { timeout: 30_000, interval: 250, timeoutMsg: `the pane never showed ${text}` },
  );
}

/**
 * Says which chats the window lists as tasks of session `asker`: `how` is one entry per task,
 * `working`, `failed` or `done`. An empty list takes every pretended task away.
 */
async function pretend(asker: number, how: ("working" | "failed" | "done")[]): Promise<void> {
  await browser.execute(
    (of: number, each: string[], workspace: string) => {
      const tasks = each.map((outcome, at) => ({
        workspace,
        chat: {
          session: 9000 + at,
          name: String(9000 + at),
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
          label: `pretended task ${at + 1}`,
          from: {
            chat: of,
            name: "its session",
            workspace,
            task: true,
            tab: false,
            reported: outcome !== "working",
            unreported: false,
            outcome: outcome === "working" ? null : outcome,
          },
        },
      }));
      window.dispatchEvent(new CustomEvent("purlis-e2e-tasks", { detail: tasks }));
    },
    asker,
    how,
    ELSEWHERE,
  );
}

/** The tab menu's longest footer (#1545): two of six running, and at its token limit. */
const LONGEST_FOOTER = "2 of 6 running · at its token limit (1.2M of 1M)";

/**
 * Says the limit numbers session `of` is listed with (`src/e2eTasks.ts`, `PRETEND_LIMITS`): two
 * of six tasks running, and a token limit it is at, which is never the footer's own count, so
 * the footer says both. `false` takes them away.
 */
async function pretendLimits(of: number, past: boolean): Promise<void> {
  await browser.execute(
    (session: number, on: boolean) => {
      const limits = on
        ? [
            {
              session,
              tasks_limit: 6,
              tasks_running: 2,
              at_limit: {
                limit: 1_000_000,
                row: "at its token limit (1.2M of 1M)",
                said: "This session is at its token limit (1.2M of 1M tokens).",
                own: false,
              },
            },
          ]
        : [];
      window.dispatchEvent(new CustomEvent("purlis-e2e-limits", { detail: limits }));
    },
    of,
    past,
  );
}

/** Every drawn tab's cell, by its name: its box, and whether it wears a chip. */
async function cells(): Promise<Record<string, { box: Box; chip: boolean }>> {
  return browser.execute((strip: string) => {
    const out: Record<string, { box: Box; chip: boolean }> = {};
    for (const cell of document.querySelectorAll<HTMLElement>(`${strip} > .tab`)) {
      const name = cell.querySelector(".tab-name")?.textContent;
      if (!name) continue;
      const { left, right, top, bottom, width } = cell.getBoundingClientRect();
      out[name] = {
        box: { left, right, top, bottom, width },
        chip: cell.querySelector(".tab-tasks") !== null,
      };
    }
    return out;
  }, STRIP);
}

/** What the cell of the tab called `name` holds, measured. */
async function measured(name: string) {
  return browser.execute(
    (strip: string, called: string) => {
      const box = (el: Element | null | undefined): Box | null => {
        if (!el) return null;
        const { left, right, top, bottom, width } = el.getBoundingClientRect();
        return { left, right, top, bottom, width };
      };
      const cell = [...document.querySelectorAll<HTMLElement>(`${strip} > .tab`)].find(
        (one) => one.querySelector(".tab-name")?.textContent === called,
      );
      const chip = cell?.querySelector<HTMLElement>(".tab-tasks");
      const counts = chip?.querySelector<HTMLElement>(".tab-tasks-counts");
      const label = cell?.querySelector<HTMLElement>(".tab-name");
      const rem = Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
      const chipBox = box(chip);
      return {
        rem,
        cell: box(cell),
        tab: box(cell?.querySelector('[role="tab"]')),
        chip: chipBox,
        closer: box(cell?.querySelector(".closer")),
        // The counts the chip shows, which are the ones on its one line.
        shown: [...(counts?.querySelectorAll<HTMLElement>(".count") ?? [])]
          .filter((count) => {
            const at = count.getBoundingClientRect();
            return chipBox !== null && at.top < chipBox.bottom - 1 && at.right <= chipBox.right + 1;
          })
          .map((count) => count.getAttribute("data-count")),
        all: [...(counts?.querySelectorAll<HTMLElement>(".count") ?? [])].map((count) =>
          count.getAttribute("data-count"),
        ),
        nameCut: label ? label.scrollWidth > label.clientWidth + 1 : false,
        // How wide the title is drawn: a tab gives up its counts before its whole title (#1867).
        nameWidth: label ? label.getBoundingClientRect().width : 0,
        said: counts?.getAttribute("aria-label") ?? null,
      };
    },
    STRIP,
    name,
  );
}

/** The window global the fence keeps its listener and what it stopped on. */
const FENCE = "__purlisTabChipFence";

/** The window global the parked case keeps what the counts heard on, and how to stop hearing. */
const HEARD = "__purlisTabChipHeard";

/** The window global {@link pointer} stamps, on the page's own clock, when it last moved onto a
 *  chip's counts, and {@link watchForMenu} when a task menu was first drawn after that (#3660). */
const RESTED = "__purlisTabChipRested";

/** How long a rest on a chip is: `REST_MS` in `src/tabTasks.ts`, which this file cannot import
 *  (it would bring the app's modules into the runner). */
const REST_MS = 350;

/** How many events the fence and the parked case's counts keep: the last ones, for a failure. */
const KEPT = 20;

/** The pointer events the engine sends of the runner's own cursor, which the fence stops. */
const REAL_POINTER = [
  "pointerover",
  "pointerout",
  "pointermove",
  "pointerdown",
  "pointerup",
  "pointercancel",
];

/**
 * **The runner's own cursor is fenced off the window** (`on`), or let in again: every pointer
 * event the engine sends itself (`isTrusted`) is stopped before the app hears it, and the last
 * {@link KEPT} are kept, each with its point and what it was on. A dispatched event is never
 * stopped. Putting it up again starts what it keeps afresh (#3610), so a case reads its own.
 */
async function fenceRealPointer(on: boolean): Promise<void> {
  await browser.execute(
    (key: string, types: string[], up: boolean, kept: number) => {
      const held = window as unknown as Record<string, unknown>;
      const was = held[key] as { stop: (event: Event) => void } | undefined;
      if (was) for (const type of types) window.removeEventListener(type, was.stop, true);
      held[key] = undefined;
      if (!up) return;
      const stopped: string[] = [];
      const stop = (event: Event) => {
        if (!event.isTrusted) return;
        event.stopImmediatePropagation();
        const at = event as PointerEvent;
        const what = at.target instanceof Element ? at.target.className : String(at.target);
        stopped.push(`${at.type} at ${at.clientX},${at.clientY} on ${what}`);
        if (stopped.length > kept) stopped.shift();
      };
      for (const type of types) window.addEventListener(type, stop, true);
      held[key] = { stop, stopped };
    },
    FENCE,
    REAL_POINTER,
    on,
    KEPT,
  );
}

/** The parked case's counts stop hearing, and what they heard goes (#3610). */
async function unhear(): Promise<void> {
  await browser.execute((key: string) => {
    const held = window as unknown as Record<string, { off?: () => void } | undefined>;
    held[key]?.off?.();
    held[key] = undefined;
  }, HEARD);
}

/** What the fence has stopped since it went up: the runner's cursor's events, in order. */
async function stoppedByFence(): Promise<string[]> {
  return browser.execute(
    (key: string) =>
      ((window as unknown as Record<string, { stopped?: string[] } | undefined>)[key]?.stopped ?? [
        "(no fence)",
      ]) as string[],
    FENCE,
  );
}

/** How many task menus are open. */
async function menus(): Promise<number> {
  return browser.execute(() => document.querySelectorAll('.tasks-menu[data-part="root"]').length);
}

/**
 * **When a task menu opens is read on the page's clock, not the runner's** (#3660). A pause
 * on the runner followed by a look at the menus says nothing on a slow runner: its look can
 * be answered after the rest is long over. This watches the page from now on and stamps, with
 * `performance.now()`, the first moment a task menu is drawn, beside the moment
 * {@link pointer} last came onto a chip. Answers whether a menu was already open.
 */
async function watchForMenu(): Promise<boolean> {
  return browser.execute((key: string) => {
    const held = window as unknown as Record<string, unknown>;
    const was = held[key] as { observer?: MutationObserver } | undefined;
    was?.observer?.disconnect();
    const open = () => document.querySelector('.tasks-menu[data-part="root"]') !== null;
    const watch: { arrived?: number; opened?: number; observer?: MutationObserver } = {};
    watch.observer = new MutationObserver(() => {
      if (watch.opened !== undefined || !open()) return;
      watch.opened = performance.now();
      watch.observer?.disconnect();
    });
    watch.observer.observe(document.body, { childList: true, subtree: true, attributes: true });
    held[key] = watch;
    return open();
  }, RESTED);
}

/** How long after the pointer came onto the chip its menu was drawn, in the page's own
 *  milliseconds, once {@link watchForMenu} has seen both; or why it cannot say. */
async function restTook(): Promise<number | string> {
  return browser.execute((key: string) => {
    const watch = (window as unknown as Record<string, { arrived?: number; opened?: number }>)[key];
    if (watch === undefined) return "nothing was watching for the menu";
    if (watch.arrived === undefined) return "the pointer never came onto the chip";
    if (watch.opened === undefined) return "no menu was drawn";
    return watch.opened - watch.arrived;
  }, RESTED);
}

/**
 * Takes the pointer along the strip, onto each thing in it in turn and off the end: every
 * tab, every chip's hand and counts, every close. `dwell` is how long it stays on each, in
 * milliseconds. Answers how many things it went over, and how many of them were chips.
 */
async function cross(dwell: number): Promise<{ over: number; chips: number }> {
  return browser.execute(
    async (strip: string, stay: number) => {
      const stops = [
        ...document.querySelectorAll<HTMLElement>(
          `${strip} [role="tab"], ${strip} .tab-tasks button, ${strip} .closer`,
        ),
      ].sort((a, b) => a.getBoundingClientRect().left - b.getBoundingClientRect().left);
      const wait = (ms: number) => new Promise((done) => setTimeout(done, ms));
      const send = (on: Element, type: string, related: Element | null) => {
        const at = on.getBoundingClientRect();
        on.dispatchEvent(
          new PointerEvent(type, {
            bubbles: true,
            cancelable: true,
            pointerId: 1,
            pointerType: "mouse",
            isPrimary: true,
            clientX: at.left + at.width / 2,
            clientY: at.top + at.height / 2,
            relatedTarget: related,
          }),
        );
      };
      let last: Element | null = null;
      for (const stop of stops) {
        if (last === null) send(stop, "pointerover", null);
        else send(last, "pointerout", stop);
        // A move at its own point on each thing: the pointer is moving, which is what would
        // start a rest on a chip if it then stayed.
        send(stop, "pointermove", null);
        last = stop;
        await wait(stay);
      }
      if (last !== null) send(last, "pointerout", null);
      return {
        over: stops.length,
        chips: stops.filter((stop) => stop.classList.contains("tab-tasks-counts")).length,
      };
    },
    STRIP,
    dwell,
  );
}

/** Puts the pointer on the counts of the chip on the tab called `name`, or takes it off. */
async function pointer(name: string, what: "on" | "off"): Promise<boolean> {
  return browser.execute(
    (strip: string, called: string, how: string, rested: string) => {
      const cell = [...document.querySelectorAll<HTMLElement>(`${strip} > .tab`)].find(
        (one) => one.querySelector(".tab-name")?.textContent === called,
      );
      const counts = cell?.querySelector<HTMLElement>(".tab-tasks-counts");
      if (!counts) return false;
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
            relatedTarget: null,
          }),
        );
      if (how === "on") {
        // Onto it, and then a move to another point: a rest is counted from a move the
        // person made, never from the pointer merely being found there.
        send("pointerover");
        // The rest is counted from this move: when it was, for a case that watches the menu.
        const watch = (window as unknown as Record<string, { arrived?: number } | undefined>)[
          rested
        ];
        if (watch) watch.arrived = performance.now();
        send("pointermove", 2);
      } else send("pointerout");
      return true;
    },
    STRIP,
    name,
    what,
    RESTED,
  );
}

function check(
  what: string,
  value: number | boolean,
  is: "atLeast" | "atMost" | "is",
  other: number | boolean,
) {
  const holds = is === "is" ? value === other : is === "atLeast" ? value >= other : value <= other;
  if (!holds) throw new Error(`${what}: ${String(value)} is not ${is} ${String(other)}`);
}

/** Whether the theme in force shares the chat strip equally between its tabs (`tabs.layout.sizing:
 *  equal`, Purlis Classic) rather than sizing each to its words (Purlis, #2914). */
async function equalShare(): Promise<boolean> {
  return browser.execute(() => {
    const layout = document.querySelector('[data-scope="tabs"][data-part="layout"]');
    return (
      layout !== null && getComputedStyle(layout).getPropertyValue("--tabs-grow").trim() === "1"
    );
  });
}

describe("the chip a session's tab wears for its tasks", () => {
  /** The tabs that were already there, so only this file's own are ended again. */
  let wereAlreadyOpen: string[] = [];
  let was = { width: 1280, height: 800 };
  /** This file's own tabs, in the order they were opened, and the first one's chat. */
  let mine: string[] = [];
  /** The chats this file started, by the number each start answered, in the same order. */
  let started: Started[] = [];
  let asker = -1;

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
      { timeout: 10_000, interval: 200, timeoutMsg: "the window never settled on a width" },
    );
  }

  async function untilChip(name: string, worn: boolean) {
    await browser.waitUntil(async () => ((await cells())[name]?.chip ?? false) === worn, {
      timeout: 10_000,
      interval: 100,
      timeoutMsg: `${name}'s tab ${worn ? "never wore" : "kept"} a chip`,
    });
  }

  before(async () => {
    // **It starts from a strip of its own.** The strip draws what fits and the tab in front,
    // so what earlier files left open decides which of this file's tabs are drawn, and how
    // wide each is: measured in CI, eleven tabs left behind, and this file's three not drawn.
    // The chatsLeft guard now starts every file with no chat running, so nothing is ended here
    // that this file did not start.
    wereAlreadyOpen = await tabNames();
    // Only the dispatched pointer is ever on the strip (#3521, the file's comment).
    await fenceRealPointer(true);
    was = await browser.getWindowSize();
    await windowIs(1280, 800);
    // **Each tab is waited for by itself, and by its name.** The pane in front already shows
    // the harness's word from the tab before, so "a pane shows it" is true at once: without
    // a wait of its own the next tab was pressed for while this one was still starting. And
    // not by a count of the strip: one app serves the whole run, and a tab the spec before
    // this one was still closing goes while this one opens, so "one more than there were"
    // never comes true (CI, twice). A tab is new when the strip draws a name it had not.
    mine = [];
    started = [];
    for (let opened = 0; opened < 3; opened++) {
      const known = new Set([...wereAlreadyOpen, ...(await tabNames()), ...mine]);
      started.push(await startAChat());
      let saw = "nothing was read";
      let came: string | undefined;
      await browser
        .waitUntil(
          async () => {
            const now = await strip();
            saw = JSON.stringify(now);
            came = now.tabs.find((name) => name !== "" && !known.has(name));
            return came !== undefined;
          },
          {
            timeout: 30_000,
            interval: 250,
            timeoutMsg: `tab ${opened + 1} of 3 never came onto the strip`,
          },
        )
        .catch((err: unknown) => {
          // What the strip held, so a red run says what it saw.
          throw new Error(
            `${String(err)}; before the press: ${JSON.stringify([...known])}; last seen: ${saw}`,
          );
        });
      await untilShows(READY);
      // Its name is read once it has stopped changing: a tab is named before its chat has
      // told the window what it is called.
      let last = "";
      await browser
        .waitUntil(
          async () => {
            const names = (await tabNames()).filter((name) => name !== "" && !known.has(name));
            const settled = names.length === 1 && names[0] === last;
            last = names[0] ?? "";
            return settled;
          },
          {
            timeout: 10_000,
            interval: 400,
            timeoutMsg: `tab ${opened + 1} of 3 never settled on a name`,
          },
        )
        .catch(async (err: unknown) => {
          throw new Error(`${String(err)}; the strip: ${JSON.stringify(await strip())}`);
        });
      mine.push(last);
    }
    // All three are drawn: a strip that had hidden one would be measuring something else.
    const drawn = await strip();
    if (!mine.every((name) => drawn.tabs.includes(name)))
      throw new Error(
        `the strip does not draw this file's three tabs ${JSON.stringify(mine)}: ${JSON.stringify(drawn)}`,
      );
    // The first one's chat, by the number its start answered: the tab is only how it is drawn.
    asker = started[0].session;
    await bringChatForward(asker);
    // The cells are measured by the names they draw: the first name is its chat's own tab's.
    const front = await browser.execute(
      (selected: string) =>
        document.querySelector(selected)?.querySelector(".tab-name")?.textContent ?? "",
      SELECTED_TAB,
    );
    if (front !== mine[0])
      throw new Error(`chat ${asker}'s own tab says ${front}, not the first name read ${mine[0]}`);
  });

  afterEach(async () => {
    await pointer(mine[0], "off");
    await pretendLimits(asker, false);
    await pretend(asker, []);
    await windowIs(1280, 800);
  });

  // One app process serves the whole run: nothing pretended is left, the window goes back to
  // the size it had, and every chat this file opened is ended.
  // **Every step is tried whatever became of the one before it**: a case that failed half
  // way must not leave its chats for the specs that share this app.
  after(async () => {
    const tried = async (step: () => Promise<unknown>) => {
      try {
        await step();
      } catch {
        // The next step still runs; the wait below says what was left.
      }
    };
    await tried(() => pretend(asker, []));
    await tried(() => fenceRealPointer(false));
    await tried(() => unhear());
    await tried(() => browser.keys("Escape"));
    await tried(() => browser.setWindowSize(was.width, was.height));
    await tried(() => endChatsStartedHere(started));
    await browser.waitUntil(
      async () => (await tabNames()).every((tab) => wereAlreadyOpen.includes(tab)),
      { timeout: 20_000, timeoutMsg: "the tab-chip spec left a chat open behind it" },
    );
  });

  it("leaves every tab the width it was, the one with tasks and the ones without", async () => {
    expect(mine.length).toBeGreaterThanOrEqual(3);
    // As the strip was before any tab had a task: what it is without this change.
    const before = await cells();
    for (const name of mine)
      check(`${name} wore a chip with no tasks`, before[name].chip, "is", false);

    await pretend(asker, ["working", "working", "failed", "done", "done", "done"]);
    await untilChip(mine[0], true);
    const after = await cells();

    // Shared equally (Purlis Classic), no tab changes width or moves for a chip. Sized to their
    // words (Purlis, #2914), the tab that wears it grows by it, and a strip at its room shares
    // the difference out among its tabs, which shrink as their words allow: none is lost.
    const equal = await equalShare();
    for (const [name, { box }] of Object.entries(before)) {
      const now = after[name];
      check(`${name} is gone from the strip`, now !== undefined, "is", true);
      if (!equal) continue;
      check(`${name} changed width`, Math.abs(now.box.width - box.width), "atMost", 0.5);
      check(`${name} moved`, Math.abs(now.box.left - box.left), "atMost", 0.5);
    }
    // Only the session that has tasks wears one.
    for (const name of mine.slice(1)) check(`${name} wears a chip`, after[name].chip, "is", false);

    const seen = await measured(mine[0]);
    if (seen.shown.length !== 3)
      console.log(`tab-chip.e2e: the chip with room was ${JSON.stringify(seen)}`);
    expect(seen.said).toContain("2 working, 1 failed, 3 done");
    // The chip shows every count the tab has room for beside its drawn title, from the start
    // and whole (#1867: the title is never squeezed out for a count), and sits inside its
    // tab's own cell, between the tab's button and its close.
    check("the tab's title is not drawn", seen.nameWidth, "atLeast", 1);
    expect(["working", "failed", "done"].slice(0, seen.shown.length)).toEqual(seen.shown);
    check("the chip shows no count", seen.shown.length, "atLeast", 1);
    const [cell, tab, chip, closer] = [seen.cell, seen.tab, seen.chip, seen.closer] as Box[];
    check("the chip starts over the tab's button", chip.left, "atLeast", tab.right - 1);
    check("the chip runs under the close", chip.right, "atMost", closer.left + 1);
    check("the close left its cell", closer.right, "atMost", cell.right + 1);
  });

  it("opens no menu for a pointer that crosses the strip", async () => {
    await pretend(asker, ["working", "done"]);
    await untilChip(mine[0], true);

    // Quickly, as a pointer on its way somewhere else, and slowly, at a third of the rest on
    // each thing: neither is a rest on the chip.
    for (const dwell of [16, 110]) {
      const went = await cross(dwell);
      check("the pointer met no chip on its way", went.chips, "atLeast", 1);
      check("the pointer crossed nothing", went.over, "atLeast", mine.length * 2);
      // Long past the rest, in case anything was still counting.
      await browser.pause(900);
      expect(await menus()).toBe(0);
    }
  });

  it("opens no menu for a chip that appears under a pointer that is not moving", async () => {
    // The pointer is parked where the chip will be drawn, and then the session gets tasks:
    // the engine finds the pointer on the new chip, and nobody's hand moved.
    await pretend(asker, ["working"]);
    await untilChip(mine[0], true);
    // What the fence stopped before this case is the cases' before it.
    await fenceRealPointer(true);
    const parked = await browser.execute(
      (strip: string, key: string, kept: number) => {
        const counts = document.querySelector<HTMLElement>(`${strip} .tab-tasks-counts`);
        if (!counts) return false;
        const at = counts.getBoundingClientRect();
        const here = {
          bubbles: true,
          cancelable: true,
          pointerId: 1,
          pointerType: "mouse",
          isPrimary: true,
          clientX: at.left + at.width / 2,
          clientY: at.top + at.height / 2,
        };
        // The last pointer events the counts hear from here, for a failure to say which moved
        // them; `off` stops hearing them, as the case ends.
        const heard: string[] = [];
        const hear = (event: Event) => {
          const one = event as PointerEvent;
          heard.push(
            `${one.isTrusted ? "the engine's" : "dispatched"} ${one.type} at ${one.clientX},${one.clientY}`,
          );
          if (heard.length > kept) heard.shift();
        };
        const types = ["pointerover", "pointermove", "pointerout"];
        for (const type of types) counts.addEventListener(type, hear);
        const off = () => {
          for (const type of types) counts.removeEventListener(type, hear);
        };
        (window as unknown as Record<string, unknown>)[key] = { heard, off };
        counts.dispatchEvent(new PointerEvent("pointerover", { ...here, relatedTarget: null }));
        // The same point again, as an engine says "still here" after a layout.
        counts.dispatchEvent(new PointerEvent("pointermove", here));
        counts.dispatchEvent(new PointerEvent("pointermove", here));
        return true;
      },
      STRIP,
      HEARD,
      KEPT,
    );
    try {
      expect(parked).toBe(true);

      await browser.pause(1_200);
      if ((await menus()) !== 0) {
        // Which pointer moved it (#3521): what the counts heard, and what the fence stopped.
        const heard = await browser.execute(
          (key: string) =>
            (window as unknown as Record<string, { heard?: string[] } | undefined>)[key]?.heard ??
            [],
          HEARD,
        );
        throw new Error(
          `a menu opened under a pointer parked 1.2 s; the counts heard ${JSON.stringify(heard)}; ` +
            `the fence stopped ${JSON.stringify(await stoppedByFence())}`,
        );
      }
    } finally {
      await unhear();
    }
  });

  it("opens the menu once the pointer has rested on the chip, and closes it once it has gone", async () => {
    await pretend(asker, ["working", "failed", "done"]);
    await untilChip(mine[0], true);

    expect(await watchForMenu()).toBe(false);
    expect(await pointer(mine[0], "on")).toBe(true);
    await browser.waitUntil(async () => (await menus()) === 1, {
      timeout: 5_000,
      interval: 50,
      timeoutMsg: "resting on the chip opened no menu",
    });
    // Not at once: the menu was drawn no sooner than a whole rest after the pointer came, on
    // the page's clock, however long the runner took to look (#3660). A millisecond either
    // side for the clock's coarsening.
    const took = await restTook();
    if (typeof took === "string") throw new Error(`the rest was not timed: ${took}`);
    check("the menu opened before the pointer had rested, ms", took, "atLeast", REST_MS - 2);

    const seen = await browser.execute(() => {
      const menu = document.querySelector<HTMLElement>('.tasks-menu[data-part="root"]');
      const chip = document.querySelector<HTMLElement>(".tab-tasks-counts");
      if (!menu || !chip) return null;
      const at = menu.getBoundingClientRect();
      return {
        rows: [...menu.querySelectorAll('[role="menuitem"] .name')].map((row) => row.textContent),
        // A state word that is cut is wider inside than its box.
        cut: [...menu.querySelectorAll<HTMLElement>(".shown-state .word")]
          .filter((word) => word.scrollWidth > word.clientWidth + 1)
          .map((word) => word.textContent),
        below: at.top >= chip.getBoundingClientRect().bottom - 1,
        inside: at.left >= 0 && at.right <= window.innerWidth && at.bottom <= window.innerHeight,
        // It took no keyboard: the pointer opened it, and whoever was typing still is.
        keyboardInIt: menu.contains(document.activeElement),
      };
    });
    expect(seen).not.toBeNull();
    // The session's own chat first, then the tasks; the finished one is in the fold.
    expect(seen?.rows.slice(0, 1)).toEqual([mine[0]]);
    expect(seen?.rows).toContain("pretended task 1");
    expect(seen?.rows).toContain("pretended task 2");
    expect(seen?.rows).toContain("Finished (1)");
    expect(seen?.cut).toEqual([]);
    expect(seen?.below).toBe(true);
    expect(seen?.inside).toBe(true);
    expect(seen?.keyboardInIt).toBe(false);

    await pointer(mine[0], "off");
    await browser.waitUntil(async () => (await menus()) === 0, {
      timeout: 5_000,
      interval: 50,
      timeoutMsg: "the menu stayed after the pointer had left the chip",
    });
  });

  it("keeps a rest-opened menu while the pointer goes onto End a task and into its rows", async () => {
    // The ways to end a task are in a menu of their own, drawn in a portal beside the first
    // (#1488 in #1487's menu). A pointer going from the menu into it has not left the menu:
    // the first must stay, and the second must be drawn.
    await pretend(asker, ["working", "working"]);
    await untilChip(mine[0], true);
    expect(await pointer(mine[0], "on")).toBe(true);
    await browser.waitUntil(async () => (await menus()) === 1, {
      timeout: 5_000,
      interval: 50,
      timeoutMsg: "resting on the chip opened no menu",
    });

    // Off the chip and onto the line that opens the ways to end a task, as a hand moves.
    const onTheLine = await browser.execute(() => {
      const counts = document.querySelector<HTMLElement>(".tab-tasks-counts");
      const line = document.querySelector<HTMLElement>(".tasks-menu .tasks-menu-end");
      if (!counts || !line) return false;
      const at = line.getBoundingClientRect();
      const here = {
        bubbles: true,
        cancelable: true,
        pointerId: 1,
        pointerType: "mouse",
        isPrimary: true,
        clientX: at.left + at.width / 2,
        clientY: at.top + at.height / 2,
      };
      counts.dispatchEvent(new PointerEvent("pointerout", { ...here, relatedTarget: line }));
      line.dispatchEvent(new PointerEvent("pointerover", { ...here, relatedTarget: counts }));
      line.dispatchEvent(new PointerEvent("pointerenter", { ...here, bubbles: false }));
      line.dispatchEvent(new PointerEvent("pointermove", here));
      line.dispatchEvent(new PointerEvent("pointermove", { ...here, clientX: here.clientX + 2 }));
      return true;
    });
    expect(onTheLine).toBe(true);
    await browser
      .waitUntil(
        async () =>
          browser.execute(
            () => document.querySelector('.tasks-menu-ends[data-part="root"]') !== null,
          ),
        { timeout: 5_000, interval: 50, timeoutMsg: "the ways to end a task never opened" },
      )
      .catch(async (err: unknown) => {
        const menu = await browser.execute(() => ({
          menus: document.querySelectorAll('[role="menu"]').length,
          rows: [...document.querySelectorAll('.tasks-menu [role="menuitem"]')].map(
            (row) => `${row.className}: ${row.textContent ?? ""}`,
          ),
          line: document.querySelector(".tasks-menu-end")?.outerHTML.slice(0, 300) ?? null,
        }));
        throw new Error(`${String(err)}; the menu: ${JSON.stringify(menu)}`);
      });

    // Into its rows, and well past the time a menu left by the pointer would have closed.
    const inside = await browser.execute(() => {
      const from = document.querySelector<HTMLElement>(".tasks-menu .tasks-menu-end");
      const row = document.querySelector<HTMLElement>('.tasks-menu-ends [role="menuitem"]');
      if (!from || !row) return false;
      const at = row.getBoundingClientRect();
      const here = {
        bubbles: true,
        cancelable: true,
        pointerId: 1,
        pointerType: "mouse",
        isPrimary: true,
        clientX: at.left + at.width / 2,
        clientY: at.top + at.height / 2,
      };
      from.dispatchEvent(new PointerEvent("pointerout", { ...here, relatedTarget: row }));
      row.dispatchEvent(new PointerEvent("pointerover", { ...here, relatedTarget: from }));
      row.dispatchEvent(new PointerEvent("pointermove", here));
      return true;
    });
    expect(inside).toBe(true);
    await browser.pause(900);
    const still = await browser.execute(() => ({
      first: document.querySelector('.tasks-menu[data-part="root"]:not(.tasks-menu-ends)') !== null,
      ways: [...document.querySelectorAll('.tasks-menu-ends [role="menuitem"] .name')].map(
        (row) => row.textContent ?? "",
      ),
      // What stands just before the last row: the line Stop all tasks sits under (#1498).
      beforeLast:
        document
          .querySelector(".tasks-menu-ends .tasks-menu-stop-all")
          ?.previousElementSibling?.getAttribute("role") ?? null,
    }));
    expect(still.first).toBe(true);
    // Each open task's two ways, by the catalogue's own titles, and nothing was ended.
    const each = still.ways.slice(0, -1);
    expect(each).toHaveLength(4);
    expect(each.every((title) => /^(Stop and get its report|Close now): task /.test(title))).toBe(
      true,
    );
    // Then Stop all tasks, last and under a line of its own (#1498, D-1498-8).
    expect(still.ways.at(-1)).toBe("Stop all tasks");
    expect(still.beforeLast).toBe("separator");
    await browser.keys("Escape");
    await browser.keys("Escape");
  });

  it("gives up the name before a count on a crowded strip, and overflows nothing", async () => {
    await pretend(asker, ["working", "working", "failed", "done", "done", "done"]);
    await untilChip(mine[0], true);

    // The chat strip is the editor's tab bar since DS-6 (#1836), as wide as the centre column
    // and not the window: both sidebars are put away for this, so narrowing the window narrows
    // the strip as it did, and they are brought back after. Below the 1024px minimum (ADR 0054)
    // with both open, the centre is not a strip's room for a tab with a chip.
    const putAway = await sidesAway();
    try {
      await narrowerAndNarrower();
    } finally {
      await sidesBack(putAway);
    }
  });

  /** Each sidebar that is open, put away as a person does (`sides.ts`); the view each showed. */
  async function sidesAway(): Promise<string[]> {
    const pressed: string[] = [];
    for (const bar of ["Navigation", "Attention"] as const) {
      if (await sideIsAway(bar)) continue;
      pressed.push(`${bar}\u0000${await putSideAway(bar)}`);
    }
    return pressed;
  }

  /** The sidebars `sidesAway` put away, open again on the view each was showing. */
  async function sidesBack(pressed: string[]): Promise<void> {
    for (const one of pressed) {
      const [bar, name] = one.split("\u0000") as [SideRegion, string];
      await bringSideBack(bar, name);
    }
  }

  async function narrowerAndNarrower() {
    // Narrower and narrower: the tabs go down to the least a tab is drawn at.
    for (const width of [900, 700, 560]) {
      await windowIs(width, 800);
      // The tab in front is always drawn, whatever the strip hides.
      await bringChatForward(asker);
      await untilChip(mine[0], true);
      const seen = await measured(mine[0]);
      const [cell, tab, chip, closer] = [seen.cell, seen.tab, seen.chip, seen.closer] as Box[];
      const at = `at ${width}px`;
      // A red run says the cell it measured.
      console.log(`tab-chip.e2e: ${at} the tab with a chip was ${JSON.stringify(seen)}`);

      // Its share of the strip is the share of every other tab drawn, where the theme shares
      // the strip equally (Purlis Classic); sized to their words (Purlis), each is its own.
      for (const [name, other] of (await equalShare()) ? Object.entries(await cells()) : [])
        check(
          `${at}: ${name} is not as wide as the tab with a chip`,
          Math.abs(other.box.width - cell.width),
          "atMost",
          1,
        );
      // Nothing in the cell is on top of anything else in it, or outside it.
      check(`${at}: the chip is over the tab's button`, chip.left, "atLeast", tab.right - 1);
      check(`${at}: the chip is under the close`, chip.right, "atMost", closer.left + 1);
      check(`${at}: the close left its cell`, closer.right, "atMost", cell.right + 1);
      check(`${at}: the tab's button starts before its cell`, tab.left, "atLeast", cell.left - 1);
      // The count of what is still working is the last to go, and it never goes.
      check(`${at}: the chip shows no count`, seen.shown.length, "atLeast", 1);
      expect(seen.shown[0]).toBe("working");
      // **The rule**: a count is given up only once the name has given up all it can. While
      // the tab's button is wider than its floor, every count is drawn.
      if (seen.shown.length < seen.all.length) {
        check(
          `${at}: a count went while the name had room to give`,
          tab.width,
          "atMost",
          2 * seen.rem + 1,
        );
        check(`${at}: a count went and the name is whole`, seen.nameCut, "is", true);
      }
      // And the name is never squeezed out: the button keeps its floor.
      check(`${at}: the tab's button is under its floor`, tab.width, "atLeast", 2 * seen.rem - 1);
    }
  }

  // The window lays the pretended numbers over its chats (`src/e2eTasks.ts`, `usePretendLimits`,
  // `withPretendedLimits`, in `PlaneView.tsx`'s `listedChats`). The widths are 1024 px and up:
  // a window is never narrower (ADR 0054), and asking for less settles at 1024.
  it("fits the footer's longest line in the menu, at every width the strip is drawn at (#1545)", async () => {
    await pretend(asker, ["working", "working", "done"]);
    await pretendLimits(asker, true);
    await untilChip(mine[0], true);

    for (const width of [1600, 1280, 1024]) {
      await windowIs(width, 800);
      await bringChatForward(asker);
      await untilChip(mine[0], true);
      expect(await pointer(mine[0], "on")).toBe(true);
      await browser.waitUntil(async () => (await menus()) === 1, {
        timeout: 5_000,
        interval: 50,
        timeoutMsg: `at ${width}px resting on the chip opened no menu`,
      });
      const seen = await browser.execute(() => {
        const menu = document.querySelector<HTMLElement>('.tasks-menu[data-part="root"]');
        const foot = menu?.querySelector<HTMLElement>(".tasks-menu-foot");
        const limits = foot?.querySelector<HTMLElement>(".tasks-menu-limits");
        if (!menu || !foot || !limits) return null;
        const box = (el: Element) => {
          const { left, right, top, bottom } = el.getBoundingClientRect();
          return { left, right, top, bottom };
        };
        return {
          said: limits.textContent ?? "",
          menu: box(menu),
          limits: box(limits),
          // A line cut short is wider inside than its box; a menu that scrolls sideways is.
          cut: limits.scrollWidth - limits.clientWidth,
          // How far it scrolls sideways: nothing where it never does (overflow-x hidden), as
          // a menu line gives way with its own ellipsis.
          menuOverflow:
            getComputedStyle(menu).overflowX === "hidden" ? 0 : menu.scrollWidth - menu.clientWidth,
          // What reaches past the menu's inner edge, if anything: named in a failure.
          past: [...menu.querySelectorAll<HTMLElement>("*")]
            .map((el) => ({
              what: `${el.tagName.toLowerCase()}.${[...el.classList].join(".")}`,
              by: el.getBoundingClientRect().right - (menu.getBoundingClientRect().right - 1),
            }))
            .filter((one) => one.by > 1)
            .sort((a, b) => b.by - a.by)
            .slice(0, 3),
          window: window.innerWidth,
        };
      });
      const at = `at ${width}px`;
      console.log(`tab-chip.e2e: ${at} the menu's footer was ${JSON.stringify(seen)}`);
      expect(seen).not.toBeNull();
      if (seen === null) return;
      expect(seen.said).toBe(LONGEST_FOOTER);
      check(`${at}: the footer's line is cut short`, seen.cut, "atMost", 1);
      check(`${at}: the menu scrolls sideways`, seen.menuOverflow, "atMost", 1);
      check(
        `${at}: something runs past the menu's edge (${JSON.stringify(seen.past)})`,
        seen.past.length,
        "atMost",
        0,
      );
      check(
        `${at}: the line starts before the menu`,
        seen.limits.left,
        "atLeast",
        seen.menu.left - 1,
      );
      check(`${at}: the line runs past the menu`, seen.limits.right, "atMost", seen.menu.right + 1);
      check(
        `${at}: the line runs under the menu`,
        seen.limits.bottom,
        "atMost",
        seen.menu.bottom + 1,
      );
      check(`${at}: the menu starts off the window`, seen.menu.left, "atLeast", -1);
      check(`${at}: the menu runs off the window`, seen.menu.right, "atMost", seen.window + 1);

      await pointer(mine[0], "off");
      await browser.waitUntil(async () => (await menus()) === 0, {
        timeout: 5_000,
        interval: 50,
        timeoutMsg: `at ${width}px the menu stayed after the pointer had left the chip`,
      });
    }
  });

  // **The narrowest centre column: 900 px with both sidebars open** (#1867). The chat strip is
  // the centre column's (DS-6), so its tabs are drawn at their floor, where a tab and its chip
  // do not both fit: the tab gives up its chip before its title, and "N more" reaches every
  // tab the strip does not draw. The extra tabs are this file's own, ended in `after`. Ranges,
  // never pixels.
  it("at 900 px with both sidebars open, keeps a tab's title over its chip, and N more reaches the rest", async () => {
    await pretend(asker, ["working", "working", "failed", "done", "done", "done"]);
    await untilChip(mine[0], true);
    const opened = await sidesOpen();
    try {
      await windowIs(900, 800);
      // More tabs than the strip has room for. Bounded: a strip that never overflows is a
      // failure here, not a hang.
      for (let more = 0; more < 16 && (await showMore()) === null; more++)
        started.push(await startAChat());
      await bringChatForward(asker);
      await untilChip(mine[0], true);

      // The chip is fitted a frame after its cell changes size (`wholeWays.refitOn`): waited
      // for, and on a timeout the checks below say what was measured.
      let at = await narrowest(mine[0]);
      await browser
        .waitUntil(
          async () => {
            at = await narrowest(mine[0]);
            return at.cell !== null && (at.closer?.right ?? Infinity) <= at.cell.right + 1;
          },
          { timeout: 5_000, interval: 100 },
        )
        .catch(() => undefined);
      console.log(`tab-chip.e2e: at 900px with both sidebars open ${JSON.stringify(at)}`);
      check("the window is wider than 1024px", at.window, "atMost", 1024);
      check("the left sidebar is not open", at.left, "atLeast", 1);
      check("the right sidebar is not open", at.right, "atLeast", 1);
      // The tab with tasks is drawn, and its title is on screen inside its cell.
      expect(at.cell).not.toBeNull();
      const cell = at.cell as Box;
      const name = at.name as Box;
      check("the tab's title is not drawn", name.width, "atLeast", 1);
      check("the title starts before its cell", name.left, "atLeast", cell.left - 1);
      check("the title runs past its cell", name.right, "atMost", cell.right + 1);
      // Nothing in the cell runs out of it: the close, and the chip where it is still drawn.
      check("the close left its cell", (at.closer as Box).right, "atMost", cell.right + 1);
      if (at.chip !== null)
        check("the chip left its cell", at.chip.right, "atMost", (at.closer as Box).left + 1);
      // The strip is inside the centre column, collapses rather than scrolls, and says so.
      check("the strip scrolls", at.scrolls, "atMost", 1);
      check("the strip runs past the centre", at.bar.right, "atMost", at.centre.right + 1);
      expect(at.more).not.toBeNull();

      // Selectable: another drawn tab is pressed and comes to the front, then this one again.
      const other = at.drawn.find((one) => one !== mine[0] && one !== "");
      expect(other).toBeDefined();
      await pressTab(other as string);
      await pressTab(mine[0]);

      // "N more" reaches the rest: it lists as many as it says, and a row brings its tab
      // to the front, drawn on the strip.
      // Read again now this tab is in front once more: the tab in front is always drawn.
      const counted = Number(/\d+/.exec((await showMore()) ?? "")?.[0] ?? "0");
      check("N more hides no tab", counted, "atLeast", 1);
      await (await $(`header.bar button[aria-label^="Show "]`)).click();
      let listed = 0;
      await browser.waitUntil(
        async () => {
          listed = (await $$('.more-menu [role="menuitem"]').getElements()).length;
          return listed === counted;
        },
        { timeout: 10_000, timeoutMsg: `N more said ${counted} and listed ${listed}` },
      );
      const row = (await $$('.more-menu [role="menuitem"]').getElements())[0];
      const going = (await row.$(".tab-name").getText()).trim();
      await row.click();
      await untilSelected(going);
    } finally {
      await sidesBack(opened);
    }
  });

  /** Each sidebar that is put away, opened by a press of a view's icon (its open one's, else
   *  the first); the icons pressed, as {@link sidesBack} takes them. */
  async function sidesOpen(): Promise<string[]> {
    const pressed: string[] = [];
    for (const [bar, slot] of [
      ["Navigation", "region-left"],
      ["Attention", "region-right"],
    ]) {
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

  /** The chat strip's "N more" button's words, or null while it draws every tab. */
  async function showMore(): Promise<string | null> {
    return browser.execute(
      () =>
        document
          .querySelector('header.bar button[aria-label^="Show "]')
          ?.getAttribute("aria-label") ?? null,
    );
  }

  /** Presses the drawn tab called `name`, and waits until it is the one in front. */
  async function pressTab(name: string): Promise<void> {
    await browser.execute(
      (strip: string, called: string) => {
        const tab = [...document.querySelectorAll<HTMLElement>(`${strip} [role="tab"]`)].find(
          (one) => one.querySelector(".tab-name")?.textContent === called,
        );
        tab?.click();
      },
      STRIP,
      name,
    );
    await untilSelected(name);
  }

  async function untilSelected(name: string): Promise<void> {
    await browser.waitUntil(
      async () =>
        (await browser.execute(
          (selected: string) =>
            document.querySelector(selected)?.querySelector(".tab-name")?.textContent ?? "",
          SELECTED_TAB,
        )) === name,
      { timeout: 10_000, timeoutMsg: `${name} never came to the front` },
    );
  }

  /** The strip at its narrowest: the cell of the tab called `name` and what it holds, the
   *  regions around it, and "N more". */
  async function narrowest(name: string) {
    return browser.execute(
      (strip: string, called: string) => {
        const box = (el: Element | null | undefined) => {
          if (!el) return null;
          const { left, right, top, bottom, width } = el.getBoundingClientRect();
          return { left, right, top, bottom, width };
        };
        const widthOf = (selector: string) =>
          document.querySelector(selector)?.getBoundingClientRect().width ?? 0;
        const row = document.querySelector<HTMLElement>(strip);
        const cell = [...document.querySelectorAll<HTMLElement>(`${strip} > .tab`)].find(
          (one) => one.querySelector(".tab-name")?.textContent === called,
        );
        const chip = cell?.querySelector<HTMLElement>(".tab-tasks");
        return {
          window: window.innerWidth,
          left: widthOf('[data-panel][id="region-left"]'),
          right: widthOf('[data-panel][id="region-right"]'),
          centre: box(document.querySelector('[data-panel][id="region-centre"]')) as Box,
          bar: box(document.querySelector('[data-panel][id="region-centre"] header.bar')) as Box,
          cell: box(cell),
          name: box(cell?.querySelector(".tab-name")),
          closer: box(cell?.querySelector(".closer")),
          chip: chip && chip.getBoundingClientRect().width > 0 ? box(chip) : null,
          drawn: [...document.querySelectorAll(`${strip} [role="tab"]`)].map(
            (tab) => tab.querySelector(".tab-name")?.textContent ?? "",
          ),
          more:
            document
              .querySelector('header.bar button[aria-label^="Show "]')
              ?.getAttribute("aria-label") ?? null,
          scrolls: row ? row.scrollWidth - row.clientWidth : -1,
        };
      },
      STRIP,
      name,
    );
  }
});
