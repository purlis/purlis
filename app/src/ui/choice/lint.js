// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The choice family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/choice-family` (error: the choice families have moved, #2907): a raw checkbox,
 * radio, slider (`range`), colour well (`color`) or file `<input>`; a `role` only a choice says
 * (`checkbox`, `radio`, `radiogroup`, `switch`, `slider`) written by hand, as words or as an
 * expression whose words are known, and an `aria-checked` written by hand; Radix's checkbox,
 * radio group, switch, slider and toggles; and the old choice classes (`ui-checkbox`,
 * `ui-radio`, `ui-switch`, `ui-segmented-*`, `ui-levels`, `ui-level`, `ui-choice-option`, …).
 * Each is a member of `ui/choice`: `Checkbox`, `CheckboxGroup`, `RadioGroup`,
 * `SegmentedControl`, `OptionCards`, `Switch`, `Slider` or `ColorWell`.
 */

import { classStrings, CHOICES, typeOf } from "../familyLint.js";

/** The roles a choice says, which only the choice families write. */
const CHOICE_ROLES = new Set(["checkbox", "radio", "radiogroup", "switch", "slider"]);

/** The old choice classes: the checkbox, radio, switch and segmented classes, the settings
 *  set's levels and its choice rows. */
const CHOICE_CLASS =
  /(?:^|\s)(ui-(?:checkbox|radio(?:-mark)?|switch(?:-thumb)?|segmented(?:-[\w-]+)?|levels?(?:-[\w-]+)?|choice-(?:option|says)))(?=\s|$)/;

/** Radix's choices. */
const CHOICE_IMPORT =
  /^@radix-ui\/react-(?:checkbox|radio-group|switch|slider|toggle|toggle-group)$/;

const choiceFamily = {
  meta: {
    type: "problem",
    docs: { description: "A checkbox, radio, switch, segmented control or slider is its family" },
    messages: {
      raw: "A raw choice `<input>`: draw it from its family in `ui/choice` — `Checkbox`, `RadioGroup`, `SegmentedControl`, `OptionCards`, `Switch`, `Slider` or `ColorWell` (docs/ui-primitives.md).",
      role: '`role="{{role}}"` by hand: a choice is drawn from its family in `ui/choice`, which says its roles itself (docs/ui-primitives.md).',
      checked:
        "`aria-checked` by hand: a choice is drawn from its family in `ui/choice`, whose native input says whether it is checked.",
      oldClass:
        "`{{word}}` is an old choice class: the choice families draw themselves from their parts (`ui/choice`).",
      oldImport: "`{{from}}`: a choice is drawn from its family in `ui/choice`.",
    },
    schema: [],
  },
  create(context) {
    return {
      ImportDeclaration(node) {
        if (CHOICE_IMPORT.test(node.source.value))
          context.report({ node, messageId: "oldImport", data: { from: node.source.value } });
      },
      JSXAttribute(node) {
        if (node.name.name === "aria-checked") {
          context.report({ node, messageId: "checked" });
          return;
        }
        if (node.name.name === "role") {
          // A role written as words, or as an expression whose words are known (`{"switch"}`,
          // a branch of two): each is the role it says.
          for (const [, role] of classStrings(node.value))
            if (CHOICE_ROLES.has(role.trim()))
              context.report({ node, messageId: "role", data: { role: role.trim() } });
          return;
        }
        if (node.name.name !== "className") return;
        for (const [at, text] of classStrings(node.value)) {
          const found = CHOICE_CLASS.exec(text);
          if (found) context.report({ node: at, messageId: "oldClass", data: { word: found[1] } });
        }
      },
      JSXOpeningElement(node) {
        if (node.name.type !== "JSXIdentifier" || node.name.name !== "input") return;
        const type = typeOf(node);
        if (type !== null && type !== "hidden" && CHOICES.has(type))
          context.report({ node, messageId: "raw" });
      },
    };
  },
};

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "choice-family": choiceFamily,
};
