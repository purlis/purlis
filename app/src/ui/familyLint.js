// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What every family's lint rule is made of** (#2904, #3387): the readers of a JSX element's
 * attributes, and the rule most families' own is built from (`moved`). Each family's
 * rule is in its own folder, `src/ui/<family>/lint.js`, and `eslint.ui-rules.js` finds it there.
 */

/** Every string a `className` value can hand on: literals, template text, branches, arrays. */
export function* classStrings(node) {
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
export const attributeOf = (node, name) =>
  node.attributes.find((one) => one.type === "JSXAttribute" && one.name.name === name);

/** The words every string of a `className` can hold. */
export const classWordsOf = (attribute) =>
  [...classStrings(attribute?.value)].flatMap(([, text]) => text.split(/\s+/)).filter(Boolean);

/** The kinds of `<input>` another family draws: a choice, a slider, a colour well. */
export const CHOICES = new Set(["checkbox", "radio", "range", "color", "file"]);

/** An `<input>`'s `type` as written, `null` for one decided at run time, `""` for none. */
export function typeOf(node) {
  const type = attributeOf(node, "type")?.value;
  if (!type) return "";
  return type.type === "Literal" && typeof type.value === "string" ? type.value : null;
}

/** The string an attribute's value is, written as a literal, in braces or as a template with
 *  nothing in it; `undefined` for one decided at run time. */
export function literalOf(value) {
  if (!value) return undefined;
  if (value.type === "Literal") return value.value;
  if (value.type !== "JSXExpressionContainer") return undefined;
  const inner = value.expression;
  if (inner.type === "Literal") return inner.value;
  if (inner.type === "TemplateLiteral" && inner.expressions.length === 0)
    return inner.quasis[0]?.value.cooked;
  return undefined;
}

/**
 * **A family that has moved** (#2913): its old styling classes are an error outside `src/ui`, and
 * so is its `data-scope` written by hand, since only its wrapper draws it. `classes` are whole
 * words of a `className`, or a prefix where one ends in `*`. A collection family (#2911) also
 * refuses its `roles` written by hand and its raw `elements` (`<table>`), which only its wrapper
 * writes.
 */
export function moved({ description, scopes, classes, message, roles = [], elements = [] }) {
  const named = (word) =>
    classes.some((one) => (one.endsWith("*") ? word.startsWith(one.slice(0, -1)) : word === one));
  return {
    meta: {
      type: "problem",
      docs: { description },
      messages: {
        oldClass: `\`{{word}}\` is the old class of ${message}`,
        scope: `\`data-scope="{{scope}}"\` is drawn by its family's wrapper only: ${message}`,
        role: `\`role="{{role}}"\` by hand is ${message}`,
        element: `A raw \`<{{element}}>\` is ${message}`,
      },
      schema: [],
    },
    create(context) {
      return {
        JSXOpeningElement(node) {
          if (node.name.type === "JSXIdentifier" && elements.includes(node.name.name))
            context.report({ node, messageId: "element", data: { element: node.name.name } });
          // A role written as a string, in quotes or in braces (`role={"listbox"}`, a branch).
          const role = attributeOf(node, "role");
          for (const [, said] of classStrings(role?.value))
            if (roles.includes(said.trim()))
              context.report({ node: role, messageId: "role", data: { role: said.trim() } });
        },
        JSXAttribute(node) {
          if (node.name.name === "className") {
            for (const [at, text] of classStrings(node.value))
              for (const word of text.split(/\s+/).filter(Boolean))
                if (named(word))
                  context.report({ node: at, messageId: "oldClass", data: { word } });
            return;
          }
          if (
            node.name.name === "data-scope" &&
            node.value?.type === "Literal" &&
            scopes.includes(node.value.value)
          )
            context.report({ node, messageId: "scope", data: { scope: node.value.value } });
        },
      };
    },
  };
}
