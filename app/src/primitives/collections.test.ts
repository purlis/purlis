// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The shared list row** (DS-3, #1835): a row's height comes from the core tokens, and the
 * selected row is drawn selected and no other is. The table and the tree are their families'
 * (`ui/table`, `ui/tree`, #2911, #3198).
 *
 * vitest does not apply stylesheets, so this reads `collections.css` off disk. What a rule draws
 * is asked of the DOM itself: the markup a view would write is put in the document and
 * `cascade.testkit.ts` runs the cascade over this sheet's rules, specificity, `:hover` and all,
 * so a selector that stops reaching its element fails here, not in someone's screenshot.
 */

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { cascadeOf } from "../cascade.testkit";
import { isGeometry } from "../theme/geometry";
import { property, TOKENS } from "../theme/theme";

const SRC = join(process.cwd(), "src");

function read(file: string): string {
  const text = readFileSync(join(SRC, file), "utf8");
  expect(text.length, `${file} read as empty`).toBeGreaterThan(0);
  return text.replace(/\/\*[\s\S]*?\*\//g, "");
}

type Rule = { selector: string; body: string };

/** Every rule, in the order the file writes them. */
function rules(css: string): Rule[] {
  return [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)]
    .map(([, selector, body]) => ({ selector: selector.trim().replace(/\s+/g, " "), body }))
    .filter(({ selector }) => !selector.startsWith("@"));
}

/** `text` as a pattern that matches exactly it: what a pattern reads as syntax is escaped. */
function literally(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/** A declaration's value in a rule's body, or undefined. */
function value(body: string, name: string): string | undefined {
  const hit = new RegExp(`(?:^|[;\\s])${literally(name)}\\s*:\\s*([^;]+);`).exec(body);
  return hit?.[1].trim().replace(/\s+/g, " ");
}

const css = read("primitives/collections.css");

const all = rules(css);

/** The rule with exactly this selector. */
function rule(selector: string): Rule {
  const found = all.find((one) => one.selector === selector);
  expect(found, `no rule ${selector}`).toBeDefined();
  return found as Rule;
}

/** What this sheet sets a property to on an element, by the cascade (`cascade.testkit.ts`). */
const resolved = cascadeOf("primitives/collections.css");

function draw(html: string): HTMLElement {
  document.body.innerHTML = html;
  return document.body;
}

afterEach(() => {
  document.body.innerHTML = "";
});

describe("the collections stylesheet", () => {
  it("is above App.css: imported into the components layer, which comes after charter's", () => {
    const styles = read("styles.css");
    // Spelled in two halves: the stylesheet guard reads a quoted path ending in the extension,
    // anywhere in a source file, as an import (`literals.test.ts`).
    expect(styles).toContain(
      ['@import "./primitives/collections', 'css" layer(components);'].join("."),
    );
    const order = (/@layer ([\w\s,-]+);/.exec(styles)?.[1] ?? "").split(/\s*,\s*/);
    expect(order.indexOf("components")).toBeGreaterThan(order.indexOf("charter"));
  });

  it("reads only tokens: a theme's colours, the core geometry, and its own --ui-* aliases", () => {
    const colours = new Set(TOKENS.map(property));
    const own = new Set([...css.matchAll(/(--ui(?:-\w+)+)\s*:/g)].map((hit) => hit[1]));
    const read = [...css.matchAll(/var\((--[\w-]+)\)/g)].map((hit) => hit[1]);
    expect(read.length).toBeGreaterThan(0);
    expect(
      read.filter((name) => !colours.has(name) && !isGeometry(name) && !own.has(name)),
    ).toEqual([]);
  });

  it("writes no length of its own but a hairline", () => {
    // Every size is a token or an alias of one; 1px is a border or a guide's width. The table,
    // and the one room it stacks under, are the table family's (`ui/table`, #2911).
    const lengths = [...css.matchAll(/(?<![\w-])(\d*\.?\d+)(px|rem|em)\b/g)]
      .map((hit) => hit[0])
      .filter((one) => one !== "1px");
    expect(lengths).toEqual([]);
  });

  it("gives every component token a core token's value", () => {
    const aliases = [...css.matchAll(/(--ui(?:-\w+)+)\s*:\s*([^;]+);/g)]
      // The depth is a number per level, and the depth drawn is worked out from it.
      .filter(([, name]) => name !== "--ui-tree-depth" && name !== "--ui-tree-drawn")
      .map(([, name, set]) => [name, set.trim()]);
    expect(aliases.length).toBeGreaterThan(0);
    for (const [name, set] of aliases) {
      const core = /^var\((--[\w-]+)\)$/.exec(set)?.[1];
      expect(core && isGeometry(core), `${name}: ${set}`).toBe(true);
    }
  });
});

describe("the list row", () => {
  it("is one height, the chrome's, with its padding from the spacing scale", () => {
    const row = rule(".ui-row").body;
    expect(value(row, "--ui-row-height")).toBe("var(--size-chrome)");
    expect(value(row, "min-block-size")).toBe("var(--ui-row-height)");
    expect(value(row, "--ui-row-pad")).toBe("var(--space-4)");
    expect(value(row, "padding-inline")).toBe("var(--ui-row-pad)");
    expect(value(row, "padding-block")).toBe("0");
    expect(value(row, "gap")).toBe("var(--ui-row-gap)");
    expect(value(row, "--ui-row-gap")).toBe("var(--space-3)");
  });

  it("has a lead, a label cut to one line, and a trail at the row's end", () => {
    const body = draw(`
      <ul class="ui-list">
        <li class="ui-row">
          <span class="ui-row-lead"></span><span class="ui-row-label">a</span>
          <span class="ui-row-trail"></span>
        </li>
      </ul>`);
    const label = body.querySelector(".ui-row-label") as Element;
    expect(resolved(label, "white-space")).toBe("nowrap");
    expect(resolved(label, "text-overflow")).toBe("ellipsis");
    expect(resolved(label, "min-inline-size")).toBe("0");
    expect(resolved(body.querySelector(".ui-row-trail") as Element, "margin-inline-start")).toBe(
      "auto",
    );
    expect(resolved(body.querySelector(".ui-row-lead") as Element, "flex")).toBe("none");
  });

  it.each([
    ['aria-selected="true"', true],
    ['aria-current="true"', true],
    ['aria-current="page"', true],
    ['aria-selected="false"', false],
    ['aria-current="false"', false],
    ["", false],
  ])("with %s is drawn selected: %s", (attribute, selected) => {
    const body = draw(`<ul class="ui-list"><li class="ui-row" ${attribute}>a</li></ul>`);
    const row = body.querySelector(".ui-row") as Element;
    expect(resolved(row, "background-color")).toBe(
      selected ? "var(--list-selected)" : "transparent",
    );
    expect(resolved(row, "box-shadow")).toBe(
      selected ? "inset var(--focus-ring-width) 0 0 var(--list-selected-edge)" : undefined,
    );
  });

  it("keeps the selection under the pointer, and draws the pointer on any other row", () => {
    const body = draw(`
      <ul class="ui-list">
        <li class="ui-row" aria-selected="true" id="selected">a</li>
        <li class="ui-row" id="plain">b</li>
      </ul>`);
    const row = (id: string) => body.querySelector(`#${id}`) as Element;
    expect(resolved(row("selected"), "background-color", { hover: true })).toBe(
      "var(--list-selected)",
    );
    expect(resolved(row("plain"), "background-color", { hover: true })).toBe(
      "var(--surface-hover)",
    );
  });

  it("draws the keyboard's place as the inside focus ring, and nothing else", () => {
    const body = draw(`<ul class="ui-list"><li class="ui-row" tabindex="0">a</li></ul>`);
    const row = body.querySelector("li") as Element;
    const focused = { focusVisible: true };
    expect(resolved(row, "outline", focused)).toBe(
      "var(--focus-ring-width) solid var(--focus-ring)",
    );
    expect(resolved(row, "outline-offset", focused)).toBe("var(--focus-ring-inset)");
    // A focused row is not a selected one.
    expect(resolved(row, "background-color", focused)).toBe("transparent");
    expect(resolved(row, "outline")).toBeUndefined();
  });

  it("drops a button's bezel when the row is a button", () => {
    const body = draw(`<div class="ui-list"><button class="ui-row">a</button></div>`);
    const row = body.querySelector("button") as Element;
    expect(resolved(row, "appearance")).toBe("none");
    expect(resolved(row, "border")).toBe("0");
    expect(resolved(row, "inline-size")).toBe("100%");
  });
});

describe("docs/ui-primitives.md", () => {
  const doc = readFileSync(join(process.cwd(), "..", "docs", "ui-primitives.md"), "utf8");
  it.each(["ui-list", "ui-row", "ui-row-lead", "ui-row-label", "ui-row-trail"])(
    "documents .%s",
    (name) => {
      expect(new RegExp(`\\.${name}\\b(?!-)`).test(doc), `.${name} is not in the doc`).toBe(true);
    },
  );
});
