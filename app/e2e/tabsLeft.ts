// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";
import { specOf } from "./chatsLeft.js";
import { recordFailure } from "./failures.js";
import { openedSince, pressClose, TABS, tabNames } from "./reach.js";
import { CURRENT_CHIP, goToWorkspace, workspacesListed } from "./titleBarWorkspaces.js";

/**
 * **Every spec file closes the tabs it opened** (#5111), as {@link ./chatsLeft.ts} has every
 * file end the chats it started (#1796).
 *
 * One app process serves a whole run, so a tab a spec file leaves on the strip is on the strip
 * of every file after it: the settings specs used to leave their Settings tab for the rest of
 * their shard, and a spec that counted tabs passed or failed by the order it ran in.
 *
 * Three parts, the first two as for chats:
 * - **Each spec closes its own tabs through the window**: `before`, it writes down `reach.ts`'s
 *   `tabNames`, and `after`, `closeTheViewTabsOpenedSince` closes each tab that was not there by
 *   its own `Close <name>` and waits until the strip is as it was.
 * - **This file's root hooks are the backstop**, run by `leftBehind.ts` after `chatsLeft.ts`'s:
 *   `beforeAll` writes down the strip in front and where it is (the project in
 *   front and its workspace), and `afterAll`, when the same strip is in front again, closes every
 *   tab on it that was not there, by its own `Close <name>`, and FAILS the file, naming it and
 *   the tabs. A file that leaves tabs behind is a red file, not a quiet cost to the files after.
 * - **Every strip a file visited is judged, wherever it ended** (#5131, #5269). The window draws
 *   one strip, and most files go to a workspace in their setup and stay there, as the next file
 *   expects. So `beforeAll` also starts a watch in the page ({@link watchTheStrips}) that writes
 *   down each project and workspace that comes to the front, with the tabs it held as it came.
 *   `afterAll` judges the strip the file ended on, then goes to each other strip it visited and
 *   judges it against what it held as the file came to it, then the one it began on, and ends
 *   where it ended ({@link stripsToJudge}). A strip the file ended on that the watch never saw
 *   arrive is judged as holding nothing of its own.
 */

/** The strip in front: the project and the workspace it is drawn for, and its tabs' names. */
export interface Strip {
  /** The project in front, by its directory; `""` when none is. */
  project: string;
  /** The workspace in front, by its chip's name; `""` for the plane root, which has none. */
  workspace: string;
  /** The names on the strip, left to right. */
  tabs: string[];
}

/** Whether two strips stand in the same project and workspace, whatever their tabs. A project of
 *  `""` says nothing, since a narrow window does not draw the rail: a file that ends in another
 *  project behind a hidden rail is not seen, which is why each spec's `after` brings the rail back. */
export function samePlace(one: Strip, other: Strip): boolean {
  return (
    one.workspace === other.workspace &&
    (one.project === "" || other.project === "" || one.project === other.project)
  );
}

/**
 * The tabs on `now` that were not on `before` (`reach.ts`'s {@link openedSince}, the rule a
 * spec's own `after` uses), when both are the same strip; none when they are not, since another
 * strip says nothing about this one.
 */
export function tabsLeftOver(before: Strip, now: Strip): string[] {
  if (before.project !== now.project || before.workspace !== now.workspace) return [];
  return openedSince(before.tabs, now.tabs);
}

/**
 * The strips seen so far with `now` added, when its project and workspace are not among them yet:
 * each strip as it held when it first came to the front.
 */
export function arrivedAt(seen: readonly Strip[], now: Strip): Strip[] {
  const known = seen.some((one) => one.project === now.project && one.workspace === now.workspace);
  return known ? [...seen] : [...seen, now];
}

/**
 * **The strips a file is judged on, each with what it held before the file left anything**, in
 * the order the guard reads them: the one it ended on (read where it stands), every other one
 * it visited, and the one it began on, each once. The one it began on is judged against what it
 * held as the file began; another, against what it held as the file first came to it; the one
 * it ended on, never seen to arrive, as holding nothing of its own.
 */
export function stripsToJudge(began: Strip, ended: Strip, visited: readonly Strip[]): Strip[] {
  const judged: Strip[] = [];
  const has = (strip: Strip) => judged.some((one) => samePlace(one, strip));
  if (samePlace(began, ended)) judged.push(began);
  else
    judged.push(
      visited.find((one) => samePlace(one, ended)) ?? {
        ...ended,
        tabs: [],
      },
    );
  // Each visited strip once, matched exactly: `samePlace` lets a project of `""` stand for any,
  // which would drop another project's strip seen behind a hidden rail.
  const exact = (one: Strip, other: Strip) =>
    one.project === other.project && one.workspace === other.workspace;
  for (const one of visited)
    if (!samePlace(one, began) && !judged.some((seen) => exact(seen, one))) judged.push(one);
  if (!has(began)) judged.push(began);
  return judged;
}

/** A spec file as the guard names it. */
const shownOf = (spec: string) =>
  spec.includes("specs/") ? spec.slice(spec.lastIndexOf("specs/")) : spec;

/** How a strip reads in what the guard says. */
const placeOf = ({ project, workspace }: Strip) =>
  `${project === "" ? "the project" : project}, ${workspace === "" ? "the plane root" : workspace}`;

/** What the guard says when a file left tabs open: the file, how many, and which. */
export function tabsLeftSaid(spec: string, left: readonly string[], on: Strip): string {
  const shown = shownOf(spec);
  const many = left.length === 1 ? "1 tab" : `${left.length} tabs`;
  return (
    `${shown} left ${many} open on ${placeOf(on)} (${left.join(", ")}); the guard closed them. ` +
    "A spec file closes the tabs it opens, through the window, in its own `after` (#5111)."
  );
}

/** The strip in front: where it is, read from the page, and its tabs. */
export async function stripInFront(): Promise<Strip> {
  const where = await browser.execute(
    (chip: string) => ({
      project:
        document
          .querySelector('[data-strip="Projects"] [role="tab"][aria-selected="true"]')
          ?.getAttribute("data-directory") ?? "",
      workspace: document.querySelector(`${chip} [data-part="name"]`)?.textContent?.trim() ?? "",
    }),
    CURRENT_CHIP,
  );
  return { ...where, tabs: await tabNames() };
}

/**
 * Brings strip `to`'s project and workspace to the front, by the rail's tab and the title bar's
 * ⌄. A project no longer
 * on the rail, or a workspace no longer listed, fails at once: a file that closed a project or
 * removed a workspace took its strip with it, and there is nothing there to judge.
 */
async function goTo(to: Strip): Promise<void> {
  if (to.project !== "") {
    const there = await browser.execute((project: string) => {
      const tab = document.querySelector<HTMLElement>(
        `[data-strip="Projects"] [role="tab"][data-directory="${CSS.escape(project)}"]`,
      );
      tab?.click();
      return tab !== null;
    }, to.project);
    if (!there) throw new Error(`${to.project} is no longer on the rail`);
    await browser.waitUntil(async () => (await stripInFront()).project === to.project, {
      timeout: 10_000,
      interval: 250,
    });
  }
  const name = to.workspace === "" ? "Plane root" : to.workspace;
  // Read twice, a second apart, for a list still drawing its rows; each read is awaited whole,
  // so no read of the ⌄ is left running behind the next step.
  const isListed = async () => (await workspacesListed()).some((one) => one.name === name);
  let listed = await isListed();
  if (!listed) {
    await browser.pause(1_000);
    listed = await isListed();
  }
  if (!listed) throw new Error(`${name} is no longer listed`);
  await goToWorkspace(name);
}

/**
 * Judges the strip in front against `had`, closes what the file left on it, and says so; none
 * when it left nothing. A tab the spec closed through the window takes a moment to go: it gets
 * that moment.
 */
async function judgeTheStripInFront(had: Strip, spec: string): Promise<string[]> {
  let left = tabsLeftOver(had, await stripInFront());
  if (left.length > 0)
    await browser
      .waitUntil(async () => (left = tabsLeftOver(had, await stripInFront())).length === 0, {
        timeout: 5_000,
        interval: 250,
      })
      .catch(() => undefined);
  if (left.length === 0) return [];
  // `chatsLeft.ts`'s `afterAll`, run first, has already ended any chat the file left, so
  // what is pressed here is a view's tab or an ended chat's. The file is red either way.
  await pressClose(left).catch(() => undefined);
  await browser
    .waitUntil(async () => tabsLeftOver(had, await stripInFront()).length === 0, {
      timeout: 10_000,
      interval: 250,
    })
    .catch(() => undefined);
  return [tabsLeftSaid(spec, left, had)];
}

/** Where the page keeps what {@link watchTheStrips} saw. */
const WATCHED = "__purlisStripsVisited";

/**
 * **Writes down, in the page, every strip that comes to the front from now on** (#5269), with
 * the tabs it holds as it comes, by {@link arrivedAt}'s rule, from a fresh list. A strip is
 * written down once its place and its tabs have read the same for 500 ms: a switch draws the
 * title bar's chip and the strip in turn, and a reading between them would pair one strip's
 * place with another's tabs. The cost: a tab opened within that moment of arriving counts as the
 * strip's own. Read again after any change to the page, at most once in 150 ms, so a terminal
 * that writes all the time never starves it. A page loaded again loses it, and the
 * guard then reads only the strips it began and ended on.
 */
async function watchTheStrips(): Promise<void> {
  await browser.execute(
    (chip: string, tabs: string, key: string) => {
      const page = window as unknown as Record<string, { settled: Strip[]; stop: () => void }>;
      page[key]?.stop();
      // The same reading as `stripInFront` and `reach.ts`'s `tabNames`, here because a function
      // the page runs takes nothing from this module: keep the three in step.
      const read = (): Strip => ({
        project:
          document
            .querySelector('[data-strip="Projects"] [role="tab"][aria-selected="true"]')
            ?.getAttribute("data-directory") ?? "",
        workspace: document.querySelector(`${chip} [data-part="name"]`)?.textContent?.trim() ?? "",
        tabs: [...(document.querySelector(tabs)?.querySelectorAll('[role="tab"]') ?? [])].map(
          (tab) => tab.querySelector(".tab-name")?.textContent ?? "",
        ),
      });
      const same = (one: Strip, other: Strip) =>
        one.project === other.project &&
        one.workspace === other.workspace &&
        one.tabs.join("\u0000") === other.tabs.join("\u0000");
      // Each reading that held still, once each time it changed; `arrivedAt` keeps the first of
      // each place.
      const settled: Strip[] = [read()];
      let last = settled[0];
      let since = performance.now();
      let timer: ReturnType<typeof setTimeout> | undefined;
      const settle = () => {
        timer = undefined;
        const now = read();
        if (!same(now, last)) {
          last = now;
          since = performance.now();
        }
        if (performance.now() - since >= 500) {
          if (!same(now, settled[settled.length - 1])) settled.push(now);
          return;
        }
        timer = setTimeout(settle, 150);
      };
      const changed = () => {
        if (timer === undefined) timer = setTimeout(settle, 150);
      };
      const watching = new MutationObserver(changed);
      watching.observe(document.body, {
        subtree: true,
        childList: true,
        attributes: true,
        characterData: true,
      });
      page[key] = {
        settled,
        stop: () => {
          watching.disconnect();
          if (timer !== undefined) clearTimeout(timer);
        },
      };
    },
    CURRENT_CHIP,
    TABS,
    WATCHED,
  );
}

/** The strips the page's watch saw, and stops it; none when the page lost it. */
async function theStripsWatched(): Promise<Strip[]> {
  const settled = await browser.execute((key: string) => {
    const page = window as unknown as Record<
      string,
      { settled: Strip[]; stop: () => void } | undefined
    >;
    const watched = page[key];
    if (watched === undefined) return [];
    watched.stop();
    page[key] = undefined;
    return watched.settled;
  }, WATCHED);
  return settled.reduce<Strip[]>(arrivedAt, []);
}

let before: Strip | null = null;

/** The tabs' root hooks, run by `leftBehind.ts` after the chats'. */
export const tabHooks = {
  async beforeAll(this: Mocha.Context): Promise<void> {
    // `chatsLeft.ts`'s `beforeAll`, run first, has waited for a project to be drawn.
    try {
      before = await stripInFront();
      await watchTheStrips();
    } catch (error) {
      // Not wrapped by WebdriverIO, so written down here, as `chatsLeft.ts`'s is.
      recordFailure(
        {
          title: '"before all" hook: the tabs a spec file began with',
          parent: "",
          file: specOf(this.test?.parent),
        },
        error,
      );
      throw error;
    }
  },

  async afterAll(this: Mocha.Context): Promise<void> {
    if (before === null) return;
    const had = before;
    // Read once: a later file whose `beforeAll` fails before reading is judged against nothing.
    before = null;
    const spec = specOf(this.test?.parent);
    const faults: string[] = [];

    const visited = await theStripsWatched().catch((trouble: unknown) => {
      console.warn(`${spec}: the strips it visited were not read`, trouble);
      return [];
    });
    if (visited.length === 0)
      console.warn(`${spec}: the page lost the strips it visited; only its two ends are read`);
    const ended = await stripInFront();
    const [here, ...elsewhere] = stripsToJudge(had, ended, visited);
    faults.push(...(await judgeTheStripInFront(here, spec)));
    for (const strip of elsewhere) {
      try {
        await goTo(strip);
        faults.push(...(await judgeTheStripInFront(strip, spec)));
      } catch (trouble) {
        // Its project may be closed, and with it the strip: there is nothing there to judge.
        console.warn(`${spec}: the strip at ${placeOf(strip)} was not read`, trouble);
      }
    }
    if (elsewhere.length > 0)
      await goTo(ended).catch((trouble: unknown) => {
        console.warn(`${spec}: the guard did not get back to the strip it ended on`, trouble);
      });
    if (faults.length === 0) return;

    const error = new Error(faults.join("\n"));
    // Not a hook WebdriverIO wraps, so written down where `tools/e2e-annotations.mjs` reads it.
    recordFailure(
      {
        title: '"after all" hook: the tabs a spec file left',
        parent: "",
        file: spec,
      },
      error,
    );
    throw error;
  },
};
