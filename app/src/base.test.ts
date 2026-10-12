// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The control baseline** (ADR 0094 §3, #1833): no `button`, `input`, `select` or `textarea`
 * is drawn with WebKit's defaults, and the baseline never beats a view's own rule.
 *
 * vitest does not apply stylesheets, so this reads `base.css` and `styles.css` off disk and
 * holds what makes the baseline safe to put under 180 controls at once: it is in the `base`
 * layer under `App.css`, every selector has no specificity, it reads only tokens, and it sets
 * no height a container's rule would lose to.
 */

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { isGeometry } from "./theme/geometry";
import { property, TOKENS } from "./theme/theme";

const SRC = join(process.cwd(), "src");

function read(file: string): string {
  const text = readFileSync(join(SRC, file), "utf8");
  expect(text.length, `${file} read as empty`).toBeGreaterThan(0);
  return text.replace(/\/\*[\s\S]*?\*\//g, "");
}

/** Every rule as `[selector, body]`, at any depth (`@supports` included). */
function rules(css: string): [string, string][] {
  return [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)]
    .map(([, selector, body]) => [selector.trim().replace(/\s+/g, " "), body] as [string, string])
    .filter(([selector]) => !selector.startsWith("@"));
}

/** A selector with every `:where(...)` taken out, nested parentheses and all. */
function outsideWhere(selector: string): string {
  let rest = selector;
  for (;;) {
    const at = rest.indexOf(":where(");
    if (at < 0) return rest;
    let depth = 0;
    let end = at + ":where".length;
    for (; end < rest.length; end++) {
      if (rest[end] === "(") depth++;
      if (rest[end] === ")" && --depth === 0) break;
    }
    rest = rest.slice(0, at) + rest.slice(end + 1);
  }
}

const css = read("base.css");
const all = rules(css);
const unclassed = (tag: string) =>
  all.filter(
    ([selector]) =>
      selector.includes(tag) && selector.includes(`:where(:not([class]), [class=""])`),
  );

describe("the control baseline", () => {
  it("is under App.css: in the base layer, which comes before charter's", () => {
    const styles = read("styles.css");
    // Spelled in two halves: the stylesheet guard reads a quoted path ending in the extension,
    // anywhere in a source file, as an import (`literals.test.ts`).
    expect(styles).toContain(['@import "./base', 'css" layer(base);'].join("."));
    const order = (/@layer ([\w\s,-]+);/.exec(styles)?.[1] ?? "").split(/\s*,\s*/);
    expect(order.indexOf("base")).toBeGreaterThanOrEqual(0);
    expect(order.indexOf("base")).toBeLessThan(order.indexOf("charter"));
  });

  it("has no specificity: every selector is :where and nothing else", () => {
    expect(all.length).toBeGreaterThan(0);
    const specific = all.map(([selector]) => selector).filter((s) => outsideWhere(s).trim() !== "");
    expect(specific).toEqual([]);
  });

  it("gives every control the window's font", () => {
    const every = all.find(([selector]) => selector === ":where(button, input, select, textarea)");
    expect(every?.[1]).toMatch(/font:\s*inherit/);
  });

  it.each(["button", "input", "select", "textarea"])(
    "draws an unclassed %s from the tokens, with the outside focus ring",
    (tag) => {
      const body = unclassed(tag)
        .map(([, b]) => b)
        .join("\n");
      expect(body).toMatch(/appearance:\s*none/);
      expect(body).toMatch(/border:\s*1px solid var\(--border-strong\)/);
      expect(body).toMatch(/border-radius:\s*var\(--radius-md\)/);
      expect(body).toMatch(/background-color:\s*var\(--(surface-sunken|control-base)\)/);
      expect(body).toMatch(/padding-inline:\s*var\(--space-4\)/);
      expect(body).toMatch(/outline:\s*var\(--focus-ring-width\) solid var\(--focus-ring\)/);
      expect(body).toMatch(/outline-offset:\s*var\(--focus-ring-offset\)/);
    },
  );

  it.each(["button", "input", "select"])(
    "makes an unclassed %s the control height from padding",
    (tag) => {
      const body = unclassed(tag)
        .map(([, b]) => b)
        .join("\n");
      expect(body).toContain("padding-block: calc((var(--size-control) - 1lh) / 2 - 1px)");
    },
  );

  it("sets no height a container's rule would lose to", () => {
    // `min-height` beats a container's own `height`, and would push a 21px strip taller. A
    // textarea's minimum is rows of text, and no container sizes one by height.
    const heights = all
      .filter(([selector]) => !/^:where\(textarea\)/.test(selector))
      .flatMap(([selector, body]) =>
        [...body.matchAll(/(?<![\w-])((?:min-|max-)?(?:height|block-size))\s*:/g)].map(
          (hit) => `${selector} | ${hit[1]}`,
        ),
      );
    expect(heights).toEqual([]);
  });

  it("reads only tokens: a theme's colours and the core geometry", () => {
    const colours = new Set(TOKENS.map(property));
    const read = [...css.matchAll(/var\((--[\w-]+)\)/g)].map((hit) => hit[1]);
    expect(read.length).toBeGreaterThan(0);
    expect(read.filter((name) => !colours.has(name) && !isGeometry(name))).toEqual([]);
  });
});
