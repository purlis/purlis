// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The notice family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/notice-family` (error: the family has moved, #2912): its old classes, and its `data-scope`
 * written by hand, which only its wrapper draws.
 */

import { moved } from "../familyLint.js";

const noticeFamily = moved({
  description: "A Notice is drawn by the shared notice family",
  scopes: ["notice", "notice-list", "notice-row"],
  classes: [
    "notice",
    "notice-*",
    "pane-notice-row",
    "pane-notice-row-*",
    "pane-notices",
    "window-notices",
  ],
  message:
    "a Notice: draw it as `<Notice>` (`src/Notice.tsx`), which draws through `ui/notice/Notice` (#2912).",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "notice-family": noticeFamily,
};
