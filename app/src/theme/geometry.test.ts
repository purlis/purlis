// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Core owns geometry; a theme cannot reach it** (ADR 0094, #1833).
 *
 * A theme is data a person or an extension writes. If one could set a control's height, a
 * corner or a gap, light and dark would stop lining up, and a tinted or shared theme could break
 * the alignment of every view. So this holds four things:
 *
 * - `geometry.css` gives every core token a value, the values are the scale the ADR records,
 *   and no other stylesheet gives one a value of its own;
 * - nothing in a theme's vocabulary is a geometry property: no colour token, no motion token;
 * - nothing a theme writes onto the document is one, for the built-ins and every tint of them
 *   (a theme file that tries every spelling of one is refused key by key, and told why, by the
 *   core's theme engine: `purlis_core::theme`'s tests);
 * - `docs/design-system.md` lists every core token.
 *
 * Each stylesheet is read off disk, not through `?raw`: vitest does not process CSS and would
 * hand back an empty string (see `literals.test.ts`).
 */

/// <reference types="node" />
import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";
import { GEOMETRY, GEOMETRY_PROPERTIES, isGeometry } from "./geometry";
import { motionProperty, motionVariables } from "./motion";
import {
  BUILT_IN,
  cssVariables,
  property,
  tinted,
  tintVariables,
  TOKENS,
  WORKSPACE_COLOURS,
} from "./theme";

const SRC = join(process.cwd(), "src");
const CORE = join(SRC, "theme", "geometry.css");

function read(path: string): string {
  const text = readFileSync(path, "utf8");
  expect(text.length, `${path} read as empty`).toBeGreaterThan(0);
  return text.replace(/\/\*[\s\S]*?\*\//g, "");
}

/** Every custom property a stylesheet gives a value, with the value. */
function declared(css: string): [string, string][] {
  return [...css.matchAll(/(?<![\w-])(--[\w-]+)\s*:\s*([^;{}]+)/g)].map(
    ([, name, value]) => [name, value.trim()] as [string, string],
  );
}

/** Every stylesheet under `src`. */
function stylesheets(dir = SRC): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return stylesheets(path);
    return entry.name.endsWith(".css") ? [path] : [];
  });
}

/**
 * A core value in px at the window's default text size (14px), which is how the ADR states the
 * scale. Only the shapes `geometry.css` uses are read, and anything else is a failure.
 */
function px(value: string): number {
  const at = 14;
  let hit = /^calc\((-?[\d.]+)rem \/ 14\)$/.exec(value);
  if (hit) return Number(hit[1]);
  hit = /^(-?[\d.]+)rem$/.exec(value);
  if (hit) return Number(hit[1]) * at;
  hit = /^(-?[\d.]+)px$/.exec(value);
  if (hit) return Number(hit[1]);
  if (value === "0") return 0;
  throw new Error(`${value} is not a core length`);
}

describe("the core geometry", () => {
  const values = new Map(declared(read(CORE)));

  it("is declared in geometry.css, every name once and nothing else", () => {
    const names = declared(read(CORE)).map(([name]) => name);
    expect(names).toEqual([...new Set(names)]);
    expect([...names].sort()).toEqual([...GEOMETRY_PROPERTIES].sort());
  });

  it("is the scale the ADR records", () => {
    const scale = (group: keyof typeof GEOMETRY) =>
      GEOMETRY[group]
        .filter((name) => name !== "--radius-full")
        .map((n) => px(values.get(n) ?? "missing"));
    expect(scale("space")).toEqual([0, 2, 4, 6, 8, 12, 16, 24, 32]);
    expect(scale("type")).toEqual([10, 11, 12, 13, 14, 16, 20]);
    // `radius.control`, an md control's corner, is Classic's `radius.md` (#3113).
    expect(scale("radius")).toEqual([3, 4, 4, 6, 8]);
    expect(scale("size")).toEqual([28, 21]);
    expect(scale("focus")).toEqual([2, 2, -2]);
  });

  it("names three weights, two line heights, the window's family and one monospace family", () => {
    const of = (group: keyof typeof GEOMETRY) => GEOMETRY[group].map((name) => values.get(name));
    expect(of("weight")).toEqual(["400", "500", "600"]);
    // The window's own line height (`:root`), and a compact line's: nothing moves.
    expect(of("leading")).toEqual(["1.2", "1.4"]);
    const root = /:root\s*\{([^}]*)\}/.exec(read(join(SRC, "App.css")))?.[1] ?? "";
    expect(root).toContain("line-height: var(--line-height-body);");
    expect(of("family")).toEqual([
      "ui-sans-serif, Inter, Avenir, Helvetica, Arial, sans-serif",
      "ui-monospace, Menlo, Consolas, monospace",
    ]);
    expect(root.length).toBeGreaterThan(0);
    expect(read(CORE)).toContain("font-family: var(--font-ui);");
  });

  it("lifts a surface in the theme's shadow colour and no other", () => {
    for (const name of GEOMETRY.elevation) {
      const value = values.get(name) ?? "";
      expect(value, name).toContain("var(--overlay-shadow)");
      expect(value.replace(/var\(--overlay-shadow\)/g, ""), name).not.toMatch(/#|rgb|hsl|var\(/);
    }
  });

  it("is given a value by no other stylesheet", () => {
    // A bridge that hands a core property on unchanged (`--radius-md: var(--radius-md)`,
    // Tailwind's `rounded-md` in `styles.css`) is not a value of its own.
    const files = stylesheets().filter((path) => path !== CORE);
    expect(files.map((path) => relative(SRC, path))).toEqual(
      expect.arrayContaining(["App.css", "styles.css", "base.css"]),
    );
    const overridden = files.flatMap((path) =>
      declared(read(path))
        .filter(([name, value]) => isGeometry(name) && value !== `var(${name})`)
        .map(([name, value]) => `${relative(SRC, path)}: ${name}: ${value}`),
    );
    expect(overridden).toEqual([]);
  });
});

/**
 * A theme sets geometry and fonts only as the core's theme engine wrote them from its numbers
 * and files (#2326, `purlis_core::theme::geometry`, whose tests hold the bounds and that what it
 * writes for Purlis Classic is `geometry.css`). Its colours, its motion and a workspace's tint
 * never write a geometry property.
 */
describe("a colour, a motion value or a tint is never geometry", () => {
  it("has no geometry among the colour and motion tokens", () => {
    expect(TOKENS.map(property).filter(isGeometry)).toEqual([]);
    const motion = Object.keys(motionVariables(BUILT_IN["purlis-dark"].motion, false));
    expect(motion.length).toBeGreaterThan(0);
    expect(motion.filter(isGeometry)).toEqual([]);
    expect(motionProperty("duration.quick")).toBe("--motion-duration-quick");
  });

  it.each(Object.keys(BUILT_IN))("writes none from %s, or from any tint of it", (name) => {
    const theme = BUILT_IN[name];
    const written = [
      ...Object.keys(cssVariables(theme)),
      ...Object.keys(motionVariables(theme.motion, false)),
      ...Object.keys(motionVariables(theme.motion, true)),
      ...WORKSPACE_COLOURS.flatMap((colour) => [
        ...Object.keys(cssVariables(tinted(theme, colour))),
        ...Object.keys(tintVariables(theme, colour, TOKENS)),
      ]),
    ];
    expect(written.filter(isGeometry)).toEqual([]);
  });
});

describe("the design-system doc", () => {
  it("lists every core token", () => {
    const doc = readFileSync(join(process.cwd(), "..", "docs", "design-system.md"), "utf8");
    expect(GEOMETRY_PROPERTIES.filter((name) => !doc.includes(`\`${name}\``))).toEqual([]);
  });
});
