// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The component layer's lint** (#2904, spec #2897, ADR 0104): a screen draws a control from
 * its family in `src/ui/<family>`, never by hand.
 *
 * - `purlis/button-family` (error, since the button family moved): no control outside
 *   `src/ui` wears the family's old classes (`ui-button`, `ui-button-primary`, …), whose look
 *   the family now draws from its wrapper's attributes. A screen gives a `Button` a place
 *   (`className` for margin, width, order), never a look.
 * - `purlis/unmigrated-control` (warning, until each family moves): a raw `<button>`,
 *   `<input>`, `<select>` or `<textarea>`, a `role="dialog"` and a native `title=` tooltip.
 *   Each family's ticket moves its call sites and turns its own case into an error.
 *
 * The primitives library itself is refused outside `src/ui` by `no-restricted-imports`, in
 * `eslint.config.js`: one wrapper per primitive, and screens never import it.
 */

/** The button family's old styling classes: `ui-button` and every `ui-button-*` but the row. */
const BUTTON_CLASS = /(?:^|\s)ui-button(?:-(?!row\b)[\w-]+)?(?=\s|$)/;

/** Every string a `className` value can hand on: literals, template text, branches, arrays. */
function* classStrings(node) {
  if (!node) return;
  switch (node.type) {
    case "Literal":
      if (typeof node.value === "string") yield [node, node.value];
      return;
    case "TemplateLiteral":
      for (const quasi of node.quasis) yield [quasi, quasi.value.cooked ?? quasi.value.raw];
      for (const expression of node.expressions) yield* classStrings(expression);
      return;
    case "JSXExpressionContainer":
      yield* classStrings(node.expression);
      return;
    case "ConditionalExpression":
      yield* classStrings(node.consequent);
      yield* classStrings(node.alternate);
      return;
    case "LogicalExpression":
    case "BinaryExpression":
      yield* classStrings(node.left);
      yield* classStrings(node.right);
      return;
    case "ArrayExpression":
      for (const element of node.elements) yield* classStrings(element);
      return;
    case "CallExpression":
      // `clsx(…)`, `[…].join(" ")`
      for (const argument of node.arguments) yield* classStrings(argument);
      yield* classStrings(node.callee.type === "MemberExpression" ? node.callee.object : null);
      return;
    default:
      return;
  }
}

const buttonFamily = {
  meta: {
    type: "problem",
    docs: { description: "A button is the shared Button family, never a hand-styled control" },
    messages: {
      oldClass:
        "`{{word}}` is the button family's old class: draw it as `<Button>`, `<IconButton>` or `<ToggleButton>` from `ui/button/Button` with a `variant` and a `size` (docs/ui-primitives.md).",
    },
    schema: [],
  },
  create(context) {
    return {
      JSXAttribute(node) {
        if (node.name.name !== "className") return;
        for (const [at, text] of classStrings(node.value)) {
          const found = BUTTON_CLASS.exec(text);
          if (found)
            context.report({ node: at, messageId: "oldClass", data: { word: found[0].trim() } });
        }
      },
    };
  },
};

const RAW_CONTROLS = new Set(["button", "input", "select", "textarea"]);

/** The button family's members, which hand a `title` on to their element as a native tooltip. */
const BUTTONS = new Set(["Button", "IconButton", "ToggleButton"]);

const unmigratedControl = {
  meta: {
    type: "suggestion",
    docs: { description: "A control is drawn from its component family in src/ui" },
    messages: {
      raw: "A raw `<{{tag}}>`: draw it from its component family in `src/ui` once that family has moved (spec #2897).",
      dialog:
        '`role="dialog"`: draw it from the dialog family in `src/ui` once that family has moved (spec #2897).',
      title:
        "A native `title=` tooltip: the shared tooltip replaces it once that family has moved (spec #2897).",
    },
    schema: [],
  },
  create(context) {
    return {
      JSXOpeningElement(node) {
        if (node.name.type !== "JSXIdentifier") return;
        const tag = node.name.name;
        const intrinsic = /^[a-z]/.test(tag);
        if (RAW_CONTROLS.has(tag)) context.report({ node, messageId: "raw", data: { tag } });
        for (const attribute of node.attributes) {
          if (attribute.type !== "JSXAttribute") continue;
          const name = attribute.name.name;
          if (
            name === "role" &&
            attribute.value?.type === "Literal" &&
            attribute.value.value === "dialog"
          )
            context.report({ node: attribute, messageId: "dialog" });
          if (name === "title" && (intrinsic || BUTTONS.has(tag)))
            context.report({ node: attribute, messageId: "title" });
        }
      },
    };
  },
};

export const purlisUi = {
  meta: { name: "purlis-ui" },
  rules: { "button-family": buttonFamily, "unmigrated-control": unmigratedControl },
};
