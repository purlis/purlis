// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";
import { recordFailure } from "./failures.js";
import { focusWorkspaceAt, workspaceTabs } from "./reach.js";
import { runningIn } from "./running.js";
import { endChatById } from "./started.js";
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
 * - **Each spec ends its own chats through the window**, as a person does, and by the number the
 *   core gave each one, never by its tab's name (two chats may share a name): `before`, a spec
 *   writes down {@link chatsRunning}, and `after`, {@link endTheChatsStartedSince} ends every
 *   chat started since, in whichever workspace it is, through `started.ts`'s `endChatById`.
 * - **This file's root hooks are the backstop.** Wired once through `mochaOpts.require` in
 *   `wdio.conf.ts`, which every config but the benchmark's spreads, so every spec file runs
 *   between them: `beforeAll` writes down the chats running in each open project, and `afterAll`
 *   ends, by id, every chat that was not, and then FAILS the file, naming it and the count. A file
 *   that leaves chats behind is a red file, not a quiet cost to the files after it.
 *
 * Ending a chat by id from outside the window (`close_session`) takes no tab away, so the guard
 * is only a backstop that keeps the files after a leaky one honest: the spec is what must clean up.
 * It stays on `close_session` on purpose (#2367): ending through the window takes each tab
 * away, but costs a find, a press and a question per chat, unbounded where the leaky file left
 * the window in a state of its own, and a guard that runs out of time names nothing. The file
 * the guard names is already red, so its tabs are left for its own `after` to take away.
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

/**
 * The chats running in every project the app has open, asked of the app itself: what a spec
 * writes down in its `before` and hands to {@link endTheChatsStartedSince} in its `after`.
 */
export async function chatsRunning(): Promise<Running> {
  const planes = await ask<string[]>("open_planes");
  const now: Running = {};
  for (const plane of planes) {
    // A project that closed between the two answers has no chats to count.
    now[plane] = await runningIn(plane).catch(() => []);
  }
  return now;
}

/** The chats the window answered once it was drawn, waited for: a project opens as it draws. */
async function runningOnceOpen(): Promise<Running> {
  let now: Running = {};
  await browser.waitUntil(
    async () => {
      now = await chatsRunning();
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
    let left = leftOver(had, await chatsRunning());
    if (left.length > 0)
      await browser
        .waitUntil(async () => (left = leftOver(had, await chatsRunning())).length === 0, {
          timeout: 10_000,
          interval: 250,
        })
        .catch(() => undefined);
    if (left.length === 0) return;

    for (const chat of left) await ask("close_session", { ...chat }).catch(() => undefined);
    await browser
      .waitUntil(async () => leftOver(had, await chatsRunning()).length === 0, {
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

/**
 * The order a cleanup looks through the workspace strip in, by each tab's place on it: the one
 * in front first, where most specs start their chats, then the others left to right.
 */
export function workspacesInTurn(strip: readonly { selected: boolean }[]): number[] {
  const front = strip.findIndex((tab) => tab.selected);
  const rest = strip.map((_, at) => at).filter((at) => at !== front);
  return front < 0 ? rest : [front, ...rest];
}

/**
 * Ends, through the window, each of `left` that the workspace in front holds, and answers the
 * ones still running after: a chat of another workspace is neither a row in this workspace's
 * Chats list nor a tab on its strip, so `endChatById` refuses it here and it is tried in the
 * next workspace.
 */
async function endTheOnesHere(left: readonly Chat[]): Promise<Chat[]> {
  const still: Chat[] = [];
  for (const chat of left) {
    try {
      await endChatById(chat.session, chat.plane);
    } catch {
      const running = await runningIn(chat.plane).catch(() => [] as number[]);
      if (running.includes(chat.session)) still.push(chat);
    }
  }
  return still;
}

/**
 * **Ends, through the window and by number, every chat started since `before`**
 * ({@link chatsRunning}), in whichever workspace it was started, and puts the focus back on the
 * workspace it found in front. Each chat is ended as a person ends it (`started.ts`'s
 * `endChatById`: brought forward by its number, its own tab's `×`, the question's Close), so a
 * chat that was running first is never found by a name it shares. One case it does not guard:
 * a split tab's `×` ends the tab's own chat, so a spec chat split beside a chat that was running
 * first ends that chat on the first press. A spec splits only its own chats. A chat it could
 * not end fails the `after`, naming it; the root hook above stays the backstop.
 */
export async function endTheChatsStartedSince(before: Running): Promise<void> {
  let left = leftOver(before, await chatsRunning());
  if (left.length === 0) return;
  const strip = await workspaceTabs();
  const front = strip.findIndex((tab) => tab.selected);
  try {
    for (const at of workspacesInTurn(strip)) {
      if (left.length === 0) break;
      if (at !== front) await focusWorkspaceAt(at);
      left = await endTheOnesHere(left);
    }
    // No workspace strip drawn (a project with one workspace): the window in front is all.
    if (strip.length === 0) left = await endTheOnesHere(left);
  } finally {
    // The focus goes back where it was; a strip that will not take it fails no cleanup.
    if (front >= 0)
      await workspaceTabs()
        .then((now) => (now[front]?.selected === true ? undefined : focusWorkspaceAt(front)))
        .catch(() => undefined);
  }
  if (left.length > 0)
    throw new Error(
      `the window did not end chat ${left.map((one) => one.session).join(", ")}, ` +
        "started by this spec file (#1796)",
    );
}
