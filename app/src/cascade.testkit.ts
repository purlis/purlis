// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * Test-only: **what `App.css` and the control primitives draw an element with, resolved the way a
 * browser would** (#1672, #1834).
 *
 * jsdom applies a stylesheet in source order and ignores specificity, and it never matches
 * `:hover` or `:focus-visible`. A test that asks it whether a selected row looks different from a
 * hovered one gets the same answer for both. So this reads `App.css` as text (as
 * `selection.test.ts` and `StripMarks.test.tsx` do) and runs the cascade itself, for one
 * property at a time:
 *
 * - every top-level rule whose selector matches the element counts. A rule inside an at-rule
 *   (`@media`, `@container`, `@supports`) does not: what is answered is the window as it is
 *   drawn at its ordinary width, with motion and no narrowing query in force;
 * - a selector that names a pseudo-element draws something else, and is skipped;
 * - `:hover` and `:focus-visible` match only when the call says the element is in that state;
 * - `primitives/controls.css`, `ui/button/button.css`, `ui/field/field.css`,
 *   `ui/select/select.css`, the window's own
 *   families (`ui/panel`, `ui/empty-state`, `ui/status`), `ui/dialog/dialog.css`,
 *   `primitives/overlays.css`, `ui/tooltip/tooltip.css`, the popover and menu families' sheets and
 *   the list family's are in the `components` layer,
 *   above `App.css`'s `charter` (`styles.css`), so a primitive's rule beats any of `App.css`'s that
 *   sets the same property, at any specificity;
 * - inside one layer, the winner is the highest specificity, then the last in the file.
 *
 * What it answers is the declared value, `var(--list-selected)` and not a colour: two rows that
 * read two different tokens are drawn in two different colours by construction, since
 * `contrast.test.ts` holds the tokens apart.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";

/** A stylesheet under `src/`, without its comments. */
function sheet(file: string): string {
  return readFileSync(join(process.cwd(), "src", file), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
}

interface Rule {
  selectors: string[];
  declarations: Map<string, string>;
  /** The rule's layer: its place in `SHEETS`. A later layer wins at any specificity. */
  layer: number;
  order: number;
}

/** A selector list split at its top-level commas: `:is(a, b), c` is two selectors, not three. */
function splitList(list: string): string[] {
  const out: string[] = [];
  let depth = 0;
  let from = 0;
  for (let at = 0; at < list.length; at += 1) {
    const char = list[at];
    if (char === "(" || char === "[") depth += 1;
    else if (char === ")" || char === "]") depth -= 1;
    else if (char === "," && depth === 0) {
      out.push(list.slice(from, at).trim());
      from = at + 1;
    }
  }
  out.push(list.slice(from).trim());
  return out.filter((one) => one !== "");
}

/** The stylesheet with every at-rule's block taken out, braces and all. */
function topLevel(css: string): string {
  let out = "";
  let at = 0;
  while (at < css.length) {
    const start = css.indexOf("@", at);
    if (start < 0) break;
    out += css.slice(at, start);
    const open = css.indexOf("{", start);
    const semi = css.indexOf(";", start);
    // `@import …;` and the like have no block.
    if (open < 0 || (semi >= 0 && semi < open)) {
      at = semi < 0 ? css.length : semi + 1;
      continue;
    }
    let depth = 0;
    let end = open;
    for (; end < css.length; end += 1) {
      if (css[end] === "{") depth += 1;
      else if (css[end] === "}") {
        depth -= 1;
        if (depth === 0) break;
      }
    }
    at = end + 1;
  }
  return out + css.slice(at);
}

/** `css`'s rules in `layer`; `from` is where they start in that layer's order, for a second sheet
 *  imported into the same layer after a first. */
function rulesOf(css: string, layer = 0, from = 0): Rule[] {
  return [...topLevel(css).matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((hit, at) => {
    const order = from + at;
    const declarations = new Map<string, string>();
    for (const line of hit[2].split(";")) {
      const colon = line.indexOf(":");
      if (colon < 0) continue;
      declarations.set(line.slice(0, colon).trim(), line.slice(colon + 1).trim());
    }
    return { selectors: splitList(hit[1].trim()), declarations, layer, order };
  });
}

/** The window's cascade: `App.css`, and in the layer above it the collections, the control
 *  classes, the button family (#2904), the field family (#2905), the select and combobox families
 *  (#2906), the window's own families
 *  (#2913), the harness icon (#2922), the dialog family (#2910), the overlay surface, the tooltip family (#2908), the popover and menu families (#2909), the list family (#2911), a chat's pane (#3142) and then the text roles,
 *  in the order `styles.css` imports them. */
const COMPONENTS = [
  "primitives/collections.css",
  "primitives/controls.css",
  "ui/button/button.css",
  "ui/field/field.css",
  "ui/select/select.css",
  "ui/panel/panel.css",
  "ui/empty-state/empty-state.css",
  "ui/status/status.css",
  "ui/harness-icon/harness-icon.css",
  "ui/dialog/dialog.css",
  "primitives/overlays.css",
  "ui/tooltip/tooltip.css",
  "ui/popover/popover.css",
  "ui/menu/menu.css",
  "ui/list/list.css",
  "paneFrame.css",
  "primitives/text.css",
].reduce<Rule[]>((rules, file) => [...rules, ...rulesOf(sheet(file), 1, rules.length)], []);
const RULES = [...rulesOf(sheet("App.css"), 0), ...COMPONENTS];

/** `[ids, classes + attributes + pseudo-classes, types + pseudo-elements]`. */
type Specificity = [number, number, number];

const add = (a: Specificity, b: Specificity): Specificity => [
  a[0] + b[0],
  a[1] + b[1],
  a[2] + b[2],
];
const more = (a: Specificity, b: Specificity) =>
  a[0] !== b[0] ? a[0] > b[0] : a[1] !== b[1] ? a[1] > b[1] : a[2] > b[2];

/** The argument of the function whose `(` is at `open`, and the index just past its `)`. */
function argumentAt(selector: string, open: number): [string, number] {
  let depth = 0;
  for (let at = open; at < selector.length; at += 1) {
    if (selector[at] === "(") depth += 1;
    else if (selector[at] === ")") {
      depth -= 1;
      if (depth === 0) return [selector.slice(open + 1, at), at + 1];
    }
  }
  return [selector.slice(open + 1), selector.length];
}

/** Selectors Level 4's specificity: `:where()` counts nothing, `:is()`, `:not()` and `:has()`
 *  count their most specific argument. */
export function specificity(selector: string): Specificity {
  let total: Specificity = [0, 0, 0];
  let at = 0;
  while (at < selector.length) {
    const char = selector[at];
    if (char === "#") {
      total = add(total, [1, 0, 0]);
      at = skipName(selector, at + 1);
    } else if (char === ".") {
      total = add(total, [0, 1, 0]);
      at = skipName(selector, at + 1);
    } else if (char === "[") {
      total = add(total, [0, 1, 0]);
      at = selector.indexOf("]", at) + 1;
    } else if (char === ":" && selector[at + 1] === ":") {
      total = add(total, [0, 0, 1]);
      at = skipName(selector, at + 2);
    } else if (char === ":") {
      const end = skipName(selector, at + 1);
      const name = selector.slice(at + 1, end);
      if (selector[end] === "(") {
        const [argument, past] = argumentAt(selector, end);
        if (["is", "not", "has"].includes(name)) {
          const best = splitList(argument)
            .map(specificity)
            .reduce<Specificity>((a, b) => (more(b, a) ? b : a), [0, 0, 0]);
          total = add(total, best);
        } else if (name !== "where") total = add(total, [0, 1, 0]);
        at = past;
      } else {
        total = add(total, [0, 1, 0]);
        at = end;
      }
    } else if (/[a-zA-Z]/.test(char)) {
      total = add(total, [0, 0, 1]);
      at = skipName(selector, at);
    } else at += 1;
  }
  return total;
}

function skipName(selector: string, from: number): number {
  let at = from;
  while (at < selector.length && /[\w-]/.test(selector[at])) at += 1;
  return at;
}

/** The states a person puts an element in, which jsdom never matches. */
export interface Drawn {
  hover?: boolean;
  focusVisible?: boolean;
}

/** `selector` as jsdom can match it for an element in `state`: a pseudo-class it cannot match is
 *  an attribute the call puts on the element for the length of the match, and so is the state
 *  attribute React Aria writes for it (`data-hovered`, `data-focus-visible`). */
function asMatched(selector: string, state: Drawn): string {
  return selector
    .replace(/:hover\b|\[data-hovered\]/g, state.hover ? "[data-cascade-hover]" : ":not(*)")
    .replace(
      /:focus-visible\b|\[data-focus-visible\]/g,
      state.focusVisible ? "[data-cascade-focus]" : ":not(*)",
    );
}

/**
 * What the window's stylesheets set `property` to on `element` in `state`, by the cascade: the declared value
 * of the winning rule, or `undefined` when no rule sets it. `background` is also answered by a
 * `background-color` declaration, whichever wins.
 */
export function drawnWith(
  element: Element,
  property: string,
  state: Drawn = {},
): string | undefined {
  return resolve(RULES, element, property, state);
}

/**
 * `drawnWith` for another stylesheet under `src/`, read once: a primitive's own sheet
 * (`primitives/collections.css`), answered from that sheet's rules alone.
 */
export function cascadeOf(
  file: string,
): (element: Element, property: string, state?: Drawn) => string | undefined {
  const rules = rulesOf(sheet(file));
  return (element, property, state = {}) => resolve(rules, element, property, state);
}

function resolve(
  rules: Rule[],
  element: Element,
  property: string,
  state: Drawn,
): string | undefined {
  const names = property === "background" ? ["background", "background-color"] : [property];
  if (state.hover) element.setAttribute("data-cascade-hover", "");
  if (state.focusVisible) element.setAttribute("data-cascade-focus", "");
  try {
    let best: { value: string; layer: number; weight: Specificity; order: number } | undefined;
    for (const rule of rules) {
      const name = names.find((one) => rule.declarations.has(one));
      if (name === undefined) continue;
      for (const selector of rule.selectors) {
        if (selector.includes("::")) continue;
        let matches = false;
        try {
          matches = element.matches(asMatched(selector, state));
        } catch {
          continue;
        }
        if (!matches) continue;
        const weight = specificity(selector);
        if (
          best === undefined ||
          rule.layer > best.layer ||
          (rule.layer === best.layer &&
            (more(weight, best.weight) || (!more(best.weight, weight) && rule.order >= best.order)))
        )
          best = {
            value: rule.declarations.get(name) ?? "",
            layer: rule.layer,
            weight,
            order: rule.order,
          };
      }
    }
    return best?.value;
  } finally {
    element.removeAttribute("data-cascade-hover");
    element.removeAttribute("data-cascade-focus");
  }
}
