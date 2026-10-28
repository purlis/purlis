// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The stories' lint** (#3535, `docs/component-stories.md`): what a story's `play` function
 * must not write. `eslint.config.js` turns each rule here on as an error in `*.stories.tsx`.
 *
 * `purlis-stories/focus-wait`: a wait for the keyboard written as
 * `waitFor(() => expect(x).toHaveFocus())`, or as a `waitFor` around a check of
 * `document.activeElement` or of a `:focus-within` selector (#3610). That is storybook's default
 * 1 s, and the focus comes back a frame or more after an overlay goes, later still when WebKit
 * draws late in a busy run (#3476, #3463); with a timeout of its own it still fails without
 * saying where the keyboard is. A story waits with `untilFocused`, or `untilFocusWithin` for the keyboard anywhere
 * in a surface (`src/stories.testkit.ts`): 3 s, and on failure it says where the keyboard is. A
 * wait for the focus to leave (`.not.toHaveFocus()`) has no such helper and stays.
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

/** Whether `node` reads where the keyboard is: `….activeElement`, or a `:focus`, `:focus-within`
 *  or `:focus-visible` selector. */
const readsTheFocus = (node) =>
  (node.type === "MemberExpression" &&
    !node.computed &&
    node.property.type === "Identifier" &&
    node.property.name === "activeElement") ||
  ((node.type === "Literal" || node.type === "TemplateElement") &&
    /:focus(?:-within|-visible)?(?![\w-])/.test(
      typeof node.value === "string" ? node.value : (node.value?.raw ?? ""),
    ));

/** Whether a `waitFor(…)` call sets its own timeout. */
const hasTimeout = (wait) =>
  wait.arguments[1]?.type === "ObjectExpression" &&
  wait.arguments[1].properties.some(
    (property) =>
      property.type === "Property" &&
      property.key.type === "Identifier" &&
      property.key.name === "timeout",
  );

const focusWait = {
  meta: {
    type: "problem",
    docs: { description: "A story waits for the keyboard with untilFocused" },
    messages: {
      focusWait:
        'A focus wait under `waitFor` {{gives}}: wait with `untilFocused(() => element, "where in the story")`, or `untilFocusWithin` for the keyboard anywhere in a surface, from `stories.testkit`, which waits 3 s and says where the keyboard is.',
    },
    schema: [],
  },
  create(context) {
    /** The `waitFor(…)` calls being walked, innermost last. */
    const waits = [];
    /** The waits already reported, each once however many checks it holds. */
    const reported = new Set();
    const isWaitFor = (node) => node.callee.type === "Identifier" && node.callee.name === "waitFor";
    const report = () => {
      const wait = waits[waits.length - 1];
      if (reported.has(wait)) return;
      reported.add(wait);
      const gives = hasTimeout(wait)
        ? "gives up after its own timeout without saying where the keyboard is"
        : "gives up after storybook's 1 s";
      context.report({ node: wait, messageId: "focusWait", data: { gives } });
    };
    return {
      CallExpression(node) {
        if (isWaitFor(node)) waits.push(node);
        else if (waits.length > 0 && isFocusCheck(node)) report();
      },
      MemberExpression(node) {
        if (waits.length > 0 && readsTheFocus(node)) report();
      },
      Literal(node) {
        if (waits.length > 0 && readsTheFocus(node)) report();
      },
      TemplateElement(node) {
        if (waits.length > 0 && readsTheFocus(node)) report();
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
