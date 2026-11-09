// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The list family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/list-family` (error: the list family has moved, #2911): a list's role written by hand
 * (`listbox`, `option`), its old class (`ui-list`) and its `data-scope`. A list to choose from
 * is the family's `ListBox`, a list of rows that act its `List` (`ui/list/List`); a list of
 * words (a sentence's bullets) is no collection and stays a `<ul>`.
 */

import { moved } from "../familyLint.js";

const listFamily = moved({
  description: "A list is the shared list family",
  scopes: ["list"],
  classes: ["ui-list"],
  roles: ["listbox", "option"],
  message:
    "a list: draw it as `<ListBox>` and `<ListOption>` (a list to choose from) or `<List>` and `<ListRow>` (rows that act), with the row's slots, from `ui/list/List` (#2911).",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "list-family": listFamily,
};
