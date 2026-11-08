// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";
import { specOf } from "./chatsLeft.js";
import { recordFailure } from "./failures.js";
import { ask, inFront, PROJECTS } from "./switching.js";

/**
 * **Every spec file closes the projects it opened** (#5609), as {@link ./chatsLeft.ts} has it
 * end its chats and {@link ./tabsLeft.ts} close its tabs.
 *
 * One app process serves a whole run, so a project a spec file leaves open is open, and often
 * in front, for every file after it. On the merge queue's macOS runner `make-it-a-project`'s
 * `after` stopped at a close question it could not answer, with this machine's project
 * (`local-project`) open and in front: `memory-move` then read its memory in the wrong project,
 * `onboarding` found the project its steps make already open and took it for the launch's, and
 * its `after` closed the launch's project, which left the window with nothing drawn for every
 * file after. One file's failure failed ten.
 *
 * So this file's root hooks are the backstop, run by `leftBehind.ts` after the chats' guard and
 * before the tabs': `beforeAll` writes down the projects open and the one in front, and
 * `afterAll` closes through the core every project that was not open then, brings the one in
 * front back, and FAILS the file, naming what it left. The files after it start where it did.
 */

/** The projects open, and the one in front, by their directories. */
export interface Projects {
  open: string[];
  front: string | null;
}

/** What a spec file did to the projects it began with: opened and left, or closed. */
export interface Left {
  opened: string[];
  closed: string[];
}

/** The projects open in `now` that were not in `had`, and those in `had` that `now` lacks. */
export function projectsLeft(had: readonly string[], now: readonly string[]): Left {
  return {
    opened: now.filter((one) => !had.includes(one)),
    closed: had.filter((one) => !now.includes(one)),
  };
}

/** What a file is told it left: `""` when it left the projects as it found them. */
export function sayWhatWasLeft(spec: string, left: Left): string {
  const said: string[] = [];
  if (left.opened.length > 0)
    said.push(
      `${spec} left ${left.opened.length === 1 ? "a project" : `${left.opened.length} projects`} open: ${left.opened.join(", ")}; the guard closed ${left.opened.length === 1 ? "it" : "them"}.`,
    );
  if (left.closed.length > 0)
    said.push(
      `${spec} closed ${left.closed.length === 1 ? "a project" : `${left.closed.length} projects`} it found open: ${left.closed.join(", ")}.`,
    );
  if (said.length === 0) return "";
  return `${said.join(" ")} A spec file leaves the projects as it found them, in its own \`after\` (#5609).`;
}

let before: Projects | null = null;

async function projectsNow(): Promise<Projects> {
  return { open: await ask<string[]>("open_planes"), front: await inFront() };
}

/** Brings `project` to the front from its tab on the strip. */
async function bringToTheFront(project: string): Promise<void> {
  await browser.waitUntil(
    async () => {
      if ((await inFront()) === project) return true;
      await browser.execute(
        (strip: string, path: string) =>
          [...document.querySelectorAll<HTMLElement>(`${strip} [role="tab"]`)]
            .find((tab) => tab.getAttribute("data-directory") === path)
            ?.click(),
        PROJECTS,
        project,
      );
      return false;
    },
    { timeout: 20_000, interval: 250, timeoutMsg: `${project} never came back to the front` },
  );
}

export const projectHooks = {
  async beforeAll(this: Mocha.Context): Promise<void> {
    try {
      before = await projectsNow();
    } catch (error) {
      // Not wrapped by WebdriverIO, so written down here, as `chatsLeft.ts`'s is.
      recordFailure(
        {
          title: '"before all" hook: the projects a spec file began with',
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
    before = null;
    const spec = specOf(this.test?.parent);
    // A close the spec asked through the window takes a moment to let go: give it that moment.
    let left = projectsLeft(had.open, await ask<string[]>("open_planes"));
    if (left.opened.length > 0)
      await browser
        .waitUntil(
          async () =>
            (left = projectsLeft(had.open, await ask<string[]>("open_planes"))).opened.length === 0,
          { timeout: 10_000, interval: 250 },
        )
        .catch(() => undefined);
    const faults: string[] = [];
    for (const plane of left.opened)
      await ask("close_plane", { plane }).catch((trouble: unknown) =>
        faults.push(`the guard could not close ${plane}: ${String(trouble)}`),
      );
    if (had.front !== null && (await inFront()) !== had.front)
      await bringToTheFront(had.front).catch((trouble: unknown) => faults.push(String(trouble)));
    const said = sayWhatWasLeft(spec, left);
    if (said !== "") faults.unshift(said);
    if (faults.length === 0) return;

    const error = new Error(faults.join("\n"));
    // Not a hook WebdriverIO wraps, so written down where `tools/e2e-annotations.mjs` reads it.
    recordFailure(
      { title: '"after all" hook: the projects a spec file left', parent: "", file: spec },
      error,
    );
    throw error;
  },
};
