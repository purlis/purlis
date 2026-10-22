// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  classNameOf,
  classUtilities,
  declarations,
  isScript,
  openingTags,
  SRC,
  sourceFiles,
  type StyleSource,
  styleEntries,
  unprefixed,
  utilityOf,
  windowFiles,
} from "./guards.testkit";
import { LENGTH_UNITS } from "./cssMath.testkit";

/**
 * **Every focus ring reads the focus-ring tokens** (#2867; `docs/design-system.md`, "The core
 * tokens").
 *
 * A ring written as `outline: 2px` draws 2px whatever a theme's `focus-ring.*` says. So wherever
 * the window draws a focus ring, in a stylesheet rule for a focused element (`:focus`,
 * `:focus-visible`, `:focus-within`, `[data-focus-visible]`) or in a component's or a script's
 * style (`styleEntries`), its width is `var(--focus-ring-width)` and its offset is read from
 * `--focus-ring-offset` (outside the element), `--focus-ring-inset` (inside it) or
 * `--focus-ring-width`, a whole share or multiple of one allowed (`calc(var(--focus-ring-offset) /
 * 2)`). No length is written in a ring, and no width keyword (`thin`, `medium`, `thick`). A ring
 * may be taken away (`none`, `0`). A Tailwind `outline-<n>` or `outline-offset-<n>` is a written
 * ring too.
 *
 * **The one sheet it does not read is the forced-colours sheet** (`theme/forced-colors.css`,
 * #2328): it is core's and holds no theme token at all, since forced colours replace what a theme
 * says, so its ring is the system's, 2px. `forced-colors.test.ts` holds that sheet. There is no
 * other allowance.
 */

/** The sheet no theme token reaches, which `forced-colors.test.ts` holds. */
const FORCED = join("theme", "forced-colors.css");

/** A selector for an element that has focus. */
const FOCUSED = /:focus\b|:focus-visible\b|:focus-within\b|\[data-focus-visible\b/;

/** A property that draws a ring, past its vendor's prefix. */
const RING = /^outline(?:-width|-offset)?$/;

/** A length written as a number, in any unit of `LENGTH_UNITS`. */
const WRITTEN_LENGTH = new RegExp(
  String.raw`(?<![\w-])[-+]?(?:\d*\.)?\d+(?:e[-+]?\d+)?(?:${LENGTH_UNITS.join("|")})(?![\w-])`,
  "i",
);
const WIDTH_KEYWORD = /(?<![\w-])(?:thin|medium|thick)(?![\w-])/i;
const TAKEN_AWAY = /^(?:none|0|0px|initial|inherit|unset|revert)$/i;

/** What is wrong with a ring's `property: value`, `undefined` when it reads the tokens. */
export function ringComplaint(property: string, value: string): string | undefined {
  const name = unprefixed(property);
  if (!RING.test(name)) return undefined;
  const written = value.replace(/^["'`]|["'`]$/g, "").trim();
  if (TAKEN_AWAY.test(written)) return undefined;
  if (WRITTEN_LENGTH.test(written)) return "a length is written in it";
  if (name !== "outline-offset" && WIDTH_KEYWORD.test(written)) return "a width keyword";
  if (name === "outline-offset") {
    return /var\(\s*--focus-ring-(?:offset|inset|width)\s*\)/.test(written)
      ? undefined
      : "its offset reads no focus-ring token";
  }
  if (name === "outline-width" || /\b(?:solid|dashed|dotted|double|auto)\b/.test(written)) {
    return /var\(\s*--focus-ring-width\s*\)/.test(written)
      ? undefined
      : "its width is not var(--focus-ring-width)";
  }
  return undefined;
}

/** Each ring in a stylesheet that does not read the tokens, as `selector { property: value }`. */
export function cssRingLiterals(css: string): string[] {
  return declarations(css)
    .filter(({ context }) => context.some((rule) => FOCUSED.test(rule)))
    .filter(({ property, value }) => ringComplaint(property, value) !== undefined)
    .map(({ context, property, value }) => `${context.at(-1)} { ${property}: ${value} }`);
}

/** A Tailwind utility that writes a ring's width or offset: `outline-2`, `outline-offset-[3px]`. */
const RING_UTILITY = /^-?outline(?:-offset)?-(?:\d|\[)/;

/** Each ring a component or a script writes that does not read the tokens. */
export function scriptRingLiterals(source: string, from: StyleSource = {}): string[] {
  const found: string[] = [];
  for (const { label, property, value } of styleEntries(source, from)) {
    if (ringComplaint(property, value) !== undefined) found.push(`${label}: ${value}`);
  }
  for (const tag of openingTags(source, /^[A-Za-z]/)) {
    for (const word of classUtilities(classNameOf(tag.text))) {
      if (RING_UTILITY.test(utilityOf(word))) found.push(`class ${word}`);
    }
  }
  return found;
}

const read = (file: string) => readFileSync(join(SRC, file), "utf8");

describe("every focus ring reads the focus-ring tokens", () => {
  it("in every stylesheet", () => {
    const sheets = sourceFiles((name) => name.endsWith(".css")).filter((file) => file !== FORCED);
    expect(sheets).toContain("App.css");
    const found = sheets.flatMap((file) =>
      cssRingLiterals(read(file)).map((one) => `${file}: ${one}`),
    );
    expect(found).toEqual([]);
  });

  it("in every component and script", () => {
    const others = windowFiles();
    const found = sourceFiles(isScript).flatMap((file) =>
      scriptRingLiterals(read(file), { file, read: others }).map((one) => `${file}: ${one}`),
    );
    expect(found).toEqual([]);
  });
});

describe("the focus-ring guard", () => {
  it("refuses a written width or offset in a focused element's rule", () => {
    expect(
      cssRingLiterals(`.a:focus-visible { outline: 2px solid var(--focus-ring); }
        .b:focus { outline-offset: -1px; }
        .c[data-focus-visible] { outline-width: thin; }
        :where(button, [tabindex]):focus-visible { outline: 0.15rem solid red; }`),
    ).toHaveLength(4);
  });

  it("refuses an offset that reads no focus-ring token", () => {
    expect(cssRingLiterals(".a:focus-visible { outline-offset: var(--space-1); }")).toHaveLength(1);
  });

  it("refuses a ring nested in a focus rule or a media query", () => {
    expect(
      cssRingLiterals(`@media (min-width: 40rem) { .a:focus-visible { outline-width: 2px; } }
        .b { &:focus-visible { outline: 2px solid; } }`),
    ).toHaveLength(2);
  });

  it("takes the tokens, a whole share of one, and a ring taken away", () => {
    expect(
      cssRingLiterals(`.a:focus-visible {
          outline: var(--focus-ring-width) solid var(--focus-ring);
          outline-offset: var(--focus-ring-inset);
        }
        .b:focus-visible { outline-offset: calc(var(--focus-ring-offset) / 2); }
        .c:focus-visible { outline-offset: calc(-1 * var(--focus-ring-width)); }
        .d:focus { outline: none; }
        .e:focus-visible { outline-color: Highlight; outline-style: solid; }`),
    ).toEqual([]);
  });

  it("leaves an outline that is no focus ring", () => {
    expect(cssRingLiterals(".danger { outline: 2px dashed CanvasText; }")).toEqual([]);
  });

  it("refuses a ring written in a component's style or class", () => {
    const source = `export const A = () => <>
      <div style={{ outline: "2px solid var(--focus-ring)", outlineOffset: "-2px" }} />
      <div className="focus-visible:outline-2 outline-offset-[3px]" />
    </>;`;
    expect(scriptRingLiterals(source, { file: "A.tsx" })).toHaveLength(4);
  });
});
