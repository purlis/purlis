// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";
import { specOf } from "./chatsLeft.js";
import { recordFailure } from "./failures.js";
import { openedSince, pressClose, tabNames } from "./reach.js";
import { CURRENT_CHIP } from "./titleBarWorkspaces.js";

/**
 * **Every spec file closes the tabs it opened** (#5111), as {@link ./chatsLeft.ts} has every
 * file end the chats it started (#1796).
 *
 * One app process serves a whole run, so a tab a spec file leaves on the strip is on the strip
 * of every file after it: the settings specs used to leave their Settings tab for the rest of
 * their shard, and a spec that counted tabs passed or failed by the order it ran in.
 *
 * Two halves, as for chats:
 * - **Each spec closes its own tabs through the window**: `before`, it writes down `reach.ts`'s
 *   `tabNames`, and `after`, `closeTheViewTabsOpenedSince` closes each tab that was not there by
 *   its own `Close <name>` and waits until the strip is as it was.
 * - **This file's root hooks are the backstop**, run by `leftBehind.ts` after `chatsLeft.ts`'s:
 *   `beforeAll` writes down the strip in front and where it is (the project in
 *   front and its workspace), and `afterAll`, when the same strip is in front again, closes every
 *   tab on it that was not there, by its own `Close <name>`, and FAILS the file, naming it and
 *   the tabs. A file that leaves tabs behind is a red file, not a quiet cost to the files after.
 *
 * It reads only the strip in front, the one strip the window draws: a file that leaves another
 * project or workspace in front is not judged on a strip it cannot see.
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

/**
 * The tabs on `now` that were not on `before` (`reach.ts`'s {@link openedSince}, the rule a
 * spec's own `after` uses), when both are the same strip; none when they are not, since another
 * strip says nothing about this one.
 */
export function tabsLeftOver(before: Strip, now: Strip): string[] {
  if (before.project !== now.project || before.workspace !== now.workspace) return [];
  return openedSince(before.tabs, now.tabs);
}

/** What the guard says when a file left tabs open: the file, how many, and which. */
export function tabsLeftSaid(spec: string, left: readonly string[]): string {
  const shown = spec.includes("specs/") ? spec.slice(spec.lastIndexOf("specs/")) : spec;
  const many = left.length === 1 ? "1 tab" : `${left.length} tabs`;
  return (
    `${shown} left ${many} open (${left.join(", ")}); the guard closed them. ` +
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
    // A tab the spec closed through the window takes a moment to go: give it that moment.
    let left = tabsLeftOver(had, await stripInFront());
    if (left.length > 0)
      await browser
        .waitUntil(async () => (left = tabsLeftOver(had, await stripInFront())).length === 0, {
          timeout: 5_000,
          interval: 250,
        })
        .catch(() => undefined);
    if (left.length === 0) return;

    // `chatsLeft.ts`'s `afterAll`, run first, has already ended any chat the file left, so
    // what is pressed here is a view's tab or an ended chat's. The file is red either way.
    await pressClose(left).catch(() => undefined);
    await browser
      .waitUntil(async () => tabsLeftOver(had, await stripInFront()).length === 0, {
        timeout: 10_000,
        interval: 250,
      })
      .catch(() => undefined);

    const error = new Error(tabsLeftSaid(spec, left));
    // Not a hook WebdriverIO wraps, so written down where `tools/e2e-annotations.mjs` reads it.
    recordFailure(
      { title: '"after all" hook: the tabs a spec file left', parent: "", file: spec },
      error,
    );
    throw error;
  },
};
