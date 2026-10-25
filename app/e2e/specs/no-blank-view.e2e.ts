// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import {
  closeTheViewTabsOpenedSince,
  focusWorkspace,
  runFromThePalette,
  tabNames,
} from "../reach.js";

/**
 * **No view is ever a blank pane** (#614 l1, FR-19): every view of every side, both halves of
 * the Panel, and the view tabs the palette opens on the fixture project draw what they hold or
 * their empty state, in the built app.
 *
 * `emptyStates.guard.test.ts` holds each component to an empty state in jsdom. What only the
 * real window can say is that what is on the screen, once the view has read, is not nothing: a
 * view whose read never answered, whose content collapsed to no size, or whose empty state was
 * never reached would pass a unit test and leave the person looking at an empty rectangle.
 *
 * **Blank** is said of the view's body once nothing in it is still reading (`aria-busy`): no
 * visible words, and no empty state. A view is shown by its icon's own click from the page, and
 * a view tab by its palette row (`reach.ts`).
 *
 * **A view tab and its view are one piece** (#3147, the operator's ruling of 2026-10-24): every
 * view tab it opens, Settings, a vault, a persona and the shared memory among them, carries no
 * title of its own (no heading, and nowhere its tab's name as words of its own), starts with no
 * band of its own fill, draws its pane's actions where a chat's pane draws them, and is in the
 * panel its chosen tab names.
 *
 * **It leaves the window as it found it**: each bar is put back on the view it had open (or put
 * away, where it was), and each view tab it opened is closed. The vault it makes for its vault
 * tab stays in the fixture's copy, as `vaults.e2e.ts`'s does: nothing deletes a vault.
 */

type Body = {
  /** The visible words in the view, its heading's left out. */
  words: string;
  /** Whether an empty state is drawn there, with a headline. */
  empty: boolean;
  /** The body's own size. */
  width: number;
  height: number;
  /** Whether anything in it still says it is reading. */
  busy: boolean;
};

/** Each activity bar's views, by their names, and the one each has open. */
async function bars(): Promise<{ bar: string; views: string[]; open: string | null }[]> {
  return browser.execute(() =>
    [...document.querySelectorAll('.activity-bar[role="tablist"]')].map((list) => {
      const tabs = [...list.querySelectorAll('[role="tab"]')];
      return {
        bar: list.getAttribute("aria-label") ?? "",
        views: tabs.map((tab) => tab.getAttribute("aria-label") ?? ""),
        open:
          tabs
            .find((tab) => tab.getAttribute("aria-selected") === "true")
            ?.getAttribute("aria-label") ?? null,
      };
    }),
  );
}

/** Presses the icon of view `view` on the bar `bar`, by its own click. */
async function pressIcon(bar: string, view: string): Promise<void> {
  const found = await browser.execute(
    (named: string, called: string) => {
      const list = [...document.querySelectorAll('.activity-bar[role="tablist"]')].find(
        (one) => one.getAttribute("aria-label") === named,
      );
      const tab = list?.querySelector<HTMLElement>(`[role="tab"][aria-label="${called}"]`);
      tab?.click();
      return tab !== null && tab !== undefined;
    },
    bar,
    view,
  );
  if (!found) throw new Error(`no ${view} icon on the ${bar} bar`);
}

/** Whether the icon of `view` on `bar` is the open one. */
async function isOpen(bar: string, view: string): Promise<boolean> {
  return browser.execute(
    (named: string, called: string) => {
      const list = [...document.querySelectorAll('.activity-bar[role="tablist"]')].find(
        (one) => one.getAttribute("aria-label") === named,
      );
      return (
        list
          ?.querySelector(`[role="tab"][aria-label="${called}"]`)
          ?.getAttribute("aria-selected") === "true"
      );
    },
    bar,
    view,
  );
}

/** What the panel the icon of `view` on `bar` controls holds, read once. */
async function panelOf(bar: string, view: string): Promise<Body | null> {
  return browser.execute(
    (named: string, called: string) => {
      const list = [...document.querySelectorAll('.activity-bar[role="tablist"]')].find(
        (one) => one.getAttribute("aria-label") === named,
      );
      const tab = list?.querySelector(`[role="tab"][aria-label="${called}"]`);
      const id = tab?.getAttribute("aria-controls");
      const panel = id ? document.getElementById(id) : null;
      if (!panel) return null;
      const box = panel.getBoundingClientRect();
      const empty = [
        ...panel.querySelectorAll('[data-scope="empty-state"][data-part="root"]'),
      ].some(
        (one) =>
          (
            one.querySelector('[data-scope="empty-state"][data-part="headline"]')?.textContent ?? ""
          ).trim() !== "" && one.getBoundingClientRect().height > 0,
      );
      return {
        words: (panel as HTMLElement).innerText.trim(),
        empty,
        width: box.width,
        height: box.height,
        busy: panel.querySelector('[aria-busy="true"]') !== null,
      };
    },
    bar,
    view,
  );
}

/** How a view tab is drawn around what it holds (#3147), read once. */
type Around = {
  /** Headings that say its tab's name, and elements at its top whose own words are it. */
  titles: string[];
  /** The fill of its first row, where it has one. */
  firstFill: string | null;
  /** Whether the panel it is in is named by the chosen tab, and that tab is called `title`. */
  namedByItsTab: boolean;
  /** Where its pane's actions are: their box's distance from the pane's top and right edge. */
  actions: { top: number; right: number } | null;
};

/** How the view whose region is named `region`, in the tab called `tab`, is drawn around what
 *  it holds. A view's region is named by its title; a vault's is `Vault <name>`. */
async function aroundOf(region: string, tab = region): Promise<Around | null> {
  return browser.execute(
    (named: string, called: string) => {
      const pane = [...document.querySelectorAll<HTMLElement>("section.view-pane")].find(
        (one) => one.getAttribute("aria-label") === named && one.getBoundingClientRect().width > 0,
      );
      if (!pane) return null;
      // Where a title would be: the view's top, down to the pane's actions' row and a little.
      const top = pane.getBoundingClientRect().top;
      const titles = [
        ...[...pane.querySelectorAll("h1, h2, h3, h4, h5, h6")]
          .filter((one) => (one.textContent ?? "").trim() === called)
          .map((one) => `heading: ${one.textContent ?? ""}`),
        ...[...pane.querySelectorAll<HTMLElement>("*")]
          .filter(
            (one) =>
              one.getBoundingClientRect().top - top < 48 &&
              [...one.childNodes].some(
                (node) =>
                  node.nodeType === Node.TEXT_NODE && (node.textContent ?? "").trim() !== "",
              ) &&
              (one.innerText ?? "").trim() === called,
          )
          .map((one) => `words: <${one.tagName.toLowerCase()} class="${one.className}">`),
      ];
      const first = pane.querySelector(":scope > .view-head");
      const panel = pane.closest('[role="tabpanel"]');
      const tab = document.getElementById(panel?.getAttribute("aria-labelledby") ?? "");
      const frame = pane.closest(".pane-frame");
      const actions = frame?.querySelector(":scope > .pane-head > .pane-actions");
      const own = frame?.getBoundingClientRect();
      const at = actions?.getBoundingClientRect();
      return {
        titles,
        firstFill: first === null ? null : getComputedStyle(first).backgroundColor,
        namedByItsTab:
          tab !== null &&
          tab.getAttribute("aria-selected") === "true" &&
          (tab.querySelector(".tab-name")?.textContent ?? "").trim() === called,
        actions:
          own === undefined || at === undefined
            ? null
            : { top: at.top - own.top, right: own.right - at.right },
      };
    },
    region,
    tab,
  );
}

/** A view tab is one piece with its tab: no title, no band, the pane's actions in their place,
 *  in the panel its tab names. */
async function onePiece(region: string, tab = region): Promise<void> {
  const around = await aroundOf(region, tab);
  if (around === null) throw new Error(`no view tab called ${tab} on screen`);
  expect(around.titles).toEqual([]);
  if (around.firstFill !== null) expect(around.firstFill).toBe("rgba(0, 0, 0, 0)");
  expect(around.namedByItsTab).toBe(true);
  expect(around.actions).not.toBeNull();
  // At the pane's top right, where a chat's pane draws them (`paneFrame.css`).
  expect(around.actions?.top).toBeLessThanOrEqual(16);
  expect(around.actions?.right).toBeLessThanOrEqual(16);
}

/** What the view tab called `title` holds below its first row, read once. */
async function viewTab(title: string): Promise<Body | null> {
  return browser.execute((called: string) => {
    const pane = [...document.querySelectorAll<HTMLElement>("section.view-pane")].find(
      (one) => one.getAttribute("aria-label") === called && one.getBoundingClientRect().width > 0,
    );
    if (!pane) return null;
    const box = pane.getBoundingClientRect();
    const body = [...pane.children].filter((one) => !one.classList.contains("view-head"));
    const words = body
      .map((one) => (one as HTMLElement).innerText.trim())
      .join(" ")
      .trim();
    const empty = [...pane.querySelectorAll('[data-scope="empty-state"][data-part="root"]')].some(
      (one) =>
        (
          one.querySelector('[data-scope="empty-state"][data-part="headline"]')?.textContent ?? ""
        ).trim() !== "" && one.getBoundingClientRect().height > 0,
    );
    return {
      words,
      empty,
      width: box.width,
      height: box.height,
      busy: pane.querySelector('[aria-busy="true"]') !== null,
    };
  }, title);
}

/** Waits until `read` answers a body that has finished reading, and answers it. */
async function settled(what: string, read: () => Promise<Body | null>): Promise<Body> {
  let body: Body | null = null;
  await browser
    .waitUntil(
      async () => {
        body = await read();
        return body !== null && !body.busy && (body.words !== "" || body.empty);
      },
      { timeout: 30_000, interval: 250 },
    )
    .catch(() => {
      throw new Error(`${what} is blank: ${JSON.stringify(body)}`);
    });
  return body as unknown as Body;
}

/** A body is drawn: it has a size, and words or an empty state in it. */
function drawn(what: string, body: Body): void {
  if (body.width <= 0 || body.height <= 0)
    throw new Error(`${what} has no size: ${JSON.stringify(body)}`);
  expect(body.words !== "" || body.empty).toBe(true);
}

/**
 * The palette's rows that open a view tab on the fixture project and read only the project, by
 * what is typed, their id and the tab's title where it is known. A workspace's Open changes is
 * left out: it asks the forge as it opens, which a scenario run does not reach.
 */
const VIEW_TABS: { typed: string; id: string; title: string | null }[] = [
  { typed: "Open dispatches", id: "dispatches.show", title: "Dispatches" },
  { typed: "Open shared memory", id: "memory.shared", title: "Shared memory" },
  { typed: "Open Skill studio", id: "skills.studio", title: "Skill studio" },
  // At the focused workspace's level, as the palette opens it there (`PlaneView`).
  { typed: "Settings", id: "settings.show", title: "Settings · alpha" },
  { typed: "Show what steward is", id: "persona.show:steward", title: "steward" },
];

/** The vault this file makes, for a vault's tab: the fixture has none. */
const VAULT = "e2e-no-blank-view";

/** Makes {@link VAULT} through the palette's New vault…, which opens its tab in front. */
async function makeTheVault(): Promise<void> {
  await runFromThePalette("New vault", "vault.create");
  const asking = await $('[role="dialog"][aria-labelledby]');
  await browser.waitUntil(async () => (await asking.getText()).includes("New vault"), {
    timeout: 20_000,
    timeoutMsg: "the New vault dialog never came up",
  });
  await asking.$("aria/Name").setValue(VAULT);
  await asking.$("button=Create vault").click();
  await $(`[data-testid="vault-tab-${VAULT}"]`).waitForExist({ timeout: 20_000 });
}

describe("no blank view", function () {
  this.timeout(300_000);

  let found: { bar: string; views: string[]; open: string | null }[] = [];
  let wereAlreadyOpen: string[] = [];

  before(async () => {
    await focusWorkspace("alpha");
    await browser.waitUntil(async () => (await bars()).length > 0, {
      timeout: 30_000,
      interval: 250,
      timeoutMsg: "no activity bar was drawn",
    });
    found = await bars();
    wereAlreadyOpen = await tabNames();
  });

  after(async () => {
    // Each bar on the view it had open, or put away where it was.
    for (const { bar, views, open } of found) {
      if (open !== null) {
        if (!(await isOpen(bar, open))) await pressIcon(bar, open);
      } else {
        // A press of the open view puts its side away again.
        for (const view of views)
          if (await isOpen(bar, view)) {
            await pressIcon(bar, view);
            break;
          }
      }
    }
    await closeTheViewTabsOpenedSince(wereAlreadyOpen);
  });

  it("draws every view of every bar, or its empty state", async () => {
    expect(found.flatMap((one) => one.views).length).toBeGreaterThan(0);
    for (const { bar, views } of found) {
      for (const view of views) {
        if (!(await isOpen(bar, view))) await pressIcon(bar, view);
        await browser.waitUntil(async () => isOpen(bar, view), {
          timeout: 10_000,
          interval: 100,
          timeoutMsg: `the ${view} view on the ${bar} bar never opened`,
        });
        drawn(`the ${view} view on the ${bar} bar`, await settled(view, () => panelOf(bar, view)));
      }
    }
  });

  it("draws every view tab the palette opens on the fixture project, or its empty state", async () => {
    for (const { typed, id, title } of VIEW_TABS) {
      const had = await tabNames();
      await runFromThePalette(typed, id);
      let opened = "";
      await browser.waitUntil(
        async () => {
          const now = await tabNames();
          opened = title ?? now.find((one) => !had.includes(one)) ?? "";
          return opened !== "" && now.includes(opened);
        },
        { timeout: 20_000, interval: 100, timeoutMsg: `${typed} opened no tab` },
      );
      drawn(`the ${opened} tab`, await settled(opened, () => viewTab(opened)));
      await onePiece(opened);
    }
  });

  it("draws a vault's tab as one piece with its tab, with what it holds", async () => {
    await makeTheVault();
    // A vault's region is named for a screen reader as `Vault <name>`; its tab is its name.
    const named = `Vault ${VAULT}`;
    drawn(`the ${VAULT} tab`, await settled(VAULT, () => viewTab(named)));
    // No title, no band, the actions in place, and in the panel its tab, the vault's name,
    // names.
    await onePiece(named, VAULT);
  });
});
