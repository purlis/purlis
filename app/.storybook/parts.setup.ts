// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Every part a story draws carries its classes, and every rule is keyed on them** (#4351; ADR
 * 0103, amended 2026-11-04; `src/ui/partClasses.ts`). After a story has rendered and run its
 * `play` function:
 *
 * - each element with a `data-scope` or a `data-part` carries exactly the `ds-<scope>` and
 *   `dp-<part>` classes they draw, and nothing else carries one: a part without them is drawn
 *   unstyled without a word, and one with another part's is styled as that part;
 * - no rule on the page, the window's own CSS or the theme's parts, still keys a part on an
 *   exact match of its attribute, which would mean the build's PostCSS step or `drawParts` was
 *   passed by.
 *
 * In Chromium only: the DOM and the stylesheets are the same in WebKit, and a second browser
 * would only say each failure twice.
 */
import { afterEach, expect } from "vitest";
import { server } from "vitest/browser";
import { partsWithoutTheirClass } from "../src/ui/partClasses";

/** An exact match on a scope or a part whose value a class can be named from. */
const ON_AN_ATTRIBUTE = /\[data-(?:scope|part)\s*=\s*["']?[A-Za-z0-9_-]+["']?\s*\]/;

/** The selectors of every style rule in `rules`, inside any grouping rule. */
function selectorsIn(rules: CSSRuleList, into: string[] = []): string[] {
  for (const rule of rules) {
    if (rule instanceof CSSStyleRule) into.push(rule.selectorText);
    if ("cssRules" in rule) selectorsIn((rule as CSSGroupingRule).cssRules, into);
  }
  return into;
}

afterEach((context) => {
  if (server.browser !== "chromium" || context.task.result?.state === "fail") return;
  expect(partsWithoutTheirClass(document.body), "parts without their classes").toEqual([]);
  const keyed = [...document.styleSheets]
    .flatMap((sheet) => selectorsIn(sheet.cssRules))
    .filter((selector) => ON_AN_ATTRIBUTE.test(selector));
  expect(keyed, "rules still keyed on a part's attribute (src/ui/partClasses.ts)").toEqual([]);
});
