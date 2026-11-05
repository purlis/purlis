// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";
import { specOf } from "./chatsLeft.js";
import { recordFailure } from "./failures.js";
import { openedSince, pressClose, tabNames } from "./reach.js";
import { CURRENT_CHIP, goToWorkspace } from "./titleBarWorkspaces.js";

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
 * - **Both ends of a file are judged, wherever it ended** (#5131). The window draws one strip, and
 *   most files go to a workspace in their setup and stay there, as the next file expects. So a
 *   file that ended on another strip than it began on is judged on that one, which should hold
 *   nothing of its own, and on the one it began on, visited for the reading, and it ends where
 *   it ended. A strip it only passed through is not read.
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

/** Brings strip `to`'s project and workspace to the front, as a person does. */
async function goTo(to: Strip): Promise<void> {
  if (to.project !== "") {
    await browser.execute((project: string) => {
      document
        .querySelector<HTMLElement>(
          `[data-strip="Projects"] [role="tab"][data-directory="${CSS.escape(project)}"]`,
        )
        ?.click();
    }, to.project);
    await browser.waitUntil(async () => (await stripInFront()).project === to.project, {
      timeout: 10_000,
      interval: 250,
    });
  }
  await goToWorkspace(to.workspace === "" ? "Plane root" : to.workspace);
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

let before: Strip | null = null;

/** The tabs' root hooks, run by `leftBehind.ts` after the chats'. */
export const tabHooks = {
  async beforeAll(this: Mocha.Context): Promise<void> {
    // `chatsLeft.ts`'s `beforeAll`, run first, has waited for a project to be drawn.
    try {
      before = await stripInFront();
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
    const spec = specOf(this.test?.parent);
    const faults: string[] = [];

    const ended = await stripInFront();
    if (samePlace(had, ended)) {
      faults.push(...(await judgeTheStripInFront(had, spec)));
    } else {
      // The strip it ended on, then the one it began on, and back to where it ended.
      faults.push(...(await judgeTheStripInFront({ ...ended, tabs: [] }, spec)));
      try {
        await goTo(had);
        faults.push(...(await judgeTheStripInFront(had, spec)));
      } catch (trouble) {
        // Its project may be closed, and with it the strip: there is nothing there to judge.
        console.warn(`${spec}: the strip it began on was not read`, trouble);
      }
      await goTo(ended).catch((trouble: unknown) => {
        console.warn(`${spec}: the guard did not get back to the strip it ended on`, trouble);
      });
    }
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
