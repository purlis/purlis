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
 * saying where the keyboard is. A story waits with `untilFocused`, or `untilFocusWithin` for the
 * keyboard anywhere in a surface (`src/stories.testkit.ts`): 3 s, and on failure it says where
 * the keyboard is. A wait for the focus to leave has no such helper and stays:
 * `.not.toHaveFocus()`, or a read of `activeElement` checked to be gone
 * (`.not.toContainElement(…)`, `.toBe(false)`, `.toBeNull()`). A timeout the rule cannot read (options passed by name, or spread) is said as
 * one it cannot tell.
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

/**
 * What a `waitFor(…)` call's options say of its timeout: `none` (storybook's 1 s), `own` (it sets
 * one), or `unknown` (options passed by name or spread, which the rule cannot read).
 */
const timeoutOf = (wait) => {
  const options = wait.arguments[1];
  if (options === undefined) return "none";
  if (options.type !== "ObjectExpression") return "unknown";
  const named = (property) =>
    property.type === "Property" &&
    !property.computed &&
    property.key.type === "Identifier" &&
    property.key.name === "timeout";
  if (options.properties.some(named)) return "own";
  return options.properties.some((property) => property.type === "SpreadElement")
    ? "unknown"
    : "none";
};

/** What each kind of timeout gives up after, said in the rule's message. */
const GIVES = {
  none: "gives up after storybook's 1 s",
  own: "gives up after its own timeout without saying where the keyboard is",
  unknown:
    "gives up after storybook's 1 s, or after the timeout its options set, without saying where the keyboard is",
};

/** Matchers that say a thing is not there: under `.not`, they say it is. */
const NOTHING = new Set(["toBeNull", "toBeFalsy", "toBeUndefined"]);

/**
 * Whether `node` is a matcher call that checks something is gone: `.not.toContainElement(…)`,
 * `.toBe(false)`, `.toBeNull()`, and not `.not.toBeNull()`. A wait holding one is a wait for the
 * keyboard to leave.
 */
const checksGone = (node) => {
  if (node.type !== "CallExpression" || node.callee.type !== "MemberExpression") return false;
  const matcher = node.callee.property;
  if (matcher.type !== "Identifier") return false;
  const object = node.callee.object;
  const negated =
    object.type === "MemberExpression" &&
    object.property.type === "Identifier" &&
    object.property.name === "not";
  const chain = negated ? object.object : object;
  if (chain.type !== "CallExpression" || chain.callee.type !== "Identifier") return false;
  if (chain.callee.name !== "expect") return false;
  const saysNothing =
    NOTHING.has(matcher.name) ||
    (matcher.name === "toBe" &&
      node.arguments[0]?.type === "Literal" &&
      node.arguments[0].value === false);
  return negated !== saysNothing;
};

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
    /** What each wait being walked reads by hand of the keyboard, and whether it checks a leave. */
    const reads = new Map();
    const isWaitFor = (node) => node.callee.type === "Identifier" && node.callee.name === "waitFor";
    const report = (wait) => {
      if (reported.has(wait)) return;
      reported.add(wait);
      const gives = GIVES[timeoutOf(wait)];
      context.report({ node: wait, messageId: "focusWait", data: { gives } });
    };
    const innermost = () => waits[waits.length - 1];
    const readHere = (node) => {
      if (waits.length > 0 && readsTheFocus(node)) reads.get(innermost()).focus = true;
    };
    return {
      CallExpression(node) {
        if (isWaitFor(node)) {
          waits.push(node);
          reads.set(node, { focus: false, gone: false });
        } else if (waits.length > 0) {
          if (isFocusCheck(node)) report(innermost());
          if (checksGone(node)) reads.get(innermost()).gone = true;
        }
      },
      MemberExpression: readHere,
      Literal: readHere,
      TemplateElement: readHere,
      "CallExpression:exit"(node) {
        if (innermost() !== node) return;
        waits.pop();
        // A read of the keyboard by hand is a wait for it to land, unless it waits for a leave.
        const read = reads.get(node);
        reads.delete(node);
        if (read.focus && !read.gone) report(node);
      },
    };
  },
};

/** The stories' rules, by name: `purlis-stories/<name>` in `eslint.config.js`, an error. */
export const purlisStories = {
  meta: { name: "purlis-stories" },
  rules: { "focus-wait": focusWait },
};
