// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The stories' lint** (`eslint.story-rules.js`, #3535): what it refuses and what it lets
 * through, on code a story's `play` function would really write.
 */
import { purlisStories } from "../eslint.story-rules.js";
import { tester } from "./ui/lint.testkit";

tester.run("purlis-stories/focus-wait", purlisStories.rules["focus-wait"], {
  valid: [
    { code: `await untilFocused(() => name, "New workspace: the keyboard starts in Name");` },
    // A check of the focus where nothing waits: the key moved it already.
    { code: `await userEvent.tab(); await expect(name).toHaveFocus();` },
    // A wait for something else, and a wait for the focus to leave, which has no helper.
    { code: `await waitFor(() => expect(menu).toBeVisible());` },
    { code: `await waitFor(() => expect(trigger).not.toHaveFocus());` },
  ],
  invalid: [
    {
      code: `await waitFor(() => expect(name).toHaveFocus());`,
      errors: [{ messageId: "focusWait" }],
    },
    {
      code: `await waitFor(() => expect(page.getByRole("button", { name: "Cancel" })).toHaveFocus());`,
      errors: [{ messageId: "focusWait" }],
    },
    // In a block, with a timeout of its own, and among other checks: each wait once.
    {
      code: `await waitFor(() => { expect(menu).toBeVisible(); expect(row).toHaveFocus(); expect(other).toHaveFocus(); }, { timeout: 3000 });`,
      errors: [{ messageId: "focusWait" }],
    },
  ],
});
