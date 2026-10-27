// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The table family's stylesheet** (#2911, after DS-3 #1835 and #1859): it reads only tokens,
 * writes one length (the room a table of controls stacks under), keeps a column's name on one
 * line, lines its cells up from the spacing scale, draws a chosen or current row as chosen, and
 * stacks a table of controls into a form where its frame is narrow.
 *
 * vitest applies no stylesheet, so what a rule draws is asked of the cascade over the window's
 * sheets (`cascade.testkit.ts`), on the markup the family's wrappers write; jsdom answers no
 * container query, so the stacking rules are read off the sheet, and the real WebView's drawing
 * is `settings-limits.e2e.ts`'s and `settings-tables.e2e.ts`'s range check.
 */

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { drawnWith } from "../../cascade.testkit";
import { isGeometry } from "../../theme/geometry";
import { property, TOKENS } from "../../theme/theme";

const css = readFileSync(join(process.cwd(), "src/ui/table/table.css"), "utf8").replace(
  /\/\*[\s\S]*?\*\//g,
  "",
);

/** The condition a stacking table's frame is queried with (#1859). */
const STACKS_UNDER = "@container table-frame (max-width: 32rem)";

type Rule = { selector: string; body: string };

function rules(text: string): Rule[] {
  return [...text.matchAll(/([^{}]+)\{([^{}]*)\}/g)]
    .map(([, selector, body]) => ({ selector: selector.trim().replace(/\s+/g, " "), body }))
    .filter(({ selector }) => !selector.startsWith("@"));
}

function value(body: string, name: string): string | undefined {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const hit = new RegExp(`(?:^|[;\\s])${escaped}\\s*:\\s*([^;]+);`).exec(body);
  return hit?.[1].trim().replace(/\s+/g, " ");
}

function draw(html: string): HTMLElement {
  document.body.innerHTML = html;
  return document.body;
}

afterEach(() => {
  document.body.innerHTML = "";
});

/** A table of controls as `ControlTable` writes one. */
const controls = (fixed = false) => `
  <div data-scope="table" data-part="frame" data-stacks="true">
    <table data-scope="table" data-part="body" data-controls="" data-stacks="true" ${fixed ? 'data-fixed="true"' : ""}>
      <thead data-scope="table" data-part="header"><tr>
        <th scope="col" data-scope="table" data-part="head" data-align="start">Level</th>
        <th scope="col" data-scope="table" data-part="head" data-align="start"><span data-part="head-tip">Tasks</span></th>
        <th scope="col" data-scope="table" data-part="head" data-align="start" data-actions="true"></th>
      </tr></thead>
      <tbody data-scope="table" data-part="rows">
        <tr data-scope="table" data-part="root"><th scope="row" data-scope="table" data-part="cell" data-names-row="">Project</th><td data-scope="table" data-part="cell" data-align="end">3</td><td data-scope="table" data-part="cell" data-actions="true"></td></tr>
        <tr data-scope="table" data-part="root" data-current="true"><th scope="row" data-scope="table" data-part="cell" data-names-row="">You</th><td data-scope="table" data-part="cell" data-align="end">2</td><td data-scope="table" data-part="cell"></td></tr>
      </tbody>
    </table>
  </div>`;

describe("the table family's stylesheet", () => {
  it("reads only tokens: a theme's colours and the core geometry", () => {
    const colours = new Set(TOKENS.map(property));
    const read = [...css.matchAll(/var\((--[\w-]+)\)/g)].map((hit) => hit[1]);
    expect(read.length).toBeGreaterThan(0);
    expect(read.filter((name) => !colours.has(name) && !isGeometry(name))).toEqual([]);
  });

  it("writes no length of its own but a hairline, and the one room a table stacks under", () => {
    // A container query's condition cannot read a token, so the width a table stacks under is
    // the one length written, once (#1859).
    expect(css.split(STACKS_UNDER)).toHaveLength(2);
    // The label role's tracking (a column's name, as a list's group name) is a share of its own
    // size, and no length.
    const lengths = [
      ...css
        .replace(STACKS_UNDER, "")
        .split("letter-spacing: 0.04em")
        .join("")
        .matchAll(/(?<![\w-])(\d*\.?\d+)(px|rem|em)\b/g),
    ]
      .map((hit) => hit[0])
      // A hairline, and WCAG 2.5.8's least target, 24 CSS px in every density.
      .filter((one) => one !== "1px" && one !== "24px");
    expect(lengths).toEqual([]);
  });

  it("keeps every column's name on one line, cut with an ellipsis, in the label role", () => {
    const body = draw(controls());
    const heads = [...body.querySelectorAll("thead th")];
    expect(heads).toHaveLength(3);
    for (const head of heads) {
      expect(drawnWith(head, "white-space")).toBe("nowrap");
      expect(drawnWith(head, "overflow")).toBe("hidden");
      expect(drawnWith(head, "text-overflow")).toBe("ellipsis");
      expect(drawnWith(head, "font-size")).toBe("var(--font-size-xs)");
      expect(drawnWith(head, "text-transform")).toBe("uppercase");
      expect(drawnWith(head, "color")).toBe("var(--text-muted)");
    }
    // A row's heading is a cell, not a column's name.
    expect(drawnWith(body.querySelector("tbody th") as Element, "white-space")).toBeUndefined();
    expect(
      drawnWith(body.querySelector('[data-part="head-tip"]') as Element, "text-decoration"),
    ).toBe("underline dotted");
  });

  it("aligns every cell to the start and the middle, from the spacing scale, a number at the end", () => {
    const body = draw(controls());
    for (const cell of body.querySelectorAll('tbody [data-part="cell"]:not([data-align="end"])')) {
      expect(drawnWith(cell, "vertical-align")).toBe("middle");
      expect(drawnWith(cell, "padding-inline")).toBe("var(--space-5)");
      expect(drawnWith(cell, "padding-block")).toBe("var(--space-4)");
    }
    expect(drawnWith(body.querySelector('[data-align="end"]') as Element, "text-align")).toBe(
      "end",
    );
  });

  it.each([
    [false, "auto"],
    [true, "fixed"],
  ])("lays out a table whose view sizes its columns (%s) as %s", (fixed, layout) => {
    const body = draw(controls(fixed));
    expect(drawnWith(body.querySelector("table") as Element, "table-layout")).toBe(layout);
  });

  it("draws the row the view is about as a chosen row, and no other", () => {
    const body = draw(controls());
    const [plain, current] = body.querySelectorAll("tbody tr");
    expect(drawnWith(current, "background-color")).toBe("var(--list-selected)");
    expect(drawnWith(plain, "background-color")).toBeUndefined();
  });

  it("holds a table of data to a least width its frame scrolls sideways, and a table of controls to none", () => {
    const body = draw(
      `<div data-scope="table" data-part="frame"><table data-scope="table" data-part="body"></table></div>`,
    );
    const table = body.querySelector("table") as Element;
    expect(drawnWith(table, "min-inline-size")).toBe("calc(var(--space-8) * 17.5)");
    expect(drawnWith(table.parentElement as Element, "overflow-x")).toBe("auto");
    table.setAttribute("data-controls", "");
    expect(drawnWith(table, "min-inline-size")).toBeUndefined();
  });
});

/**
 * **A table of controls that stacks into a form where its frame is narrow** (#1859): `stacks`
 * on `ControlTable`.
 */
describe("the rule between rows", () => {
  it("is its own part, which the family draws as none, and no cell or name draws a border", () => {
    const body = draw(controls());
    const [plain] = body.querySelectorAll("tbody tr");
    const rule = document.createElement("span");
    rule.setAttribute("data-scope", "table");
    rule.setAttribute("data-part", "rule");
    plain.querySelector("td")?.append(rule);
    expect(drawnWith(rule, "background-color")).toBeUndefined();
    expect(drawnWith(rule, "block-size")).toBe("1px");
    // The check's own edge is a control's (the choice family's, later); nothing else draws a line.
    expect(css).not.toMatch(/border-block(-start|-end)?:\s*1px/);
  });
});

describe("the stacking table", () => {
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
  const STACKS = '[data-scope="table"][data-part="body"][data-stacks]';
  const inside = (selector: string) => {
    const found = stacked().find((one) => one.selector === `${STACKS} ${selector}`);
    expect(found, `no stacked rule ${selector}`).toBeDefined();
    return (found as Rule).body;
  };

  it("measures its frame, the holder it is drawn in", () => {
    const frame = rules(css).find(
      (one) => one.selector === '[data-scope="table"][data-part="frame"][data-stacks]',
    );
    expect(value(frame?.body ?? "", "container")).toBe("table-frame / inline-size");
  });

  it("puts the columns' names away and makes each row a grid of its cells, under the row's own heading", () => {
    const away = stacked().find((one) => one.selector === `${STACKS} > [data-part="header"]`);
    expect(value(away?.body ?? "", "display")).toBe("none");
    const row = inside('[data-part="root"]');
    expect(value(row, "display")).toBe("grid");
    expect(value(row, "grid-template-columns")).toMatch(
      /^repeat\(auto-fill, minmax\(calc\(var\(--size-control\) \* \d+\), 1fr\)\)$/,
    );
    expect(
      value(inside('[data-part="cell"]:is([data-names-row], [data-group])'), "grid-column"),
    ).toBe("1 / -1");
    expect(value(inside('[data-part="cell"][data-actions]'), "grid-column")).toBe("1 / -1");
  });

  it("gives a cell across the columns the row's whole width (#1865)", () => {
    expect(value(inside("td[colspan]"), "grid-column")).toBe("1 / -1");
  });

  it("says each cell's column above it, in its whole words, and drops a cell with nothing in it", () => {
    const words = inside('[data-part="cell"][data-heading]::before');
    expect(value(words, "content")).toBe("attr(data-heading)");
    expect(value(words, "display")).toBe("block");
    expect(value(inside('td[data-part="cell"][data-empty]'), "display")).toBe("none");
  });

  it("lets a cell's words wrap, never cutting them", () => {
    const cell = inside('[data-part="cell"]');
    expect(value(cell, "white-space")).toBe("normal");
    expect(value(cell, "overflow-wrap")).toBe("anywhere");
  });
});
