// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **Purlis draws every corner on the record's scale** (#3177, R157, R158): 7, 8, 10, 12 and
 * full, or none.
 *
 * Every corner a family's stylesheet or a primitive's writes is worked out with what each Purlis
 * appearance hands the window (`.storybook/built-in-themes.json`): its geometry, and the custom
 * properties the same stylesheet declares (`--tabs-hover-corner`). A corner that comes out off the
 * scale (the 9 the rails drew, the 5 of a tab's ×, a 6 or a 3, the peek's 20) fails here, and so
 * does a corner Purlis's parts set off it. Purlis Classic keeps its own corners and is not held
 * to this scale. Each stylesheet is read off disk, not through `?raw`: vitest does not process CSS.
 */
import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";
import { readBuiltInThemes } from "../../.storybook/builtInThemes";

const APP = process.cwd();
const SRC = join(APP, "src");
const SCALE = [0, 7, 8, 10, 12, 9999];

type Cell = { geometry: { comfortable: Record<string, string> }; parts: string };
const PURLIS = readBuiltInThemes<Cell>(join(APP, ".storybook", "built-in-themes.json")).purlis;

function stylesheets(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return stylesheets(path);
    return entry.name.endsWith(".css") ? [path] : [];
  });
}

const SHEETS = [...stylesheets(join(SRC, "ui")), ...stylesheets(join(SRC, "primitives"))];

/** A sum of `+ - * /` over numbers, brackets, `max()` and `min()`, as `calc()` works it out. */
function evaluate(text: string): number {
  const tokens = text.match(/max|min|-?[\d.]+|[()+\-*/,]/g) ?? [];
  let at = 0;
  const peek = () => tokens[at];
  const take = () => tokens[at++];
  const sum = (): number => {
    let value = product();
    while (peek() === "+" || peek() === "-")
      value = take() === "+" ? value + product() : value - product();
    return value;
  };
  const product = (): number => {
    let value = atom();
    while (peek() === "*" || peek() === "/")
      value = take() === "*" ? value * atom() : value / atom();
    return value;
  };
  const atom = (): number => {
    const token = take();
    if (token === "(") {
      const value = sum();
      take();
      return value;
    }
    if (token === "max" || token === "min") {
      take();
      const values = [sum()];
      while (peek() === ",") {
        take();
        values.push(sum());
      }
      take();
      return token === "max" ? Math.max(...values) : Math.min(...values);
    }
    const value = Number(token);
    if (Number.isNaN(value)) throw new Error(`${token} in ${text}`);
    return value;
  };
  const value = sum();
  if (at !== tokens.length) throw new Error(`${text} is not one sum`);
  return value;
}

/**
 * `value` worked out to px: every `var()` replaced by what `known` gives it (else its fallback),
 * then `calc`, `min` and `max` evaluated. Each of a shorthand's corners is one number.
 */
function corners(value: string, known: Record<string, string>): number[] {
  let text = value;
  for (let round = 0; round < 8 && text.includes("var("); round++) {
    text = text.replace(/var\((--[\w-]+)(?:,\s*([^()]+))?\)/g, (_, name: string, fallback) => {
      const found = known[name] ?? fallback;
      if (found === undefined) throw new Error(`${name} is not known in ${value}`);
      return `(${found})`;
    });
  }
  text = text.replace(/calc\(\s*(-?[\d.]+)rem \/ 14\)/g, "$1px");
  text = text.replace(/(-?[\d.]+)rem/g, (_, n: string) => `${Number(n) * 14}px`);
  text = text.replace(/(-?[\d.]+)px/g, "$1");
  text = text.replace(/\bcalc\(/g, "(");
  // The corners of a shorthand are split on the spaces that are not inside a bracket.
  const parts: string[] = [];
  let depth = 0;
  let one = "";
  for (const char of text.trim()) {
    if (char === "(") depth++;
    if (char === ")") depth--;
    if (/\s/.test(char) && depth === 0) {
      if (one) parts.push(one);
      one = "";
    } else one += char;
  }
  if (one) parts.push(one);
  return parts.map((part) => Math.round(evaluate(part) * 100) / 100);
}

describe("Purlis's corners", () => {
  for (const [id, cell] of Object.entries(PURLIS)) {
    const geometry = cell.geometry.comfortable;

    it.each(SHEETS.map((path) => [relative(SRC, path), path]))(
      `${id}: every corner %s draws is on the scale`,
      (_, path) => {
        const css = readFileSync(path, "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
        const own: Record<string, string> = {};
        for (const [, name, value] of css.matchAll(/(?<![\w-])(--[\w-]+)\s*:\s*([^;{}]+);/g)) {
          own[name] ??= value.trim();
        }
        const known = { ...own, ...geometry };
        for (const [, value] of css.matchAll(/border(?:-[a-z]+)*-radius\s*:\s*([^;{}]+);/g)) {
          const said = value.trim();
          if (said === "inherit" || said.endsWith("%")) continue;
          for (const corner of corners(said, known)) {
            expect(SCALE, `${said} is ${corner}px`).toContain(corner);
          }
        }
      },
    );

    it(`${id}: every corner its parts set is on the scale`, () => {
      for (const [, length] of cell.parts.matchAll(/border-radius: min\(([\d.]+)px/g)) {
        expect(SCALE).toContain(Number(length));
      }
    });
  }
});
