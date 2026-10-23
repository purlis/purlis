// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The shared table, list row and tree row** (DS-3, #1835): a row's height and a tree's indent
 * come from the core tokens, the selected row is drawn selected and no other is, a parent's
 * twist is drawn and a leaf's is not, and a table's headings stay on one line.
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

/** The condition a stacking table's room is queried with (#1859). */
const STACKS_UNDER = "@container ui-table (max-width: 29rem)";
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

  it("writes no length of its own but a hairline, and the one room a table stacks under", () => {
    // Every size is a token or an alias of one; 1px is a border or a guide's width. A container
    // query's condition cannot read a token (CSS has no `var()` there), so the width a table
    // stacks under is the one length written, once (#1859).
    expect(css.match(new RegExp(literally(STACKS_UNDER), "g"))).toHaveLength(1);
    const lengths = [
      ...css.replace(STACKS_UNDER, "").matchAll(/(?<![\w-])(\d*\.?\d+)(px|rem|em)\b/g),
    ]
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

describe("the tree row", () => {
  it("steps in by a token, one step per level", () => {
    expect(value(rule(".ui-tree").body, "--ui-tree-step")).toBe("var(--space-5)");
    expect(value(rule(".ui-tree-row").body, "padding-inline-start")).toBe(
      "calc((var(--ui-tree-drawn) + 1) * var(--ui-tree-step))",
    );
  });

  it("draws a row no deeper than the cap a narrow view sets around the tree (DS-11)", () => {
    // The depth drawn is the row's depth, or the view's cap where that is less; with no cap,
    // the deepest drawn is five levels' four steps, as before.
    expect(value(rule(".ui-tree-row").body, "--ui-tree-drawn")).toBe(
      "min(var(--ui-tree-depth), var(--ui-tree-depth-cap, 4))",
    );
    // The cap is never set by this sheet, so a view's, set on an element around the tree,
    // reaches every row by inheritance and contends with no rule here.
    expect(css).not.toMatch(/--ui-tree-depth-cap\s*:/);
    // The guides and the twist are drawn at the depth drawn, not the level's.
    expect(value(rule(".ui-tree-row").body, "background-size")).toBe(
      "calc(var(--ui-tree-drawn) * var(--ui-tree-step)) 100%",
    );
    expect(value(rule(".ui-tree-twist").body, "inset-inline-start")).toBe(
      "calc(var(--ui-tree-drawn) * var(--ui-tree-step))",
    );
  });

  it.each([
    [1, "0"],
    [2, "1"],
    [3, "2"],
    [4, "3"],
    [5, "4"],
    [9, "4"],
  ])("puts a row at level %i at depth %s, flat or nested", (level, depth) => {
    const body = draw(`
      <ul class="ui-tree" role="tree">
        <li class="ui-row ui-tree-row" role="treeitem" aria-level="${level}" id="flat">a</li>
        <li class="ui-row ui-tree-row" data-level="${level}" id="data">b</li>
        <li role="treeitem" aria-level="${level}"><div class="ui-row ui-tree-row" id="nested">c</div></li>
      </ul>`);
    for (const id of ["flat", "data"]) {
      expect(resolved(body.querySelector(`#${id}`) as Element, "--ui-tree-depth"), id).toBe(depth);
    }
    // Nested: the level is on the treeitem, and the row inherits the depth it sets.
    const nested = body.querySelector("#nested") as Element;
    expect(resolved(nested, "--ui-tree-depth")).toBeUndefined();
    expect(resolved(nested.parentElement as Element, "--ui-tree-depth")).toBe(depth);
  });

  it("draws the twist only on a row with children, and it takes no room", () => {
    const body = draw(`
      <ul class="ui-tree" role="tree">
        <li class="ui-row ui-tree-row" role="treeitem" aria-level="1" aria-expanded="false" id="parent">
          <button class="ui-tree-twist" tabindex="-1"></button><span class="ui-row-label">p</span>
        </li>
        <li class="ui-row ui-tree-row" role="treeitem" aria-level="1" id="leaf">
          <button class="ui-tree-twist" tabindex="-1"></button><span class="ui-row-label">l</span>
        </li>
        <li role="treeitem" aria-level="1" aria-expanded="true" id="open">
          <div class="ui-row ui-tree-row"><span class="ui-tree-twist"></span>o</div>
          <ul role="group">
            <li role="treeitem" aria-level="2" id="child">
              <div class="ui-row ui-tree-row"><span class="ui-tree-twist"></span>c</div>
            </li>
          </ul>
        </li>
      </ul>`);
    const twist = (id: string) => body.querySelector(`#${id} .ui-tree-twist`) as Element;
    expect(resolved(twist("parent"), "display")).toBe("inline-flex");
    expect(resolved(twist("open"), "display")).toBe("inline-flex");
    expect(resolved(twist("leaf"), "display")).toBe("none");
    // A leaf inside an open parent is still a leaf.
    expect(resolved(twist("child"), "display")).toBe("none");
    // It hangs in the step before the name, so names line up with no empty twist on a leaf.
    expect(resolved(twist("parent"), "position")).toBe("absolute");
    expect(resolved(twist("parent"), "inline-size")).toBe("var(--ui-tree-step)");
    expect(resolved(twist("parent"), "inset-inline-start")).toBe(
      "calc(var(--ui-tree-drawn) * var(--ui-tree-step))",
    );
    expect(resolved(twist("open"), "transform")).toBe("rotate(90deg)");
    expect(resolved(twist("parent"), "transform")).toBeUndefined();
  });

  it("centres the twist on the row's line: a box of its own size, held by its margins (qw358)", () => {
    // The operator's screenshot: every folder's chevron stood above its name. The explorer gave
    // the class to the icon itself, an element with a size of its own, and between two insets
    // such an element keeps its size and sits at the first one. So the box's height is said.
    const body = draw(`
      <ul class="ui-tree" role="tree">
        <li class="ui-row ui-tree-row" role="treeitem" aria-level="1" aria-expanded="false" id="boxed">
          <span class="ui-tree-twist"><svg></svg></span><span class="ui-row-label">p</span>
        </li>
        <li class="ui-row ui-tree-row" role="treeitem" aria-level="3" aria-expanded="true" id="bare">
          <svg class="ui-tree-twist"></svg><span class="ui-row-label">q</span>
        </li>
      </ul>`);
    for (const id of ["boxed", "bare"]) {
      const row = body.querySelector(`#${id}`) as Element;
      const twist = row.querySelector(".ui-tree-twist") as Element;
      // The row is one flex line with its parts centred on it.
      expect(resolved(row, "display"), id).toBe("flex");
      expect(resolved(row, "align-items"), id).toBe("center");
      // The twist: one step wide and one row high, both tokens, centred between the row's edges.
      expect(resolved(twist, "inline-size"), id).toBe("var(--ui-tree-step)");
      expect(resolved(twist, "block-size"), id).toBe("var(--ui-row-height)");
      expect(resolved(twist, "inset-block"), id).toBe("0");
      expect(resolved(twist, "margin-block"), id).toBe("auto");
      // And what it holds is in its middle, both ways.
      expect(resolved(twist, "align-items"), id).toBe("center");
      expect(resolved(twist, "justify-content"), id).toBe("center");
    }
    // The chevron in the box is one step square, whatever the text's size beside it.
    const icon = body.querySelector("#boxed .ui-tree-twist > svg") as Element;
    expect(resolved(icon, "inline-size")).toBe("var(--ui-tree-step)");
    expect(resolved(icon, "block-size")).toBe("var(--ui-tree-step)");
    // Nothing nudges it: no baseline, and no offset written by hand.
    for (const one of all.filter(({ selector }) => selector.includes(".ui-tree-twist"))) {
      expect(one.body, one.selector).not.toMatch(/vertical-align|translate|(?:^|[;\s])top\s*:/);
      expect(value(one.body, "margin-block-start"), one.selector).toBeUndefined();
    }
    expect(value(rule(".ui-row").body, "--ui-row-height")).toBe("var(--size-chrome)");
  });

  it("is a list row too: one height and one selection", () => {
    const body = draw(`
      <ul class="ui-tree" role="tree">
        <li class="ui-row ui-tree-row" role="treeitem" aria-level="2" aria-selected="true">a</li>
      </ul>`);
    const row = body.querySelector("li") as Element;
    expect(resolved(row, "min-block-size")).toBe("var(--ui-row-height)");
    expect(resolved(row, "background-color")).toBe("var(--list-selected)");
    expect(resolved(row, "padding-inline-start")).toBe(
      "calc((var(--ui-tree-drawn) + 1) * var(--ui-tree-step))",
    );
  });
});

describe("the table", () => {
  const table = (variant = "") => `
    <table class="ui-table ${variant}">
      <thead><tr>
        <th scope="col">Level</th>
        <th scope="col"><span class="ui-table-tip" data-tooltip-target="">Tasks</span></th>
        <th scope="col" class="ui-cell-actions"><span>Actions</span></th>
      </tr></thead>
      <tbody>
        <tr><th scope="row">Project</th><td class="ui-num">3</td><td class="ui-cell-actions"></td></tr>
        <tr aria-selected="true"><th scope="row">You</th><td class="ui-num">2</td><td></td></tr>
      </tbody>
    </table>`;

  it("keeps every heading on one line, cut with an ellipsis", () => {
    const body = draw(table());
    const headings = [...body.querySelectorAll("thead th")];
    expect(headings).toHaveLength(3);
    for (const heading of headings) {
      expect(resolved(heading, "white-space")).toBe("nowrap");
      expect(resolved(heading, "overflow")).toBe("hidden");
      expect(resolved(heading, "text-overflow")).toBe("ellipsis");
      expect(resolved(heading, "font-size")).toBe("var(--font-size-sm)");
    }
    // A row's heading is a cell, not a column's heading.
    expect(resolved(body.querySelector("tbody th") as Element, "white-space")).toBeUndefined();
  });

  it("marks a heading that has a tooltip", () => {
    const body = draw(table());
    const [plain, titled] = body.querySelectorAll("thead th");
    expect(resolved(titled.querySelector(".ui-table-tip") as Element, "text-decoration")).toBe(
      "underline dotted",
    );
    expect(resolved(plain, "text-decoration")).toBeUndefined();
  });

  it("aligns every cell to the start and the middle, from the spacing scale", () => {
    const body = draw(table());
    for (const cell of body.querySelectorAll("td:not(.ui-num), th")) {
      expect(resolved(cell, "vertical-align")).toBe("middle");
      expect(resolved(cell, "padding-inline")).toBe("var(--ui-cell-pad-inline)");
    }
    expect(value(rule(".ui-table").body, "--ui-cell-pad-inline")).toBe("var(--space-4)");
    expect(value(rule(".ui-table").body, "--ui-cell-pad-block")).toBe("var(--space-2)");
    expect(resolved(body.querySelector(".ui-num") as Element, "text-align")).toBe("end");
  });

  it.each([
    ["", "auto"],
    ["ui-table-fixed", "fixed"],
  ])("lays out %s as %s", (variant, layout) => {
    const body = draw(table(variant));
    expect(resolved(body.querySelector("table") as Element, "table-layout")).toBe(layout);
  });

  it("draws a selected row as a list's selected row", () => {
    const body = draw(table());
    const [plain, selected] = body.querySelectorAll("tbody tr");
    expect(resolved(selected.querySelector("td") as Element, "background-color")).toBe(
      "var(--list-selected)",
    );
    expect(resolved(plain.querySelector("td") as Element, "background-color")).toBeUndefined();
  });
});

/**
 * **A table that stacks into a form where its room is narrow** (#1859): `.ui-table-stacks`, in a
 * `.ui-table-room`. jsdom answers no container query (`cascade.testkit.ts` reads the window at its
 * ordinary width), so the rules are read off the sheet here, and the real WebView's drawing is
 * `settings-limits.e2e.ts`'s range check.
 */
describe("the stacking table", () => {
  /** The rules inside the stacking query. */
  function stacked(): Rule[] {
    const at = css.indexOf(STACKS_UNDER);
    expect(at, "no stacking query").toBeGreaterThan(-1);
    const open = css.indexOf("{", at);
    let depth = 0;
    let end = open;
    for (; end < css.length; end++) {
      if (css[end] === "{") depth++;
      if (css[end] === "}" && --depth === 0) break;
    }
    return rules(css.slice(open + 1, end));
  }
  const inside = (selector: string) => {
    const found = stacked().find((one) => one.selector === selector);
    expect(found, `no stacked rule ${selector}`).toBeDefined();
    return (found as Rule).body;
  };

  it("measures its room, the holder it is drawn in", () => {
    expect(value(rule(".ui-table-room").body, "container")).toBe("ui-table / inline-size");
  });

  it("puts the column headings away and makes each row a grid of its cells, under the row's own heading", () => {
    expect(value(inside(".ui-table-stacks thead"), "display")).toBe("none");
    const row = inside(".ui-table-stacks tbody tr");
    expect(value(row, "display")).toBe("grid");
    expect(value(row, "grid-template-columns")).toMatch(
      /^repeat\(auto-fill, minmax\(calc\(var\(--size-control\) \* \d+\), 1fr\)\)$/,
    );
    expect(value(inside(".ui-table-stacks tbody th"), "grid-column")).toBe("1 / -1");
    expect(value(inside(".ui-table-stacks tbody .ui-cell-actions"), "grid-column")).toBe("1 / -1");
  });

  it("gives a cell across the columns the row's whole width (#1865)", () => {
    expect(value(inside(".ui-table-stacks tbody td[colspan]"), "grid-column")).toBe("1 / -1");
  });

  it("says each cell's column above it, in its whole words, and drops a cell with nothing in it", () => {
    const words = inside(".ui-table-stacks td[data-heading]::before");
    expect(value(words, "content")).toBe("attr(data-heading)");
    expect(value(words, "display")).toBe("block");
    expect(value(inside(".ui-table-stacks td:empty"), "display")).toBe("none");
  });

  it("lets a cell's words wrap, never cutting them", () => {
    const cell = inside(".ui-table-stacks tbody :is(th, td)");
    expect(value(cell, "white-space")).toBe("normal");
    expect(value(cell, "overflow-wrap")).toBe("anywhere");
  });
});

describe("docs/ui-primitives.md", () => {
  const doc = readFileSync(join(process.cwd(), "..", "docs", "ui-primitives.md"), "utf8");
  it.each([
    "ui-table",
    "ui-table-fixed",
    "ui-table-stacks",
    "ui-table-room",
    "ui-num",
    "ui-cell-actions",
    "ui-list",
    "ui-row",
    "ui-row-lead",
    "ui-row-label",
    "ui-row-trail",
    "ui-tree",
    "ui-tree-row",
    "ui-tree-twist",
  ])("documents .%s", (name) => {
    expect(new RegExp(`\\.${name}\\b(?!-)`).test(doc), `.${name} is not in the doc`).toBe(true);
  });
});
