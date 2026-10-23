// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";
import { ask } from "./switching.js";

/**
 * **Which project is in front, and which chats it runs**, as the app's own commands answer.
 *
 * A leaf: it imports no other helper of the e2e's that reaches a chat, so `reach.ts`,
 * `started.ts` and `chatsLeft.ts` can each use it without importing one another in a circle
 * (#1828).
 */

/** The chats `plane` is running, by number. */
export async function runningIn(plane: string): Promise<number[]> {
  return ask<number[]>("running_sessions", { plane });
}

/**
 * The project in front, as the app names it to its commands (`open_planes`): the one whose path
 * its tab on the projects strip carries, else the first the app has open. It is the project's
 * folder, so a spec writes into it by this path.
 */
export async function projectInFront(): Promise<string> {
  const front = await browser.execute(
    () =>
      document
        .querySelector('[data-strip="Projects"] [role="tab"][aria-selected="true"]')
        ?.getAttribute("data-directory") ?? null,
  );
  let open: string[] = [];
  await browser.waitUntil(
    async () => {
      open = await ask<string[]>("open_planes");
      return open.length > 0;
    },
    { timeout: 30_000, interval: 250, timeoutMsg: "the app had no project open" },
  );
  return open.find((one) => one === front) ?? open[0];
}
