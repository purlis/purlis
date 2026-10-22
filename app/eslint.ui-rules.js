// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The component layer's lint** (#2904, spec #2897, ADR 0104): a screen draws a control from
 * its family in `src/ui/<family>`, never by hand.
 *
 * **One rule per family**, so each family's ticket turns its own rule into an error as it moves
 * every call site (ADR 0104 §5-6):
 *
 * - `purlis/button-family` (error: the button family has moved): a raw `<button>` is a control
 *   of another family that has not moved yet, said by its role or its class (`otherFamilies`,
 *   set in `eslint.config.js`), or it is an error; and no control outside `src/ui` wears the
 *   family's old classes (`ui-button`, `ui-button-primary`, `ui-button-row`, …), whose look the
 *   family draws from its wrapper's attributes. A screen gives a `Button` a place (`className`
 *   for margin, width, order), never a look.
 * - `purlis/field-family` (error: the field family has moved, #2905): a raw `<textarea>`, or a
 *   raw `<input>` a person types in (no `type`, or `text`, `search`, `password`, `url`, `email`,
 *   `tel`, `number`, or a `type` decided at run time), is the family's `TextField` or `TextArea`
 *   (`ui/field/Field`); and no control wears the family's old classes (`ui-input`,
 *   `ui-input-lead`, `ui-list-field`, …).
 * - `purlis/select-family` (warning until the select and combobox families move): a raw
 *   `<select>`, or an `<input role="combobox">` (the palette's), which is the combobox's and not
 *   the field family's.
 * - `purlis/choice-family` (warning until the choice families move): a raw checkbox, radio,
 *   slider (`range`) or colour well (`color`) `<input>`.
 * - `purlis/dialog-family` (warning until the dialog family moves): a `role="dialog"`.
 * - `purlis/tooltip-family` (warning until the tooltip family moves): a native `title=`.
 *
 * The primitives library itself is refused outside `src/ui` by `no-restricted-imports`, in
 * `eslint.config.js`: one wrapper per primitive, and screens never import it.
 */

/** The button family's old styling classes: `ui-button` and every `ui-button-*`. */
const BUTTON_CLASS = /(?:^|\s)ui-button(?:-[\w-]+)?(?=\s|$)/;

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

/** A JSX attribute of `node` by name. */
const attributeOf = (node, name) =>
  node.attributes.find((one) => one.type === "JSXAttribute" && one.name.name === name);

/** The words every string of a `className` can hold. */
const classWordsOf = (attribute) =>
  [...classStrings(attribute?.value)].flatMap(([, text]) => text.split(/\s+/)).filter(Boolean);

/** The button family's members, which hand a `title` on to their element as a native tooltip. */
const BUTTONS = new Set(["Button", "IconButton", "ToggleButton"]);

const buttonFamily = {
  meta: {
    type: "problem",
    docs: { description: "A button is the shared Button family, never a hand-drawn control" },
    messages: {
      oldClass:
        "`{{word}}` is the button family's old class: draw it as `<Button>`, `<IconButton>`, `<ToggleButton>` or `<ButtonRow>` from `ui/button/Button` with a `variant` and a `size` (docs/ui-primitives.md).",
      raw: "A raw `<button>` is the button family's `<Button>`, `<IconButton>` or `<ToggleButton>` (`ui/button/Button`), unless it is another family's control: say which by its role or its family's class (`otherFamilies` in eslint.config.js).",
    },
    schema: [
      {
        type: "object",
        properties: {
          otherFamilies: {
            type: "object",
            properties: {
              roles: { type: "array", items: { type: "string" } },
              classes: { type: "array", items: { type: "string" } },
            },
            additionalProperties: false,
          },
        },
        additionalProperties: false,
      },
    ],
  },
  create(context) {
    const { roles = [], classes = [] } = context.options[0]?.otherFamilies ?? {};
    return {
      JSXAttribute(node) {
        if (node.name.name !== "className") return;
        for (const [at, text] of classStrings(node.value)) {
          const found = BUTTON_CLASS.exec(text);
          if (found)
            context.report({ node: at, messageId: "oldClass", data: { word: found[0].trim() } });
        }
      },
      JSXOpeningElement(node) {
        if (node.name.type !== "JSXIdentifier" || node.name.name !== "button") return;
        const role = attributeOf(node, "role")?.value;
        if (role?.type === "Literal" && roles.includes(role.value)) return;
        if (classWordsOf(attributeOf(node, "className")).some((word) => classes.includes(word)))
          return;
        context.report({ node, messageId: "raw" });
      },
    };
  },
};

/** A rule that warns of one family's raw control until that family moves. */
function unmoved({ description, message, test }) {
  return {
    meta: { type: "suggestion", docs: { description }, messages: { unmoved: message }, schema: [] },
    create(context) {
      return {
        JSXOpeningElement(node) {
          if (node.name.type !== "JSXIdentifier") return;
          for (const at of test(node)) context.report({ node: at, messageId: "unmoved" });
        },
      };
    },
  };
}

/** The field family's old styling classes: `ui-input`, every `ui-input-*`, `ui-list-field`. */
const FIELD_CLASS = /(?:^|\s)(ui-input(?:-[\w-]+)?|ui-list-field)(?=\s|$)/;

/** The kinds of `<input>` another family draws: a choice, a slider, a colour well. */
const CHOICES = new Set(["checkbox", "radio", "range", "color", "file"]);

/** An `<input>`'s `type` as written, `null` for one decided at run time, `""` for none. */
function typeOf(node) {
  const type = attributeOf(node, "type")?.value;
  if (!type) return "";
  return type.type === "Literal" && typeof type.value === "string" ? type.value : null;
}

const fieldFamily = {
  meta: {
    type: "problem",
    docs: { description: "A box a person types in is the shared field family" },
    messages: {
      raw: "A raw `<{{tag}}>`: draw it as `<TextField>` or `<TextArea>` from `ui/field/Field`, with its label, help and validation (docs/ui-primitives.md).",
      oldClass:
        "`{{word}}` is the field family's old class: draw the box as `<TextField>` or `<TextArea>` from `ui/field/Field` (docs/ui-primitives.md).",
    },
    schema: [],
  },
  create(context) {
    return {
      JSXAttribute(node) {
        if (node.name.name !== "className") return;
        for (const [at, text] of classStrings(node.value)) {
          const found = FIELD_CLASS.exec(text);
          if (found) context.report({ node: at, messageId: "oldClass", data: { word: found[1] } });
        }
      },
      JSXOpeningElement(node) {
        if (node.name.type !== "JSXIdentifier") return;
        const tag = node.name.name;
        if (tag === "textarea") context.report({ node, messageId: "raw", data: { tag } });
        if (tag !== "input") return;
        const type = typeOf(node);
        if (type === "hidden" || (type !== null && CHOICES.has(type))) return;
        if (combobox(node)) return;
        context.report({ node, messageId: "raw", data: { tag } });
      },
    };
  },
};

/** Whether an `<input>` is a combobox's, by its role. */
function combobox(node) {
  const role = attributeOf(node, "role")?.value;
  return role?.type === "Literal" && role.value === "combobox";
}

const selectFamily = unmoved({
  description: "A select or a combobox is drawn from its family in src/ui once it moves",
  message:
    "A raw `<select>` or combobox: draw it from its family in `src/ui` once that family has moved (spec #2897).",
  test: (node) =>
    node.name.name === "select" || (node.name.name === "input" && combobox(node)) ? [node] : [],
});

const choiceFamily = unmoved({
  description: "A checkbox, radio, slider or colour well is drawn from its family once it moves",
  message:
    "A raw choice `<input>`: draw it from its family in `src/ui` once that family has moved (spec #2897).",
  test: (node) => {
    if (node.name.name !== "input") return [];
    const type = typeOf(node);
    return type !== null && type !== "hidden" && CHOICES.has(type) ? [node] : [];
  },
});

const dialogFamily = unmoved({
  description: "A dialog is drawn from the dialog family in src/ui once it moves",
  message:
    '`role="dialog"`: draw it from the dialog family in `src/ui` once that family has moved (spec #2897).',
  test: (node) => {
    const role = attributeOf(node, "role");
    return role?.value?.type === "Literal" && role.value.value === "dialog" ? [role] : [];
  },
});

const tooltipFamily = unmoved({
  description: "A tooltip is the shared tooltip once that family moves",
  message:
    "A native `title=` tooltip: the shared tooltip replaces it once that family has moved (spec #2897).",
  test: (node) => {
    const tag = node.name.name;
    const title = attributeOf(node, "title");
    return title && (/^[a-z]/.test(tag) || BUTTONS.has(tag)) ? [title] : [];
  },
});

export const purlisUi = {
  meta: { name: "purlis-ui" },
  rules: {
    "button-family": buttonFamily,
    "field-family": fieldFamily,
    "select-family": selectFamily,
    "choice-family": choiceFamily,
    "dialog-family": dialogFamily,
    "tooltip-family": tooltipFamily,
  },
};
