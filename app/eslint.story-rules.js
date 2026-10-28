// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The stories' lint** (#3535, `docs/component-stories.md`): what a story's `play` function
 * must not write. `eslint.config.js` turns each rule here on as an error in `*.stories.tsx`.
 *
 * `purlis-stories/focus-wait`: a wait for the keyboard written as
 * `waitFor(() => expect(x).toHaveFocus())`. That is storybook's default 1 s, and the focus comes
 * back a frame or more after an overlay goes, later still when WebKit draws late in a busy run
 * (#3476, #3463). A story waits with `untilFocused` (`src/stories.testkit.ts`): 3 s, and on
 * failure it says where the keyboard is. A wait for the focus to leave (`.not.toHaveFocus()`)
 * has no such helper and stays.
 */

/** Whether `node` is a call of `.toHaveFocus()` that is not under `.not`. */
const isFocusCheck = (node) =>
  node.type === "CallExpression" &&
  node.callee.type === "MemberExpression" &&
  node.callee.property.type === "Identifier" &&
  node.callee.property.name === "toHaveFocus" &&
  !(
    node.callee.object.type === "MemberExpression" &&
    node.callee.object.property.type === "Identifier" &&
    node.callee.object.property.name === "not"
  );

const focusWait = {
  meta: {
    type: "problem",
    docs: { description: "A story waits for the keyboard with untilFocused" },
    messages: {
      focusWait:
        'A focus wait under `waitFor` gives up after storybook\'s 1 s: wait with `untilFocused(() => element, "where in the story")` from `stories.testkit`, which waits 3 s and says where the keyboard is.',
    },
    schema: [],
  },
  create(context) {
    /** The `waitFor(…)` calls being walked, innermost last. */
    const waits = [];
    /** The waits already reported, each once however many checks it holds. */
    const reported = new Set();
    const isWaitFor = (node) => node.callee.type === "Identifier" && node.callee.name === "waitFor";
    return {
      CallExpression(node) {
        if (isWaitFor(node)) waits.push(node);
        else if (waits.length > 0 && isFocusCheck(node)) {
          const wait = waits[waits.length - 1];
          if (reported.has(wait)) return;
          reported.add(wait);
          context.report({ node: wait, messageId: "focusWait" });
        }
      },
      "CallExpression:exit"(node) {
        if (waits[waits.length - 1] === node) waits.pop();
      },
    };
  },
};

/** The stories' rules, by name: `purlis-stories/<name>` in `eslint.config.js`, an error. */
export const purlisStories = {
  meta: { name: "purlis-stories" },
  rules: { "focus-wait": focusWait },
};
