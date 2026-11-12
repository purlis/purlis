// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The text roles** (#1920): each role class is exactly the size, weight and colour of the
 * table in `docs/ui-primitives.md` ("Text roles"), built from tokens and nothing else, and it
 * sits in the layer above `App.css` so no view rule re-ranks a word that wears one.
 *
 * vitest does not apply stylesheets, so the sheet is read off disk and the cascade is run over
 * its rules (`cascade.testkit.ts`), as `collections.test.ts` does.
 */

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { cascadeOf, drawnWith } from "../cascade.testkit";
import { declarations } from "../guards.testkit";
import { GEOMETRY, isGeometry } from "../theme/geometry";
import { property, TOKENS } from "../theme/theme";

const SRC = join(process.cwd(), "src");

function read(file: string): string {
  const text = readFileSync(join(SRC, file), "utf8");
  expect(text.length, `${file} read as empty`).toBeGreaterThan(0);
  return text;
}

const css = read("primitives/text.css");
const resolved = cascadeOf("primitives/text.css");

/** The core's own values, to say a token in the terms the table uses. */
const core = new Map(
  declarations(read("theme/geometry.css")).map(({ property: name, value }) => [name, value]),
);

/** A size token in px at the window's default text size (14px). */
function px(token: string): number {
  const value = core.get(token) ?? "";
  const share = /^calc\(([\d.]+)rem \/ 14\)$/.exec(value);
  if (share) return Number(share[1]);
  const rem = /^([\d.]+)rem$/.exec(value);
  if (rem) return Number(rem[1]) * 14;
  throw new Error(`${token} is ${value}, not a core size`);
}

/** What a `var(--x)` names, or the value itself when it is not one. */
const named = (value: string | undefined) => /^var\((--[\w-]+)\)$/.exec(value ?? "")?.[1] ?? value;

function wearing(classes: string): HTMLElement {
  document.body.innerHTML = `<span class="${classes}">words</span>`;
  return document.body.firstElementChild as HTMLElement;
}

afterEach(() => {
  document.body.innerHTML = "";
});

/** The table (#1920, the audit's section 6): px at the default text size, weight, colour. */
const ROLES: [role: string, size: number, weight: string, colour: string][] = [
  ["ui-text-title", 16, "600", "--text-primary"],
  ["ui-text-heading", 14, "600", "--text-primary"],
  ["ui-text-body", 13, "400", "--text-primary"],
  ["ui-text-secondary", 12, "400", "--text-secondary"],
  ["ui-text-meta", 11, "400", "--text-quiet"],
  ["ui-text-label", 11, "600", "--text-secondary"],
];

describe("the text roles", () => {
  it.each(ROLES)("%s is %ipx, %s and %s", (role, size, weight, colour) => {
    const word = wearing(role);
    expect(px(named(resolved(word, "font-size")) ?? "")).toBe(size);
    expect(core.get(named(resolved(word, "font-weight")) ?? "")).toBe(weight);
    expect(named(resolved(word, "color"))).toBe(colour);
  });

  it("the row's one strong line is a body at the strong weight, not a second role", () => {
    const word = wearing("ui-text-body ui-text-strong");
    expect(core.get(named(resolved(word, "font-weight")) ?? "")).toBe("600");
    expect(px(named(resolved(word, "font-size")) ?? "")).toBe(13);
    expect(named(resolved(word, "color"))).toBe("--text-primary");
    // Whatever order the classes are written in, and on every role.
    for (const [role] of ROLES) {
      expect(named(resolved(wearing(`ui-text-strong ${role}`), "font-weight"))).toBe(
        "--font-weight-strong",
      );
    }
    // The modifier is a weight and nothing else.
    const strong = declarations(css).filter(({ context }) => context[0] === ".ui-text-strong");
    expect(strong.map(({ property: name }) => name)).toEqual(["font-weight"]);
  });

  it("a label is in capitals, spaced as the sidebar's titles are", () => {
    const word = wearing("ui-text-label");
    expect(resolved(word, "text-transform")).toBe("uppercase");
    expect(resolved(word, "letter-spacing")).toBe("0.05em");
  });

  it("code is the one monospace family, at the size and colour of what it sits in", () => {
    const word = wearing("ui-text-code");
    expect(named(resolved(word, "font-family"))).toBe("--font-mono");
    expect(core.get(named(resolved(word, "font-weight")) ?? "")).toBe("400");
    expect(resolved(word, "font-size")).toBeUndefined();
    expect(resolved(word, "color")).toBeUndefined();
    // Worn with a role, it keeps that role's size and colour.
    const path = wearing("ui-text-secondary ui-text-code");
    expect(px(named(resolved(path, "font-size")) ?? "")).toBe(12);
    expect(named(resolved(path, "color"))).toBe("--text-secondary");
  });

  it("sets no line height, so a word keeps the line of what it sits in", () => {
    expect(declarations(css).filter(({ property: name }) => name === "line-height")).toEqual([]);
  });

  it("is built from tokens only: sizes, weights and the family from core, colours from a theme", () => {
    const colours = new Set(TOKENS.map(property));
    const typed = declarations(css).filter(({ property: name }) =>
      ["font-size", "font-weight", "font-family", "line-height", "color"].includes(name),
    );
    expect(typed.length).toBeGreaterThanOrEqual(ROLES.length * 3);
    for (const { context, property: name, value } of typed) {
      const token = named(value) ?? "";
      const where = `${context.join(" ")} ${name}: ${value}`;
      if (name === "color") expect(colours.has(token), where).toBe(true);
      else expect(isGeometry(token), where).toBe(true);
    }
    // Each of the core's weights and its family is read by a role, but medium: no role is
    // medium, and a form's label reads it (`App.css`, #2317). The window's own family is the
    // root's (`geometry.css`), which every role inherits.
    for (const token of [...GEOMETRY.weight, ...GEOMETRY.family]) {
      if (token === "--font-weight-medium" || token === "--font-ui") continue;
      expect(css, token).toContain(`var(${token})`);
    }
    expect(read("App.css")).toMatch(/\.ui-setting-label \{[^}]*var\(--font-weight-medium\)/);
  });

  it("is one class per role and nothing else", () => {
    const selectors = [...new Set(declarations(css).map(({ context }) => context.join(" ")))];
    expect(selectors).toEqual([
      ...ROLES.map(([role]) => `.${role}`),
      ".ui-text-code",
      ".ui-text-strong",
    ]);
  });

  it("is above App.css: the last sheet of the components layer", () => {
    const styles = read("styles.css").replace(/\/\*[\s\S]*?\*\//g, "");
    // Spelled in two halves: the stylesheet guard reads a quoted path ending in the extension,
    // anywhere in a source file, as an import (`literals.test.ts`).
    const line = ['@import "./primitives/text', 'css" layer(components);'].join(".");
    expect(styles).toContain(line);
    expect(styles.trimEnd().indexOf(line)).toBeGreaterThan(
      styles.indexOf(['@import "./ui/popover/popover', 'css"'].join(".")),
    );
    const order = (/@layer ([\w\s,-]+);/.exec(styles)?.[1] ?? "").split(/\s*,\s*/);
    expect(order.indexOf("components")).toBeGreaterThan(order.indexOf("charter"));
  });

  it("outranks a view's rule for the same word, in the window's whole cascade", () => {
    // `.label` is one of App.css's own (xs, secondary): a word that also wears a role is the
    // role's.
    const word = wearing("label ui-text-body");
    expect(named(drawnWith(word, "font-size"))).toBe("--font-size-md");
    expect(named(drawnWith(word, "color"))).toBe("--text-primary");
  });
});
