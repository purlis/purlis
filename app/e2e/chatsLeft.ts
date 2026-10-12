// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";
import { recordFailure } from "./failures.js";
import { focusWorkspaceAt, workspaceTabs } from "./reach.js";
import { ask } from "./switching.js";

/**
 * **Every spec file ends the chats it started** (#1796).
 *
 * WebdriverIO's Tauri service keeps ONE app process for a whole run, so a chat a spec file leaves
 * running is still running in every file after it: it is counted by the next file that counts
 * processes, drawn on the strip the next file measures, and it holds the machine's memory for the
 * rest of the run. `panes.e2e.ts` alone used to leave about fifty.
 *
 * Two halves:
 * - **Each spec ends its own chats through the window**, as a person does: the tab's `End chat`
 *   and the question it asks. {@link chatsOnTheStrip} and {@link endTheChatsStartedHere} do that
 *   for the workspace in front, {@link chatsInEachWorkspace} and {@link endTheChatsStartedSince}
 *   for every workspace on the strip.
 * - **This file's root hooks are the backstop.** Wired once through `mochaOpts.require` in
 *   `wdio.conf.ts`, which every config but the benchmark's spreads, so every spec file runs
 *   between them: `beforeAll` writes down the chats running in each open project, and `afterAll`
 *   ends, by id, every chat that was not, and then FAILS the file, naming it and the count. A file
 *   that leaves chats behind is a red file, not a quiet cost to the files after it.
 *
 * Ending a chat by id from outside the window (`close_session`) takes no tab away, so the guard
 * is only a backstop that keeps the files after a leaky one honest: the spec is what must clean up.
 */

/** The chats running in each open project, by the project's path. */
export type Running = Record<string, number[]>;

/** One chat, as the app's commands name it. */
export interface Chat {
  plane: string;
  session: number;
}

/**
 * The chats running `now` that were not running `before`: every chat of a project the file
 * opened, and none of a project it closed again or a chat that ended on its own.
 */
export function leftOver(before: Running, now: Running): Chat[] {
  const left: Chat[] = [];
  for (const [plane, sessions] of Object.entries(now)) {
    const had = new Set(before[plane] ?? []);
    for (const session of sessions) if (!had.has(session)) left.push({ plane, session });
  }
  return left;
}

/** What the guard says when a file left chats running: the file, how many, and which. */
export function leftOverSaid(spec: string, left: readonly Chat[]): string {
  const shown = spec.includes("specs/") ? spec.slice(spec.lastIndexOf("specs/")) : spec;
  const many = left.length === 1 ? "1 chat" : `${left.length} chats`;
  const which = left.map((one) => one.session).join(", ");
  return (
    `${shown} left ${many} running (session ${which}); the guard ended them. ` +
    `A spec file ends the chats it starts, through the window, in its own \`after\` (#1796).`
  );
}

/** The spec file a root hook ran for: the file of the first suite under mocha's root suite. */
export function specOf(root: { suites?: readonly { file?: string }[] } | undefined): string {
  return root?.suites?.[0]?.file ?? "(a spec file mocha did not name)";
}

/** The chats running in every project the app has open, asked of the app itself. */
async function running(): Promise<Running> {
  const planes = await ask<string[]>("open_planes");
  const now: Running = {};
  for (const plane of planes) {
    // A project that closed between the two answers has no chats to count.
    now[plane] = await ask<number[]>("running_sessions", { plane }).catch(() => []);
  }
  return now;
}

/** The chats the window answered once it was drawn, waited for: a project opens as it draws. */
async function runningOnceOpen(): Promise<Running> {
  let now: Running = {};
  await browser.waitUntil(
    async () => {
      now = await running();
      return Object.keys(now).length > 0;
    },
    { timeout: 30_000, interval: 250, timeoutMsg: "the app had no project open" },
  );
  return now;
}

let before: Running | null = null;

/**
 * Mocha's root hooks, which `handleRequires` collects from a module named in `mochaOpts.require`.
 * They run once per spec file, because each file is a worker of its own.
 */
export const mochaHooks = {
  async beforeAll(this: Mocha.Context): Promise<void> {
    try {
      before = await runningOnceOpen();
    } catch (error) {
      // Not wrapped by WebdriverIO either (below), so written down here.
      recordFailure(
        {
          title: '"before all" hook: the chats a spec file began with',
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
    // A chat the spec ended through the window takes a moment to go: give it that moment.
    let left = leftOver(had, await running());
    if (left.length > 0)
      await browser
        .waitUntil(async () => (left = leftOver(had, await running())).length === 0, {
          timeout: 10_000,
          interval: 250,
        })
        .catch(() => undefined);
    if (left.length === 0) return;

    for (const chat of left) await ask("close_session", { ...chat }).catch(() => undefined);
    await browser
      .waitUntil(async () => leftOver(had, await running()).length === 0, {
        timeout: 20_000,
        interval: 250,
      })
      .catch(() => undefined);

    const error = new Error(leftOverSaid(spec, left));
    // A root hook is not one WebdriverIO wraps, so its `afterHook` never hears this failure:
    // it is written down here, where `tools/e2e-annotations.mjs` reads every other one.
    recordFailure(
      { title: '"after all" hook: the chats a spec file left', parent: "", file: spec },
      error,
    );
    throw error;
  },
};

const TABS = '[data-strip="Tabs"]';
const ENDS = `${TABS} button[aria-label^="End chat "]`;
const END = "End chat ";

/** The chats on the focused workspace's strip, by the name each tab's `End chat` carries. */
export async function chatsOnTheStrip(): Promise<string[]> {
  return browser.execute(
    (ends: string, end: string) =>
      [...document.querySelectorAll(ends)].map((button) =>
        (button.getAttribute("aria-label") ?? "").slice(end.length),
      ),
    ENDS,
    END,
  );
}

/** Answers `Close` to the question a chat's `End chat` asks, by the button's own click. */
async function answerClose(asked: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute((said: string) => {
        const asking = document.querySelector('[role="alertdialog"]');
        if (asking === null || !(asking.textContent ?? "").includes(said)) return false;
        const close = [...asking.querySelectorAll<HTMLButtonElement>("button")].find(
          (button) => button.textContent?.trim() === "Close",
        );
        close?.click();
        return close !== undefined;
      }, asked),
    { timeout: 20_000, interval: 100, timeoutMsg: `${asked} never asked whether to end it` },
  );
  await browser.waitUntil(
    async () => browser.execute(() => document.querySelector('[role="alertdialog"]') === null),
    { timeout: 20_000, interval: 100, timeoutMsg: `the question ${asked} asked never went` },
  );
}

/**
 * Ends, through the window, every chat on the focused workspace's strip that `before` did not
 * name. **The strip collapses rather than scrolling** (ADR 0039), so at fifty chats it draws a
 * handful: closing a drawn tab lets a hidden one onto the strip, which is why this is a loop and
 * not a list, and it is bounded so a close that stops taking fails rather than hangs.
 */
export async function endTheChatsStartedHere(before: readonly string[], most = 200): Promise<void> {
  for (let pressed = 0; pressed < most; pressed++) {
    const asked = await browser.execute(
      (ends: string, end: string, kept: string[]) => {
        const button = [...document.querySelectorAll<HTMLElement>(ends)].find(
          (one) => !kept.includes((one.getAttribute("aria-label") ?? "").slice(end.length)),
        );
        button?.click();
        return button?.getAttribute("aria-label") ?? null;
      },
      ENDS,
      END,
      [...before],
    );
    if (asked === null) return;
    await answerClose(asked);
  }
  throw new Error(`${most} presses did not end every chat this spec started`);
}

/** The chats on each workspace's strip, by the workspace's name, the focus put back after. */
export async function chatsInEachWorkspace(): Promise<Record<string, string[]>> {
  const strip = await workspaceTabs();
  const front = strip.findIndex((tab) => tab.selected);
  const chats: Record<string, string[]> = {};
  for (const [at, tab] of strip.entries()) {
    await focusWorkspaceAt(at);
    chats[tab.name] = await chatsOnTheStrip();
  }
  if (front >= 0) await focusWorkspaceAt(front);
  return chats;
}

/**
 * Ends, through the window, every chat started in any workspace since `before`
 * ({@link chatsInEachWorkspace}), and puts the focus back on the workspace it found in front.
 */
export async function endTheChatsStartedSince(before: Record<string, string[]>): Promise<void> {
  const strip = await workspaceTabs();
  const front = strip.findIndex((tab) => tab.selected);
  for (const [at, tab] of strip.entries()) {
    await focusWorkspaceAt(at);
    await endTheChatsStartedHere(before[tab.name] ?? []);
  }
  if (front >= 0) await focusWorkspaceAt(front);
}
