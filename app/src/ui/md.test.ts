// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **One md height and one md corner, in every family and every built-in theme** (#3113, #3177).
 *
 * An md control is `size.control` tall (28 at the default text size, the ruling on #2793) and
 * rounded by `radius.control` (8 in Purlis, the record's fields and rows; Classic's `radius.md`
 * in Classic). This holds both ends: what every built-in theme and appearance hands the window
 * (`.storybook/built-in-themes.json`, which a test of the app's crate holds to the engine), and
 * that each md family's stylesheet sizes and rounds its root by those two tokens alone, with no
 * built-in's parts drawing it at another. A family that drifts, by a literal or by a part, fails
 * here. The engine holds the same numbers (`every_built_in_draws_one_md_height_and_one_md_corner`).
 *
 * Each stylesheet is read off disk, not through `?raw`: vitest does not process CSS.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const APP = process.cwd();
const SRC = join(APP, "src");

type Cell = { geometry: { comfortable: Record<string, string> }; parts: string };
const THEMES = JSON.parse(
  readFileSync(join(APP, ".storybook", "built-in-themes.json"), "utf8"),
) as Record<string, Record<string, Cell>>;

const CELLS: [string, string, Cell][] = Object.entries(THEMES).flatMap(([theme, appearances]) =>
  Object.entries(appearances).map(([id, cell]) => [theme, id, cell] as [string, string, Cell]),
);

/** A geometry length in px at the default text size: `calc(Nrem / 14)`, `Nrem` or `Npx`. */
function px(value: string): number {
  let hit = /^calc\((-?[\d.]+)rem \/ 14\)$/.exec(value);
  if (hit) return Number(hit[1]);
  hit = /^(-?[\d.]+)rem$/.exec(value);
  if (hit) return Number(hit[1]) * 14;
  hit = /^(-?[\d.]+)px$/.exec(value);
  if (hit) return Number(hit[1]);
  throw new Error(`${value} is not a geometry length`);
}

function stylesheet(path: string): string {
  const text = readFileSync(join(SRC, path), "utf8");
  expect(text.length, `${path} read as empty`).toBeGreaterThan(0);
  return text.replace(/\/\*[\s\S]*?\*\//g, "");
}

/** The declarations of every rule whose selector list names `selector` itself. */
function declarations(css: string, selector: string): string {
  return [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)]
    .filter(([, selectors]) =>
      selectors
        .split(",")
        .map((one) => one.trim())
        .includes(selector),
    )
    .map(([, , body]) => body)
    .join("\n");
}

/** Each md family's root, the stylesheet it is drawn by, and the height property it takes. */
const MD: [string, string, string, string][] = [
  ["button", "ui/button/button.css", '[data-scope="button"][data-part="root"]', "block-size"],
  ["text input", "ui/field/field.css", '[data-scope="text-input"][data-part="root"]', "block-size"],
  ["list row", "ui/list/list.css", '[data-scope="list"][data-part="root"]', "min-block-size"],
  ["select", "ui/select/select.css", '[data-scope="select"][data-part="root"]', "block-size"],
  ["combobox", "ui/select/select.css", '[data-scope="combobox"][data-part="root"]', "block-size"],
];

describe("an md control", () => {
  it.each(CELLS)("is 28 tall with the control corner in %s %s", (theme, _, cell) => {
    const at = cell.geometry.comfortable;
    expect(px(at["--size-control"])).toBe(28);
    const corner = theme === "purlis" ? 8 : px(at["--radius-md"]);
    expect(px(at["--radius-control"])).toBe(corner);
  });

  it.each(MD)(
    "%s: its root is sized and rounded by the two tokens",
    (_, file, selector, height) => {
      const said = declarations(stylesheet(file), selector);
      expect(said).toMatch(new RegExp(`(^|[\\s;])${height}: var\\(--size-control\\);`));
      expect(said).toMatch(/(^|[\s;])border-radius: var\(--radius-control\);/);
    },
  );

  it.each(CELLS)("no part of %s %s draws an md root at another size", (_, __, cell) => {
    for (const [, , selector] of MD.filter(([, file]) => file.startsWith("ui/"))) {
      const rules = cell.parts.split("\n").filter((line) => line.includes(`:where(${selector})`));
      for (const rule of rules) {
        expect(rule, rule).not.toMatch(/border-radius|block-size|--purlis-part-height/);
      }
    }
  });

  it("a title-bar chip is rounded by the control corner too", () => {
    const css = stylesheet("ui/titlebar/titlebar.css");
    expect(declarations(css, '[data-part="workspace"][data-scope="titlebar"]')).toMatch(
      /border-radius: var\(--radius-control\);/,
    );
  });
});
