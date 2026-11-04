// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { chatHooks } from "./chatsLeft.js";
import { tabHooks } from "./tabsLeft.js";

/**
 * **What a spec file left behind, judged once per file**: the chats it left running
 * (`chatsLeft.ts`, #1796), then the tabs it left open (`tabsLeft.ts`, #5111).
 *
 * One module, so one root hook of each kind. Mocha skips the rest of a kind's root hooks once one
 * throws, so with each guard required on its own, a file the chats guard failed was never judged
 * for its tabs, and the tabs it left became the next file's baseline. Here the chats go first, so
 * a chat the file left is ended before its tab is pressed, and the tabs are judged whatever the
 * chats' verdict; the file fails on the first guard that failed it, each failure written down by
 * its own guard.
 */
export const mochaHooks = {
  async beforeAll(this: Mocha.Context): Promise<void> {
    await chatHooks.beforeAll.call(this);
    await tabHooks.beforeAll.call(this);
  },

  async afterAll(this: Mocha.Context): Promise<void> {
    let failed: unknown;
    for (const guard of [chatHooks, tabHooks]) {
      try {
        await guard.afterAll.call(this);
      } catch (error) {
        failed ??= error;
      }
    }
    if (failed !== undefined) throw failed;
  },
};
