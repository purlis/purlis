// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The message family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/message-family` (error, #5649): its `data-scope` written by hand, which only its
 * wrapper writes. A message of the Inbox's views is the family's `MessageRow`.
 */

import { moved } from "../familyLint.js";

const messageFamily = moved({
  description: "A message of the Inbox's views is the shared message family",
  scopes: ["message"],
  classes: [],
  roles: [],
  elements: [],
  message:
    "a message of the Inbox's views: draw it as `<MessageRow>` with `<MessageTop>`, `<MessageMeta>`, `<MessageBody>`, `<MessageLinks>` and `<MessageForm>` from `ui/message/Message`",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "message-family": messageFamily,
};

/** The files a rule is still off in, until their lanes move them. */
export const off = {};
