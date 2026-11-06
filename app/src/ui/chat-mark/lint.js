// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The chat mark family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/chat-mark-family` (error: the family has moved, #5340): the old marks' classes, and its
 * `data-scope` written by hand, which only its wrapper draws.
 */

import { moved } from "../familyLint.js";

const chatMarkFamily = moved({
  description: "A chat's mark is the shared ChatMark family",
  scopes: ["chat-mark"],
  classes: [
    "chat-persona",
    "state-dot",
    "tab-persona",
    "state",
    "state-unknown",
    "state-running",
    "state-waiting",
    "state-done",
    "state-failed",
  ],
  message: "a chat's mark: draw it as `<ChatMark>` from `ui/chat-mark/ChatMark` (#5340).",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "chat-mark-family": chatMarkFamily,
};
