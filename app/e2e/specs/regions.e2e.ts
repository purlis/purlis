// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect, $ } from "@wdio/globals";
import { untilTheStripLists } from "../reading.js";
import { goToWorkspace } from "../titleBarWorkspaces.js";
import { showView } from "../opening.js";
import { bringSideBack, putSideAway } from "../sides.js";
import { tooltipOf } from "../tooltips.js";
import { gutter } from "../gutter.js";
import { theWindowHoldsTheRail, WINDOW_1280, windowAt } from "../rail.js";

/**
 * The right region and the Changes view (ADR 0038; the bottom region until #1676, the left
 * sidebar's until #1739, the Panel's left half since),
 * against the real app started in a copy of the `daily` fixture plane — with real clones in it, one real piece cut off `svc`,
 * and the forge cache a refresher would have left.
 *
 * Nothing is stubbed. The app runs git against those clones and reads that cache, and what
 * this asserts on is what a person would read off the window.
 *
 * This file replaces `panels.e2e.ts`: the repos and the CI it asserted on are still asserted
 * on, in the region they moved to.
 */

/** Waits for a region to say something, and says what it said when it never does. */
async function untilSays(testid: string, want: string | RegExp): Promise<void> {
  const region = await $(`[data-testid="${testid}"]`);
  let last = "";
  try {
    await browser.waitUntil(
      async () => {
        last = await region.getText();
        return typeof want === "string" ? last.includes(want) : want.test(last);
      },
      { timeout: 30_000, interval: 250 },
    );
  } catch {
    throw new Error(`${testid} never said ${want.toString()}; it said ${JSON.stringify(last)}`);
  }
}

/** Focuses a workspace from the title bar's ⌄ (#2916), by its row's own name. */
async function focus(workspace: string): Promise<void> {
  await goToWorkspace(workspace);
}

/**
 * Puts the window on `alpha`, which is the workspace every assertion below is about.
 *
 * **Focused rather than assumed.** `alpha` is what a launch opens on, but one app process
 * serves the whole scenario run and any spec before this one may have left the window
 * somewhere else — which is exactly what happened: five of these went red for 30 s each
 * against a `beta` that holds no repos at all, on both platforms.
 */
async function onAlpha(): Promise<void> {
  await untilTheStripIsRead();
  await focus("alpha");
}

/** Waits until the plane has been read, so a workspace can be focused. */
async function untilTheStripIsRead(): Promise<void> {
  await untilTheStripLists(["alpha", "beta"]);
}

/**
 * **The Changes view** (#1676): what the bottom region drew, the same content, as a view of the
 * left side, in the Panel's left half since #1739. WebDriver reads only what is displayed, so the
 * view is shown first, and the half is left on Repos after, as it was found.
 */
describe("the Changes view", () => {
  before(async () => {
    await untilTheStripIsRead();
    await showView("Changes");
  });
  after(async () => {
    await showView("Repos");
  });

  it("is drawn in the Panel's left half, not the sidebar (#1739)", async () => {
    await expect(
      await $('[data-panel][id="region-panel"] [data-testid="changes-view"]'),
    ).toBeDisplayed();
  });

  it("lists the focused workspace's repos with the branch each is on", async () => {
    await onAlpha();

    await untilSays("repo-svc", "main");
    await expect(await $('[data-testid="repo-tool"]')).toHaveText(expect.stringContaining("main"));
  });

  it("says which repo has something uncommitted and which has not", async () => {
    await onAlpha();

    // The fixture leaves one untracked file in `tool` and nothing in `svc`.
    await untilSays("repo-tool", "untracked");
    await untilSays("repo-svc", "clean");
  });

  it("counts the worktrees cut off each clone", async () => {
    await onAlpha();

    await untilSays("worktrees-svc", "1 branch");
    // Cut with plain git, so no charter layer — counted here, named on the explorer's row.
    await untilSays("worktrees-svc", "1 unwired");
    await untilSays("worktrees-tool", "no branches");
  });

  it("shows the CI state the forge cache holds, and says how old it is", async () => {
    await onAlpha();

    await untilSays("ci-svc", "failed");
    await untilSays("ci-svc", "#41");
    await untilSays("ci-svc", /\d+[smh] ago/);
  });

  it("says a repo nobody has fetched is not fetched, rather than leaving it blank", async () => {
    await onAlpha();

    // `tool` has no entry in the cache at all. A blank cell would read as green.
    await untilSays("ci-tool", "not fetched");
  });

  it("says the app reads CI state and never fetches it", async () => {
    await untilTheStripIsRead();

    await untilSays("changes-view", "never fetches");
  });

  it("follows the focus to another workspace", async () => {
    await onAlpha();

    await focus("beta");

    // `beta` holds no repos.
    await untilSays("changes-view", "No repos in this workspace");

    await focus("alpha");
    await untilSays("repo-svc", "main");
  });

  /**
   * **The rows stay rows when the view is narrow, and jsdom cannot say so.**
   *
   * The operator asked for it of every tree in the window: a row names one thing and never
   * folds, and the view scrolls sideways instead. Here that is every row of the Changes tree
   * (#1701) — a repo's heading, its changes, its branches and each branch — held by `App.css`'s
   * `.tree-row` (#1718). A used height and a `scrollWidth` are the only evidence for either, and
   * jsdom gives every box a size of zero — so this is the only place it can be asked. It also
   * catches a class that emitted no CSS at all.
   *
   * The rows that hold a SENTENCE — a tree purlis could not read, a fetch that did not happen —
   * still wrap, so they are left out here, and the next test is the one that holds them to that.
   */
  it("keeps every row that names something on one line, and scrolls sideways", async () => {
    await onAlpha();
    await untilSays("repo-svc", "main");

    const measured = await browser.execute(() => {
      const bar = document.querySelector<HTMLElement>('[data-testid="changes-view"]');
      if (!bar) throw new Error("no Changes view to narrow");
      // **The view's own box is narrowed**, to a width narrower than its content whatever the
      // side's width is: the question is about this scroll container, not about the slot.
      const was = bar.getAttribute("style") ?? "";
      // A tree is narrower than the table was, so the box is narrower than it was asked at.
      bar.style.width = "120px";

      /** One line of this element's own font, plus half a line and four pixels of slack — a
       *  row that wrapped is a WHOLE line taller than that, and a row that did not can still
       *  run a few pixels over its own line box because a chip in it carries a border. */
      const limitOf = (el: Element) => {
        const css = getComputedStyle(el);
        const line = Number.parseFloat(css.lineHeight);
        const one = Number.isFinite(line) ? line : Number.parseFloat(css.fontSize) * 1.5;
        return one * 1.5 + 4 + Number.parseFloat(css.paddingTop) * 2;
      };
      const naming = [...bar.querySelectorAll('[role="treeitem"]')].filter(
        (row) => row.querySelector(".none, .unreadable") === null,
      );
      const rows = naming.map((row) => ({
        what: `a row (${(row.textContent ?? "").slice(0, 40)})`,
        height: row.getBoundingClientRect().height,
        limit: limitOf(row),
      }));

      const answer = {
        scrollWidth: bar.scrollWidth,
        clientWidth: bar.clientWidth,
        over: 0,
        rows,
        // As the engine resolved them: a class that emitted no CSS at all — the trap
        // `docs/design-system.md` names — reads `normal` here.
        naming: naming.map((el) => getComputedStyle(el).whiteSpace),
      };
      bar.scrollLeft = 10_000;
      answer.over = bar.scrollLeft;
      bar.scrollLeft = 0;

      bar.setAttribute("style", was);
      return answer;
    });

    expect(measured.rows.length).toBeGreaterThan(3);
    const wrapped = measured.rows
      .filter((row) => row.height > row.limit)
      .map((row) => `${row.what}: ${row.height}px, and one line of it is ${row.limit}px`);
    expect(wrapped).toEqual([]);
    expect([...new Set(measured.naming)]).toEqual(["nowrap"]);
    expect(measured.scrollWidth).toBeGreaterThan(measured.clientWidth);
    expect(measured.over).toBeGreaterThan(0);
  });

  it("lets the row that holds a sentence wrap, because a refusal is not a name", async () => {
    // A pipeline row with nothing to name says why, in charter's own words and at charter's
    // own length — `not fetched — <reason>`, or `no pipeline recorded` once a refresher this
    // very run has written an entry that names none. Both are `.none`, and which of the two
    // is on screen depends on whether a refresh has landed yet, so this waits for the cell
    // rather than for either sentence. Held on one line, either would push the region's
    // horizontal scroll out past every row it has.
    await onAlpha();
    const cell = await $('[data-testid="ci-tool"] .none');
    await cell.waitForExist({ timeout: 30_000 });

    const wraps = await browser.execute(() => {
      const said = document.querySelector('[data-testid="ci-tool"] .none');
      return said === null ? null : getComputedStyle(said).whiteSpace;
    });

    expect(wraps).toBe("normal");
  });

  it("has nothing in it to press, because Changes is what is true and not what you do", async () => {
    // ADR 0038's reading — *"the bottom is where you read what is true and do not
    // touch it"*, the bottom being this view since #1676 — as far as a test can hold it.
    await onAlpha();
    await untilSays("repo-svc", "main");

    const bar = await $('[data-testid="changes-view"]');
    const pressable = await bar.$$("button, input, textarea, select, a[href]").getElements();

    expect(pressable.length).toBe(0);
  });
});

describe("the right-hand region", () => {
  it("shows the focused workspace's open todos and the plane's personas", async () => {
    await onAlpha();

    // Todos is in the Panel's right half (#1738), Personas a view of the right sidebar.
    await showView("Todos");
    await untilSays("panel-todos", "Review the rollout plan");
    await showView("Personas");
    await untilSays("panel-personas", "steward");
    // A persona's row is its badge and its name (#1674); the default says so on hover.
    // The row is the list family's (#2911): its tooltip is the row's own.
    const fallback = await $('[data-testid="panel-personas"] .panel-row.is-default');
    await fallback.waitForExist({ timeout: 30_000 });
    expect(await tooltipOf('[data-testid="panel-personas"] .panel-row.is-default')).toContain(
      "default",
    );
    // As the window was found: the sidebar on the Inbox, the right half on Memory.
    await showView("Inbox");
    await showView("Memory");
  });

  /**
   * **A panel is a contribution now, and this is the half only a scenario can carry**
   * (charter-app#191).
   *
   * Todos and personas stopped being hardcoded React fed by named fields: they are
   * `purlis_core::panel` values, produced in `app/src-tauri/src/panels.rs` and drawn by the
   * loop in `Panels.tsx` that draws a stranger's declared panel. The jsdom tests assert the
   * renderer against contract values and the Rust tests assert the producer against a plane on
   * disk; **neither can say that the two meet through a real command, across specta's generated
   * bindings, in a Vite build.** That is this.
   *
   * `data-panel-from` is the attribute the claim rides on, and it is deliberately one
   * ADR 0041 item 5 already wanted on screen — *show what is in force, after approval and not
   * only at it*. If the production build dropped it, an operator would have no way to tell a
   * panel his own charter draws from one an extension contributed, and this goes red.
   */
  it("draws its own panels as contributions, with the contributor named on each", async () => {
    await onAlpha();
    await showView("Todos");
    await untilSays("panel-todos", "Review the rollout plan");
    await showView("Memory");

    const whose = await browser.execute(() =>
      [...document.querySelectorAll("[data-panel-from]")].map((panel) => [
        panel.getAttribute("data-testid"),
        panel.getAttribute("data-panel-from"),
      ]),
    );

    // The fixture plane has no extension installed, so every panel on screen is charter's —
    // and every one of them still went through the seam, which is what `data-panel-from`
    // existing at all proves.
    // The right sidebar's first, then the Panel's right half (#1738).
    expect(whose).toEqual([
      ["panel-personas", "charter"],
      ["panel-memory", "charter"],
      ["panel-todos", "charter"],
      ["panel-sessions", "charter"],
    ]);
  });

  // What a persona row opens — the persona's own tab — is `view-tabs.e2e.ts`.

  it("holds no needs-you queue, which is the title bar's now (charter-app#249)", async () => {
    await untilTheStripIsRead();

    const panels = await $('.region-views[data-region="aside"]');
    await panels.waitForExist({ timeout: 20_000 });
    expect(await panels.$('[aria-label="Needs you"]').isExisting()).toBe(false);
    expect(await panels.$(".needs-you").isExisting()).toBe(false);
  });

  it("holds no alerts section, because alerts are the window's drawer now", async () => {
    // An alert is about a plane and this region is one project's, so alerts moved to the
    // drawer the status line opens (`status-line.e2e.ts` drives it).
    await untilTheStripIsRead();
    await $('.region-views[data-region="aside"]').waitForExist({ timeout: 20_000 });

    expect(await $('[data-testid="panel-alerts"]').isExisting()).toBe(false);
  });

  it("no longer holds the repos or the CI, which are the Changes view's", async () => {
    await onAlpha();
    await $('[data-testid="repo-svc"]').waitForExist({ timeout: 30_000 });

    const panels = await $('.region-views[data-region="aside"]');
    await expect(panels).not.toHaveText(expect.stringContaining("not fetched"));
    expect(await (await $('[data-testid="panel-repos"]')).isExisting()).toBe(false);
  });
});

/**
 * A view of the left side that is drawn: put away, the side keeps its views mounted and hidden
 * (#1673), so it is the shown one that comes and goes, not the explorer's element. The left
 * side's alone: the right side has views of its own since #1678.
 */
const A_VIEW_SHOWN =
  '.region-views[data-region="navigation"]:not([hidden]) > [role="tabpanel"]:not([hidden])';

describe("putting a region away", () => {
  it("takes it off the window and brings it back, and never takes the panes", async () => {
    await untilTheStripIsRead();
    await $('[data-testid="explorer"]').waitForExist({ timeout: 20_000 });

    const view = await putSideAway("Navigation");
    await browser.waitUntil(async () => !(await $(A_VIEW_SHOWN).isExisting()), {
      timeout: 20_000,
      timeoutMsg: "the left side's views did not go away when it was put away",
    });
    // The centre cannot be put away: the terminal panes are the product.
    await expect(await $('[data-strip="Tabs"]')).toBeExisting();

    await bringSideBack("Navigation", view);
    await $(A_VIEW_SHOWN).waitForExist({ timeout: 20_000 });
  });

  it("leaves the slot in the group, collapsed rather than removed", async () => {
    // charter-app#141: taking a panel out of a live group throws *"Panel constraints not found
    // for index 3"* from a document listener, so a slot that is put away stays and is collapsed
    // to nothing. jsdom never lays a group out, so this is the only place the real library is
    // asked — and a width of zero with the content gone is what "put away" has to mean.
    await untilTheStripIsRead();
    await $('[data-testid="explorer"]').waitForExist({ timeout: 20_000 });

    const view = await putSideAway("Navigation");
    await browser.waitUntil(async () => !(await $(A_VIEW_SHOWN).isExisting()), {
      timeout: 20_000,
      timeoutMsg: "the left side's views did not go away when it was put away",
    });

    const slot = await $('[data-panel][id="region-left"]');
    await expect(slot).toBeExisting();
    expect((await slot.getSize("width")) as number).toBe(0);

    await bringSideBack("Navigation", view);
    await $(A_VIEW_SHOWN).waitForExist({ timeout: 20_000 });
  });
});

/**
 * **The left side's activity bar** (#1673, ADR 0038 as amended 2026-10-10), in the real window:
 * the icons switch the side between its views, a press of the open one puts the side away and
 * leaves the bar, and nothing the side draws ever lies over the terminals. What a press decides
 * and what the keys do are `FourRegions.test.tsx`'s; only a window that lays out can say where
 * things are. It leaves the side on Chats and out, as it found it.
 */
describe("the left side's activity bar", () => {
  const BAR = '[role="tablist"][aria-label="Navigation"]';
  const tab = (name: string) => $(`${BAR} [role="tab"][aria-label="${name}"]`);
  const box = (selector: string) =>
    browser.execute((where: string) => {
      const found = document.querySelector(where)?.getBoundingClientRect();
      return found ? { left: found.left, right: found.right, width: found.width } : null;
    }, selector);

  it("switches the side between its views, one view at a time", async () => {
    await untilTheStripIsRead();
    await (await tab("Chats")).waitForExist({ timeout: 20_000 });
    await expect(await $('[data-testid="chats-section"]')).toBeDisplayed();
    await expect(await $('nav[aria-label="Explorer"]')).not.toBeDisplayed();

    await (await tab("Explorer")).click();
    await $('nav[aria-label="Explorer"]').waitForDisplayed({ timeout: 20_000 });
    await expect(await $('[data-testid="chats-section"]')).not.toBeDisplayed();
    await expect(await tab("Explorer")).toHaveAttribute("aria-selected", "true");

    await (await tab("Chats")).click();
    await $('[data-testid="chats-section"]').waitForDisplayed({ timeout: 20_000 });
  });

  it("leaves the side out on a press of the open view, and its key puts it away and back", async () => {
    await untilTheStripIsRead();
    await $('[data-testid="chats-section"]').waitForDisplayed({ timeout: 20_000 });
    const slot = await $('[data-panel][id="region-left"]');

    // R121 (#2917): the side panel's own tabs stay, so a press of the open one is nothing.
    await (await tab("Chats")).click();
    await browser.pause(400);
    expect((await slot.getSize("width")) as number).toBeGreaterThan(0);

    const view = await putSideAway("Navigation");
    await browser.waitUntil(async () => ((await slot.getSize("width")) as number) === 0, {
      timeout: 20_000,
      timeoutMsg: "the left slot did not go to nothing on its key",
    });
    // Hidden, never unmounted: the list is still in the document.
    await expect(await $('[data-testid="chats-section"]')).toBeExisting();

    await bringSideBack("Navigation", view);
    await $('[data-testid="chats-section"]').waitForDisplayed({ timeout: 20_000 });
    expect((await slot.getSize("width")) as number).toBeGreaterThan(0);
  });

  it("is a column beside the project rail, and nothing on the left lies over the terminals", async () => {
    await untilTheStripIsRead();
    await $('[data-testid="chats-section"]').waitForDisplayed({ timeout: 20_000 });

    // The window's edge is the project rail's, where it is shown (#2915); the bar is the next
    // column, 44px, and the side panel after it (#3144).
    const rail = await box('[data-scope="rail"][data-part="slot"]');
    const column = await box(".activity-column[data-side='left']");
    const slot = await box('[data-panel][id="region-left"]');
    const centre = await box('[data-panel][id="region-centre"]');
    // A gutter and a half from the rail (D-3144-13): 12px in Purlis, flush in Classic.
    const fromRail = ((await gutter()) * 3) / 2;
    expect(column).not.toBeNull();
    expect(column?.width).toBe(44);
    if (rail !== null && rail.width > 0) {
      expect(Math.abs((column?.left ?? Infinity) - rail.right - fromRail)).toBeLessThanOrEqual(1);
    } else {
      // The rail hidden (a narrow window, or auto-hide): the column is the window's first, a
      // gutter and a half from its edge, as from the rail (#3144, D-3144-13).
      expect(Math.abs((column?.left ?? Infinity) - fromRail)).toBeLessThanOrEqual(1);
    }
    expect(slot?.left ?? -1).toBeGreaterThanOrEqual((column?.right ?? 0) - 1);
    expect(slot?.right ?? Infinity).toBeLessThanOrEqual((centre?.left ?? 0) + 1);
  });

  it("keeps the column where it is while the side goes away and comes back", async () => {
    await untilTheStripIsRead();
    await $('[data-testid="chats-section"]').waitForDisplayed({ timeout: 20_000 });
    const before = await box(".activity-column[data-side='left']");

    const view = await putSideAway("Navigation");
    const slot = await $('[data-panel][id="region-left"]');
    await browser.waitUntil(async () => ((await slot.getSize("width")) as number) === 0, {
      timeout: 20_000,
      timeoutMsg: "the left slot did not go to nothing on its key",
    });
    expect(await box(".activity-column[data-side='left']")).toEqual(before);

    await bringSideBack("Navigation", view);
    await $('[data-testid="chats-section"]').waitForDisplayed({ timeout: 20_000 });
    expect(await box(".activity-column[data-side='left']")).toEqual(before);
  });
});

describe("the right side's activity bar (#1678)", () => {
  const BAR = '[role="tablist"][aria-label="Attention"]';
  const tab = (name: string) => $(`${BAR} [role="tab"][aria-label="${name}"]`);
  const box = (selector: string) =>
    browser.execute((where: string) => {
      const found = document.querySelector(where)?.getBoundingClientRect();
      return found ? { left: found.left, right: found.right, width: found.width } : null;
    }, selector);

  /** The right sidebar's Inbox, drawn: the view it opens on since #1738. */
  const INBOX = '[data-panel][id="region-right"] .region-view[data-view="inbox"]';

  // On the Inbox, the view the side opens on (#1738; `regions.test.ts` holds that default): one
  // app process serves the whole run, and a spec before this one may have left another view open.
  before(async () => {
    await onAlpha();
    await showView("Inbox");
  });

  it("is on the Inbox and switches to Personas, one view at a time (#1738)", async () => {
    await onAlpha();
    await (await tab("Inbox")).waitForExist({ timeout: 20_000 });
    await expect(await tab("Inbox")).toHaveAttribute("aria-selected", "true");

    await (await tab("Personas")).click();
    await $('[data-testid="panel-personas"]').waitForDisplayed({ timeout: 20_000 });
    await expect(await $(INBOX)).not.toBeDisplayed();

    await (await tab("Inbox")).click();
    await $(INBOX).waitForDisplayed({ timeout: 20_000 });
  });

  it("puts the side away on a press of the open view, keeps the bar, and brings it back", async () => {
    await onAlpha();
    await $(INBOX).waitForDisplayed({ timeout: 20_000 });

    await (await tab("Inbox")).click();
    const slot = await $('[data-panel][id="region-right"]');
    await browser.waitUntil(async () => ((await slot.getSize("width")) as number) === 0, {
      timeout: 20_000,
      timeoutMsg: "the right slot did not go to nothing when its open view was pressed",
    });
    await expect(await $(BAR)).toBeDisplayed();

    await (await tab("Inbox")).click();
    await $(INBOX).waitForDisplayed({ timeout: 20_000 });
    expect((await slot.getSize("width")) as number).toBeGreaterThan(0);
  });

  it("stands at the window's right edge, and nothing on the right lies over the terminals", async () => {
    await onAlpha();
    await $(INBOX).waitForDisplayed({ timeout: 20_000 });

    const bar = await box(".activity-column[data-side='right']");
    const slot = await box('[data-panel][id="region-right"]');
    const centre = await box('[data-panel][id="region-centre"]');
    const width = await browser.execute(() => document.documentElement.clientWidth);
    expect(bar).not.toBeNull();
    expect(bar?.right ?? -1).toBeGreaterThanOrEqual(width - 1);
    expect(slot?.right ?? Infinity).toBeLessThanOrEqual((bar?.left ?? 0) + 1);
    expect(centre?.right ?? Infinity).toBeLessThanOrEqual((slot?.left ?? 0) + 1);
  });
});

/**
 * **No view lies over the terminals** (#1676): every view of the left side, the right-hand
 * region and both activity bars, each laid out in the real window and measured against the
 * centre, whose panes are the product. The Panel is under the centre (#1737), and the centre
 * runs down to it.
 */
describe("the views and the terminals", () => {
  type Box = { left: number; right: number; top: number; bottom: number };
  const boxOf = (selector: string) =>
    browser.execute((where: string) => {
      const found = document.querySelector(where);
      if (found === null) return null;
      const box = found.getBoundingClientRect();
      return { left: box.left, right: box.right, top: box.top, bottom: box.bottom };
    }, selector);
  /** Whether two boxes share any area; touching edges, a pixel either way, is not overlap. */
  const overlaps = (a: Box, b: Box) =>
    Math.min(a.right, b.right) - Math.max(a.left, b.left) > 1 &&
    Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top) > 1;

  after(async () => {
    await showView("Chats");
  });

  // The left sidebar's views; Changes is the Panel's since #1739, and the Panel is measured
  // against the centre below.
  for (const view of ["Chats", "Explorer", "Search"] as const) {
    it(`draws the ${view} view beside the terminals, never over them`, async () => {
      await untilTheStripIsRead();
      await showView(view);
      const panel = `.region-views > [role="tabpanel"][aria-labelledby]:not([hidden])`;
      await $(panel).waitForDisplayed({ timeout: 20_000 });

      const centre = await boxOf('[data-panel][id="region-centre"]');
      if (centre === null) throw new Error("no centre to measure against");
      const regions: [string, Box | null][] = [
        [`the ${view} view`, await boxOf(panel)],
        ["the left slot", await boxOf('[data-panel][id="region-left"]')],
        ["the right-hand region", await boxOf('[data-panel][id="region-right"]')],
        ["the left activity bar", await boxOf(".activity-bar[data-side='left']")],
      ];
      expect(regions[0][1]).not.toBeNull();
      const over = regions
        .filter((entry): entry is [string, Box] => entry[1] !== null)
        .filter(([, box]) => overlaps(box, centre))
        .map(([name, box]) => `${name} ${JSON.stringify(box)} over ${JSON.stringify(centre)}`);
      expect(over).toEqual([]);
    });
  }

  it("runs the terminals down to the Panel, and the Panel down to the status line", async () => {
    await untilTheStripIsRead();
    const centre = await boxOf('[data-panel][id="region-centre"]');
    const panel = await boxOf('[data-panel][id="region-panel"]');
    const line = await boxOf('[data-testid="status-line"]');
    if (centre === null || panel === null || line === null)
      throw new Error("no centre, Panel or status line to measure");

    // The old bottom region is gone (#1676); the Panel is what is under the centre (#1737).
    await expect(await $('[data-panel][id="region-bottom"]')).not.toBeExisting();
    expect(overlaps(centre, panel)).toBe(false);
    // Flush, or the theme's gutter apart (#3034).
    const apart = (await gutter()) + 2;
    expect(Math.abs(panel.top - centre.bottom)).toBeLessThanOrEqual(apart);
    expect(Math.abs(line.top - panel.bottom)).toBeLessThanOrEqual(apart);
  });
});

/**
 * **The Panel** (spec #1736, P-1, P-12; #1737), at real sizes, which only a window that lays out
 * can give: under both sidebars and the editor, the window's whole width between the activity
 * bars, 34% of the regions' height by default, never past 70%, and back to 34% on a double-click
 * of its handle. What a press decides is `regions.test.ts`'s and `FourRegions.test.tsx`'s.
 */
describe("the Panel", () => {
  type Box = { left: number; right: number; top: number; bottom: number; height: number };
  const boxOf = (selector: string) =>
    browser.execute((where: string) => {
      const found = document.querySelector(where);
      if (found === null) return null;
      const box = found.getBoundingClientRect();
      return {
        left: box.left,
        right: box.right,
        top: box.top,
        bottom: box.bottom,
        height: box.height,
      };
    }, selector) as Promise<Box | null>;
  const PANEL = '[data-panel][id="region-panel"]';
  /** The column the regions are drawn in: the sidebars and the editor above, the Panel below. */
  const COLUMN = ".region-frame > .regions";
  /** The Panel's height as a share of the column it is in. */
  const share = async () => {
    const panel = await boxOf(PANEL);
    const column = await boxOf(COLUMN);
    if (panel === null || column === null) throw new Error("no Panel or column to measure");
    return panel.height / column.height;
  };
  /** The handle between the editor and the Panel: the last of the column's own. */
  const HANDLE = `${COLUMN} > [role="separator"]`;

  before(async () => {
    await untilTheStripIsRead();
    await $('[data-testid="repos-view"]').waitForDisplayed({ timeout: 20_000 });
  });

  it("holds the Repos view, open, its icon at the foot of the left column", async () => {
    await expect(await $(`${PANEL} [data-testid="repos-view"]`)).toBeDisplayed();
    const lower = await boxOf('[role="tablist"][aria-label="Panel, left half"]');
    const upper = await boxOf('[role="tablist"][aria-label="Navigation"]');
    if (lower === null || upper === null) throw new Error("no Lower or upper icons to measure");
    // One column (#3144): the bottom group under the top group's, in its line.
    expect(Math.abs(lower.left - upper.left)).toBeLessThanOrEqual(1);
    expect(lower.top).toBeGreaterThanOrEqual(upper.bottom - 1);
  });

  it("runs under both sidebars and the editor, the whole width between the bars", async () => {
    const panel = await boxOf(PANEL);
    const left = await boxOf('[data-panel][id="region-left"]');
    const right = await boxOf('[data-panel][id="region-right"]');
    const rightBar = await boxOf(".activity-column[data-side='right']");
    if ([panel, left, right, rightBar].includes(null)) throw new Error("nothing to measure");
    // From the side panel's edge (R126), the left column before it (#3144); flush with the
    // right bar, or the theme's gutter from it (#3034).
    const apart = (await gutter()) + 2;
    expect(Math.abs((panel as Box).left - (left as Box).left)).toBeLessThanOrEqual(2);
    expect(Math.abs((panel as Box).right - (rightBar as Box).left)).toBeLessThanOrEqual(apart);
    expect((panel as Box).top).toBeGreaterThanOrEqual((left as Box).bottom - 1);
    expect((panel as Box).top).toBeGreaterThanOrEqual((right as Box).bottom - 1);
  });

  /** The bottom row's default share (R126, D-2917-17): 34% from a 900px-tall window up, 30%
   *  below. */
  const defaultShare = async () =>
    (await browser.execute(() => window.innerHeight)) < 900 ? 0.3 : 0.34;

  it("is 34% of the regions' height by default (30% in a short window), and never under 160px (R126)", async () => {
    const wanted = await defaultShare();
    expect(await share()).toBeGreaterThan(wanted - 0.04);
    expect(await share()).toBeLessThan(wanted + 0.04);
    expect(((await boxOf(PANEL)) as Box).height).toBeGreaterThanOrEqual(159);
  });

  it("grows no taller than 70% however far it is dragged, and a double-click puts it back", async () => {
    // With the keyboard, as the widths' spec drags a side: the arrows are a resize the library
    // reports exactly as it reports a drag. Whatever the library's step, the handle is pressed
    // until the Panel reaches its ceiling (or 60 presses), and a range is asserted, not a pixel:
    // well past where it started, and never past 70%, give or take a pixel's rounding.
    await focusOn(HANDLE);
    for (let press = 0; press < 60 && (await share()) < 0.69; press++) {
      await browser.keys(["ArrowUp"]);
    }
    await browser.waitUntil(async () => (await share()) > 0.6, {
      timeout: 20_000,
      timeoutMsg: "the Panel never got taller",
    });
    expect(await share()).toBeLessThanOrEqual(0.71);

    await doubleClickOn(HANDLE);

    const wanted = await defaultShare();
    await browser.waitUntil(async () => Math.abs((await share()) - wanted) < 0.04, {
      timeout: 20_000,
      timeoutMsg: "a double-click did not put the Panel back at its default height",
    });
  });

  /**
   * **The halves** (#1738, P-5, P-11, P-12): Repos left and Memory right, sharing the width half
   * each, a handle between them that keeps either a fifth at least and goes back to half each on
   * a double-click, and each half headed by one line.
   */
  const HALF = (side: string) => `${PANEL} .panel-half[data-side="${side}"]`;
  const halfShare = async () => {
    const left = await boxOf(HALF("left"));
    const panel = await boxOf(PANEL);
    if (left === null || panel === null) throw new Error("no half or Panel to measure");
    return (left.right - left.left) / (panel.right - panel.left);
  };
  /** The handle between the halves. */
  const BETWEEN = `${PANEL} [role="separator"]`;
  /** A half's chevron, by the name a person and a screen reader are given. */
  const AWAY = (side: string) => `[aria-label="Put the panel's ${side} half away"]`;

  it("holds Memory in its right half, its icon at the right bar's foot", async () => {
    // Memory is a workspace's: focused, so its panel and its + are drawn.
    await onAlpha();
    await $(`${HALF("right")} [data-testid="panel-memory"]`).waitForDisplayed({ timeout: 20_000 });
    const lower = await boxOf('[role="tablist"][aria-label="Panel, right half"]');
    const upper = await boxOf('[role="tablist"][aria-label="Attention"]');
    if (lower === null || upper === null) throw new Error("no Lower or upper icons to measure");
    // The gap between the groups (R117): the Lower icons below the top group, at the foot.
    expect(lower.top).toBeGreaterThanOrEqual(upper.bottom + 11);
  });

  it("heads each half with one line: its title, its actions and its chevron side by side", async () => {
    for (const side of ["left", "right"]) {
      const head = await boxOf(`${HALF(side)} .half-head`);
      const title = await boxOf(`${HALF(side)} .half-head [data-part="title"]`);
      const chevron = await boxOf(`${HALF(side)} .half-head ${AWAY(side)}`);
      if (head === null || title === null || chevron === null)
        throw new Error(`no line to measure on the ${side} half`);
      // One line: no taller than two lines of the sidebar's title, and the title and the
      // chevron on it, the chevron at its end.
      expect(head.height).toBeLessThan(48);
      expect(chevron.top).toBeLessThan(title.bottom);
      expect(title.top).toBeLessThan(chevron.bottom);
      expect(chevron.left).toBeGreaterThanOrEqual(title.right - 1);
    }
    // Memory's own + is on its line, not under it.
    await $(`${HALF("right")} .half-head [aria-label="New memory in alpha…"]`).waitForExist({
      timeout: 20_000,
    });
    const plus = await boxOf(`${HALF("right")} .half-head [aria-label="New memory in alpha…"]`);
    const line = await boxOf(`${HALF("right")} .half-head`);
    if (plus === null || line === null) throw new Error("no + on Memory's line");
    expect(plus.top).toBeGreaterThanOrEqual(line.top - 1);
    expect(plus.bottom).toBeLessThanOrEqual(line.bottom + 1);
  });

  it("draws each half's title as the section label and its words as a body (#1920)", async () => {
    // The text roles, as the window draws them: a label is 11/14 of the window's text and a
    // body 13/14, whatever that text is set to. A range, never a pixel.
    const drawn = (await browser.execute(() => {
      const px = (one: Element | null) =>
        one === null ? Number.NaN : Number.parseFloat(getComputedStyle(one).fontSize);
      return {
        text: px(document.documentElement),
        titles: [...document.querySelectorAll('.panel-half .half-head [data-part="title"]')].map(
          px,
        ),
        memory: px(document.querySelector('.panel-half .panels[data-view="memory"]')),
        repos: px(document.querySelector(".panel-half .explorer")),
      };
    })) as { text: number; titles: number[]; memory: number; repos: number };
    expect(drawn.titles).toHaveLength(2);
    for (const title of drawn.titles)
      expect(Math.abs(title - (drawn.text * 11) / 14)).toBeLessThanOrEqual(0.5);
    expect(Math.abs(drawn.memory - (drawn.text * 13) / 14)).toBeLessThanOrEqual(0.5);
    expect(Math.abs(drawn.repos - (drawn.text * 13) / 14)).toBeLessThanOrEqual(0.5);
  });

  it("draws one hairline between its halves, each half's content the same step clear of it (qw337)", async () => {
    const line = await boxOf(BETWEEN);
    const panel = await boxOf(PANEL);
    const left = await boxOf(HALF("left"));
    const right = await boxOf(HALF("right"));
    if (line === null || panel === null || left === null || right === null)
      throw new Error("no handle, Panel or halves to measure");
    // A hairline, a device pixel of rounding either way, and it is drawn in a colour. Where the
    // theme keeps a gutter, the handle is the gutter wide and the line its middle (`App.css`,
    // `.panel-halves > [data-separator]`, #3034).
    expect(line.right - line.left).toBeGreaterThanOrEqual(0.5);
    expect(line.right - line.left).toBeLessThanOrEqual(Math.max(1.5, (await gutter()) + 0.5));
    const drawnIn = await browser.execute(
      (where: string) => getComputedStyle(document.querySelector(where) as Element).backgroundColor,
      BETWEEN,
    );
    expect(drawnIn).not.toBe("rgba(0, 0, 0, 0)");
    expect(drawnIn).not.toBe("transparent");
    // Between the halves, and the Panel's whole height: the halves' headers included.
    expect(line.left).toBeGreaterThanOrEqual(left.right - 1);
    expect(line.right).toBeLessThanOrEqual(right.left + 1);
    expect(line.top).toBeLessThanOrEqual(left.top + 1);
    expect(line.bottom).toBeGreaterThanOrEqual(left.bottom - 1);
    expect(line.height).toBeGreaterThanOrEqual(panel.height - 3);

    // What each half holds starts the same step clear of the line: its heading's line and its
    // views' box, on both sides.
    const insets: number[] = [];
    for (const [side, part] of [
      ["left", ".half-head"],
      ["left", ".region-views"],
      ["right", ".half-head"],
      ["right", ".region-views"],
    ] as const) {
      const held = await boxOf(`${HALF(side)} > ${part}`);
      if (held === null) throw new Error(`no ${part} in the ${side} half`);
      insets.push(side === "left" ? line.left - held.right : held.left - line.right);
    }
    for (const inset of insets) {
      expect(inset).toBeGreaterThanOrEqual(2);
      expect(inset).toBeLessThanOrEqual(8);
      expect(Math.abs(inset - insets[0])).toBeLessThanOrEqual(1);
    }
  });

  it("shares its width half each, the handle keeping either a fifth, a double-click half each", async () => {
    expect(Math.abs((await halfShare()) - 0.5)).toBeLessThan(0.03);

    // Pressed only until it is under 40%: a half is collapsible, and arrows past its fifth would
    // put it away, which is not what a resize is being asked about here.
    await focusOn(BETWEEN);
    for (let press = 0; press < 10 && (await halfShare()) >= 0.4; press++) {
      await browser.keys(["ArrowLeft"]);
    }
    await browser.waitUntil(async () => (await halfShare()) < 0.4, {
      timeout: 20_000,
      timeoutMsg: "the left half never got narrower",
    });
    expect(await halfShare()).toBeGreaterThanOrEqual(0.19);

    await doubleClickOn(BETWEEN);

    await browser.waitUntil(async () => Math.abs((await halfShare()) - 0.5) < 0.03, {
      timeout: 20_000,
      timeoutMsg: "a double-click did not put the halves back at half each",
    });
  });

  it("gives the left half the whole width while the right is away, and back", async () => {
    await (await $(`${HALF("right")} ${AWAY("right")}`)).click();
    await browser.waitUntil(async () => (await halfShare()) > 0.97, {
      timeout: 20_000,
      timeoutMsg: "the left half did not take the Panel's width",
    });
    await expect(await $(HALF("right"))).not.toBeDisplayed();

    await (
      await $('[role="tablist"][aria-label="Panel, right half"] [role="tab"][aria-label="Memory"]')
    ).click();
    await $(`${HALF("right")} [data-testid="panel-memory"]`).waitForDisplayed({ timeout: 20_000 });
    expect(Math.abs((await halfShare()) - 0.5)).toBeLessThan(0.03);
  });

  /**
   * **A narrow window** (P-7, #1741): below about 900 px, counted at the default text size, the
   * Panel draws one half at a time, the one used last, and the other side's Lower icons switch
   * it. Nothing hangs out of the window: the page never scrolls sideways, and the regions'
   * frame, its bars, slots and the half drawn, is inside its width.
   */
  describe("in a narrow window", () => {
    /** The halves drawn, by side. */
    const drawnHalves = () =>
      browser.execute(() =>
        [...document.querySelectorAll<HTMLElement>(".panel-half:not([hidden])")]
          .filter((one) => one.getBoundingClientRect().width > 0)
          .map((one) => one.dataset.side),
      ) as Promise<string[]>;
    /** How far the page scrolls sideways, and what of the frame is outside the window. */
    const overflow = () =>
      browser.execute(() => {
        const page = document.documentElement;
        const frame = [
          ".region-frame",
          ".activity-column",
          "[data-panel]",
          ".panel-half:not([hidden])",
          ".half-head",
        ];
        const out = [...document.querySelectorAll<HTMLElement>(frame.join(","))]
          .filter((one) => {
            const box = one.getBoundingClientRect();
            return box.width > 0 && (box.left < -1 || box.right > window.innerWidth + 1);
          })
          .map((one) => one.id || one.className);
        return { sideways: page.scrollWidth - page.clientWidth, out };
      });
    /** Each half's open view, whose Lower icon draws it. */
    const OPEN = { left: "Repos", right: "Memory" } as const;
    const lowerIcon = (side: "left" | "right") =>
      $(
        `[role="tablist"][aria-label="Panel, ${side} half"] [role="tab"][aria-label="${OPEN[side]}"]`,
      );
    let wide: { width: number; height: number };
    /** The window text size the spec found, in px, to put back. */
    let text = 14;
    /** The window's width as the narrow rule counts it: at the default text size of 14 px. */
    const counted = () =>
      browser.execute(
        () =>
          (window.innerWidth * 14) /
          Number.parseFloat(getComputedStyle(document.documentElement).fontSize),
      ) as Promise<number>;
    /** The window text size, a step bigger or smaller, by its own keys (`textSize.ts`). */
    const textStep = (key: "=" | "-") =>
      browser.execute((pressed: string) => {
        const mac = navigator.platform.startsWith("Mac");
        document.body.dispatchEvent(
          new KeyboardEvent("keydown", {
            key: pressed,
            metaKey: mac,
            ctrlKey: !mac,
            bubbles: true,
            cancelable: true,
          }),
        );
        return Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
      }, key) as Promise<number>;

    before(async () => {
      wide = await browser.getWindowSize();
      text = (await browser.execute(() =>
        Number.parseFloat(getComputedStyle(document.documentElement).fontSize),
      )) as number;
      // The window is never under 1024 px (ADR 0054), so a narrow one is the narrowest window
      // with its text made bigger, which the rule counts as zoom: from 16 px of window text.
      await browser.setWindowSize(1024, wide.height);
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
      for (let step = 0; step < 10 && (await counted()) >= 880; step++) await textStep("=");
      await browser.waitUntil(async () => (await counted()) < 900, {
        timeout: 20_000,
        timeoutMsg: "the narrowest window with bigger text is still not counted under 900 px",
      });
    });

    after(async () => {
      for (let step = 0; step < 10; step++) {
        const now = (await browser.execute(() =>
          Number.parseFloat(getComputedStyle(document.documentElement).fontSize),
        )) as number;
        if (now <= text) break;
        await textStep("-");
      }
      await browser.setWindowSize(wide.width, wide.height);
      await browser.waitUntil(async () => (await drawnHalves()).length === 2, {
        timeout: 20_000,
        timeoutMsg: "a wide window again did not draw both halves",
      });
    });

    it("puts the left column a gutter and a half from the window's edge, the rail hidden (#3144)", async () => {
      // The narrowest window: the rail hides itself (#3077), and the column it leaves is the
      // window's first, as the prototype's variant D draws a hidden rail.
      await browser.waitUntil(
        async () =>
          browser.execute(
            () =>
              (document
                .querySelector('[data-scope="rail"][data-part="slot"]')
                ?.getBoundingClientRect().width ?? 0) < 1,
          ),
        { timeout: 20_000, timeoutMsg: "the rail still takes room in the narrowest window" },
      );
      const left = await browser.execute(
        () =>
          document.querySelector(".activity-column[data-side='left']")?.getBoundingClientRect()
            .left ?? null,
      );
      expect(left).not.toBeNull();
      expect(Math.abs((left as number) - ((await gutter()) * 3) / 2)).toBeLessThanOrEqual(1);
    });

    it("draws one half at a time, the other side's Lower icons switching it, and overflows nothing", async () => {
      await browser.waitUntil(async () => (await drawnHalves()).length === 1, {
        timeout: 20_000,
        timeoutMsg: "a narrow window still draws both halves",
      });
      expect(await overflow()).toEqual({ sideways: 0, out: [] });

      const [shown] = await drawnHalves();
      const other = shown === "left" ? "right" : "left";
      await (await lowerIcon(other)).click();

      await browser.waitUntil(async () => (await drawnHalves()).join() === other, {
        timeout: 20_000,
        timeoutMsg: `the ${other} half's Lower icon did not draw it in place of the ${shown}`,
      });
      // The half it took the place of was never put away: its icon draws it back.
      expect(await overflow()).toEqual({ sideways: 0, out: [] });
      await (await lowerIcon(shown as "left" | "right")).click();
      await browser.waitUntil(async () => (await drawnHalves()).join() === shown, {
        timeout: 20_000,
        timeoutMsg: `the ${shown} half's Lower icon did not draw it back`,
      });
    });
  });
});

/**
 * **The arrangement drives the window** (`app/src/regions.ts`), against the real app.
 *
 * jsdom gives every element a size of zero, so `react-resizable-panels` defers its layout there
 * and no unit test can read a region's width. These are the assertions that need a window that
 * really lays out.
 */
describe("the layout as data", () => {
  // At 1280×800, both sidebars open, where the project rail hides itself to leave the work its
  // room (`rail.ts`, #3077): measured with the rail as the window draws it.
  before(() => windowAt(WINDOW_1280));
  after(() => theWindowHoldsTheRail());

  it("draws each region in the slot the arrangement names", async () => {
    await untilTheStripIsRead();
    await $('[data-testid="explorer"]').waitForExist({ timeout: 20_000 });

    // The default arrangement (ADR 0038), read off the real DOM rather than off the
    // JSX: the navigation region's views in the left slot, and what is asking for you in the
    // right. Nothing along the bottom but the Panel.
    await expect(await $('[data-panel][id="region-left"] [data-testid="explorer"]')).toBeExisting();
    await expect(
      await $('[data-panel][id="region-right"] .region-views[data-region="aside"]'),
    ).toBeExisting();
    await expect(await $('[data-panel][id="region-bottom"]')).not.toBeExisting();
    // And Repos in the Panel's left half (#1737), the repo state beside it (#1739).
    await expect(
      await $('[data-panel][id="region-panel"] [data-testid="repos-view"]'),
    ).toBeExisting();
    await expect(
      await $('[data-panel][id="region-panel"] [data-testid="changes-view"]'),
    ).toBeExisting();
    await expect(
      await $('[data-panel][id="region-left"] [data-testid="changes-view"]'),
    ).not.toBeExisting();
  });

  it("gives a slot the width the arrangement asks for, and not an equal share", async () => {
    // The sizes are the arrangement's — a slot laid out at a third of the window would mean the
    // library never read them. The explorer is 16% of its row by default and the right-hand
    // side 20%, so the centre is much the widest thing in the window.
    await untilTheStripIsRead();
    await $('[data-testid="explorer"]').waitForExist({ timeout: 20_000 });

    const width = async (id: string) =>
      (await (await $(`[data-panel][id="${id}"]`)).getSize("width")) as number;
    const widths = async () =>
      Promise.all([width("region-left"), width("region-centre"), width("region-right")]);
    // Read once the layout has settled: the same widths twice, a frame apart. A window that
    // has just been laid out (by the spec before, or the strip being read) may still be moving.
    let seen = await widths();
    await browser.waitUntil(
      async () => {
        await browser.execute(
          () => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))),
        );
        const now = await widths();
        const still = now.every((one, i) => Math.abs(one - seen[i]) < 0.5);
        seen = now;
        return still;
      },
      { timeout: 10_000, interval: 100, timeoutMsg: "the regions never settled" },
    );
    const [left, centre, right] = seen;

    expect(centre).toBeGreaterThan(left + right);
    expect(left).toBeGreaterThan(0);
    expect(right).toBeGreaterThan(left);
  });

  it("remembers how wide a slot was dragged, and brings it back that wide", async () => {
    // What charter-app#141 deferred: only WHICH regions were drawn was remembered, never how
    // big. A drag settles, the width is written down, and putting the region away and bringing
    // it back comes back to the dragged width rather than to the library's minimum.
    await untilTheStripIsRead();
    await $('[data-testid="explorer"]').waitForExist({ timeout: 20_000 });
    const slot = await $('[data-panel][id="region-left"]');
    const before = (await slot.getSize("width")) as number;

    // The handle between the explorer and the centre, moved with the keyboard: a drag in
    // pixels is a pointer path a CI runner times differently, and the arrow keys are a resize
    // the library reports exactly as it reports a drag.
    // Focused by the page, not clicked: a handle is 0px across in the Linux WebView, and a
    // click at its centre lands on what is beside it (see {@link focusOn}).
    const handle = '[role="separator"][aria-controls="region-left"]';
    await focusOn(handle);
    for (let step = 0; step < 6; step++) await browser.keys(["ArrowRight"]);
    await browser.waitUntil(async () => ((await slot.getSize("width")) as number) > before, {
      timeout: 20_000,
      timeoutMsg: "the explorer's slot never got wider",
    });
    const dragged = (await slot.getSize("width")) as number;

    const view = await putSideAway("Navigation");
    await browser.waitUntil(async () => !(await $(A_VIEW_SHOWN).isExisting()), {
      timeout: 20_000,
      timeoutMsg: "the left side's views did not go away when it was put away",
    });
    await bringSideBack("Navigation", view);
    await $(A_VIEW_SHOWN).waitForExist({ timeout: 20_000 });

    await browser.waitUntil(
      async () => Math.abs(((await slot.getSize("width")) as number) - dragged) <= 2,
      {
        timeout: 20_000,
        timeoutMsg: `the explorer came back at a different width than the ${dragged}px it was dragged to`,
      },
    );

    // One app process serves the whole scenario run, and this spec is the only one that changes
    // a width — so it puts it back, rather than leaving every spec after it looking at a window
    // this one rearranged.
    await focusOn(handle);
    for (let step = 0; step < 6; step++) await browser.keys(["ArrowLeft"]);
  });
});

/**
 * **The keyboard on a handle, given by the page and not by a click.** A handle is drawn as a
 * hairline (and measures 0 across in the Linux WebView), so a driver's click at its centre lands
 * on whatever is beside it — a branch's row in Repos, which a click picks, and the next spec's
 * chat would start there.
 */
async function focusOn(selector: string): Promise<void> {
  await browser.execute((where: string) => {
    document.querySelector<HTMLElement>(where)?.focus();
  }, selector);
}

/**
 * **A double-click on a handle, as the window hears one**: two presses at its centre, from the
 * handle itself, with nothing clicked beside it (see {@link focusOn}). The real window's driver
 * sends a double-click as `mousedown`s and `click`s and no `dblclick`; the presses are what the
 * handle's reset counts (`RegionFrame`'s `useHandleReset`).
 */
async function doubleClickOn(selector: string): Promise<void> {
  await browser.execute((where: string) => {
    const handle = document.querySelector<HTMLElement>(where);
    if (handle === null) return;
    const box = handle.getBoundingClientRect();
    const at = { clientX: box.left + box.width / 2, clientY: box.top + box.height / 2 };
    for (let press = 0; press < 2; press++) {
      for (const type of ["mousedown", "mouseup"])
        handle.dispatchEvent(
          new MouseEvent(type, { ...at, bubbles: true, cancelable: true, button: 0 }),
        );
    }
  }, selector);
}
