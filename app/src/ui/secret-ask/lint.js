// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Secret ask's lint** (`eslint.ui-rules.js` finds this file, #5080):
 *
 * `purlis/secret-ask-family` (error): its `data-scope` written by hand, which only its wrapper
 * writes. A Secret ask's body is drawn from `ui/secret-ask/SecretAsk`'s parts.
 */

import { moved } from "../familyLint.js";

const secretAskFamily = moved({
  description: "A Secret ask's body is the shared secret ask family",
  scopes: ["secret-ask"],
  classes: [],
  roles: [],
  elements: [],
  message:
    "a Secret ask's body: draw it from `ui/secret-ask/SecretAsk`'s parts, which keep the app's facts apart from the chat's words (#5080)",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "secret-ask-family": secretAskFamily,
};

/** The files a rule is still off in, until their lanes move them. */
export const off = {};
