// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { chatHooks } from "./chatsLeft.js";
import { interruptHooks } from "./interrupts.js";
import { projectHooks } from "./projectsLeft.js";
import { tabHooks } from "./tabsLeft.js";

/**
 * **What a spec file left behind, judged once per file**: the chats it left running
 * (`chatsLeft.ts`, #1796), then the projects it left open (`projectsLeft.ts`, #5609), then the
 * tabs it left open (`tabsLeft.ts`, #5111).
 *
 * One module, so one root hook of each kind. Mocha skips the rest of a kind's root hooks once one
 * throws, so with each guard required on its own, a file the chats guard failed was never judged
 * for its tabs, and the tabs it left became the next file's baseline. Here the chats go first, so
 * a chat the file left is ended before its tab is pressed, and the tabs are judged whatever the
 * chats' verdict; the file fails on the first guard that failed it, each failure written down by
 * its own guard. The projects go before the tabs so the tabs are judged on the strip the file
 * began on, with the launch's project back in front.
 *
 * **The interruptions each flow costs** are counted here too (`interrupts.ts`, #6021): around
 * every test, and judged once the file's own guards have, so a file is failed for what it left
 * behind first.
 */
export const mochaHooks = {
  async beforeAll(this: Mocha.Context): Promise<void> {
    await chatHooks.beforeAll.call(this);
    await projectHooks.beforeAll.call(this);
    await tabHooks.beforeAll.call(this);
    interruptHooks.beforeAll();
  },

  async beforeEach(): Promise<void> {
    await interruptHooks.beforeEach();
  },

  async afterEach(this: Mocha.Context): Promise<void> {
    await interruptHooks.afterEach.call(this);
  },

  async afterAll(this: Mocha.Context): Promise<void> {
    let failed: unknown;
    for (const guard of [chatHooks, projectHooks, tabHooks, interruptHooks]) {
      try {
        await guard.afterAll.call(this);
      } catch (error) {
        failed ??= error;
      }
    }
    if (failed !== undefined) throw failed;
  },
};
