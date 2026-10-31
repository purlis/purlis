// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The review page's lint** (`eslint.ui-rules.js` finds this file, #3672):
 *
 * `purlis/review-page-family` (error): `data-scope="review-page"` by hand, which only its
 * wrapper draws. A review too long for a dialog is the family's `ReviewPage`
 * (`ui/review-page/ReviewPage`), whose bar holds the dialog's own answers.
 */

import { moved } from "../familyLint.js";

const reviewPageFamily = moved({
  description: "A review with a fixed action bar is the shared review page",
  scopes: ["review-page"],
  classes: [],
  message:
    "a review page: draw it as `<ReviewPage>` from `ui/review-page/ReviewPage`, whose bar holds the summary and a dialog's answers (#3672).",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "review-page-family": reviewPageFamily,
};
