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
    // The keyboard read where nothing waits, and the helper for the keyboard in a surface.
    { code: `await expect(group).not.toContainElement(document.activeElement);` },
    { code: `await untilFocusWithin(() => card, "Popover: the keyboard is in the card");` },
    { code: `await waitFor(() => expect(row).toHaveClass("focused"));` },
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
    // A wait around where the keyboard is, read by hand (#3610): in a surface, or on a kind of row.
    {
      code: `await waitFor(() => expect(card.contains(document.activeElement)).toBe(true));`,
      errors: [{ messageId: "focusWait" }],
    },
    {
      code: `await waitFor(() => expect(sub).toContainElement(document.activeElement as HTMLElement));`,
      errors: [{ messageId: "focusWait" }],
    },
    {
      code: `await waitFor(() => expect(row.ownerDocument.activeElement).toHaveAttribute("aria-selected", "true"));`,
      errors: [{ messageId: "focusWait" }],
    },
    {
      code: `await waitFor(() => expect(card.matches(":focus-within")).toBe(true));`,
      errors: [{ messageId: "focusWait" }],
    },
    {
      code: "await waitFor(() => expect(list.querySelector(`[role=option]:focus`)).not.toBeNull());",
      errors: [{ messageId: "focusWait" }],
    },
  ],
});

// The message says what the wait gives up after: storybook's 1 s, or the timeout it set (#3610).
tester.run("purlis-stories/focus-wait says the wait it found", purlisStories.rules["focus-wait"], {
  valid: [],
  invalid: [
    {
      code: `await waitFor(() => expect(name).toHaveFocus());`,
      errors: [{ message: /gives up after storybook's 1 s:/ }],
    },
    {
      code: `await waitFor(() => expect(name).toHaveFocus(), { timeout: 3000 });`,
      errors: [{ message: /gives up after its own timeout without saying where the keyboard is:/ }],
    },
  ],
});
