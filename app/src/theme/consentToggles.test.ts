// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The suggested answer and the consent answers are never toggles** (#2958, ADR 0097 §6).
 *
 * A toggle (`aria-pressed`, `data-selected`) is a plain, small or ghost button, drawn in the
 * family's toggle look when it is on: a primary, an Allow or a danger button is never one. Nor is
 * any other button drawn in that pressed look (`ToggleButton`, `data-selected`): the parts
 * manifest draws it only in the variants `DRAWN_ONLY` lists
 * (`crates/purlis-core/src/theme/parts.rs`, #3255), and the theme check holds no pressed look it
 * leaves out, so this reads them from there. It reads every component and fails naming the
 * element that breaks either.
 */

/// <reference types="node" />
import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import ts from "typescript";
import { describe, expect, it } from "vitest";
import { BUTTON_VARIANTS } from "../ui/button/Button";

const SRC = join(process.cwd(), "src");
const PARTS = join(process.cwd(), "..", "crates", "purlis-core", "src", "theme", "parts.rs");

/** The variants the parts manifest says a button is pressed as a toggle in (`DRAWN_ONLY`). */
function toggledIn(source: string): string[] {
  const listed = /\("button", State::Selected, &\[([^\]]*)\]\)/.exec(source)?.[1] ?? "";
  return [...listed.matchAll(/"([^"]+)"/g)].map(([, variant]) => variant);
}

/** The variants a toggle is never, in any way: the suggested answer, and the consent ones. */
const NEVER = ["primary", "grants", "danger"];
/** What makes a button a toggle, beside being the family's `ToggleButton`. */
const TOGGLES = ["aria-pressed", "data-selected"];
/** The variants never drawn in the pressed look (`[data-selected]`, which `ToggleButton` and a
 *  `data-selected` give): every one the manifest does not draw pressed. */
const UNDRAWN: string[] = BUTTON_VARIANTS.filter(
  (one) => !toggledIn(readFileSync(PARTS, "utf8")).includes(one),
);
/** What draws a button in the pressed look. */
const PRESSED_LOOK = ["data-selected"];

function sources(dir: string): { name: string; text: string }[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return sources(path);
    return /\.tsx$/.test(entry.name) && !/\.(test|stories)\.tsx$/.test(entry.name)
      ? [{ name: relative(SRC, path), text: readFileSync(path, "utf8") }]
      : [];
  });
}

/** Every string a JSX attribute's value can hold, as written. */
function strings(node: ts.Node | undefined): string[] {
  if (node === undefined) return [];
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) return [node.text];
  if (ts.isTemplateExpression(node))
    return [
      node.head.text,
      ...node.templateSpans.flatMap((span) => [span.literal.text, ...strings(span.expression)]),
    ];
  const said: string[] = [];
  ts.forEachChild(node, (child) => said.push(...strings(child)));
  return said;
}

/** Each element of `files` drawn in one of [`NEVER`]'s variants that is made a toggle. */
function toggled(files: { name: string; text: string }[]): string[] {
  const found: string[] = [];
  for (const file of files) {
    const source = ts.createSourceFile(
      file.name,
      file.text,
      ts.ScriptTarget.Latest,
      true,
      ts.ScriptKind.TSX,
    );
    const visit = (node: ts.Node) => {
      if (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) {
        const attributes = node.attributes.properties.filter(ts.isJsxAttribute);
        const named = (name: string) => attributes.find((one) => one.name.getText(source) === name);
        // A button's look is its variant, as the family draws it (#3365: no class marks one).
        const variants = strings(
          named("data-variant")?.initializer ?? named("variant")?.initializer,
        );
        const wears = (one: string) => variants.includes(one);
        const tag = node.tagName.getText(source);
        const marked = (by: string[]) =>
          tag === "ToggleButton" ? "ToggleButton" : by.find((one) => named(one) !== undefined);
        const never = NEVER.find(wears);
        const undrawn = UNDRAWN.find(wears);
        if (never !== undefined && marked(TOGGLES) !== undefined)
          found.push(`${file.name}: <${tag}> ${never} with ${marked(TOGGLES)}`);
        else if (undrawn !== undefined && marked(PRESSED_LOOK) !== undefined)
          found.push(`${file.name}: <${tag}> ${undrawn} with ${marked(PRESSED_LOOK)}`);
      }
      ts.forEachChild(node, visit);
    };
    visit(source);
  }
  return found;
}

describe("a toggle", () => {
  it("is drawn only in the variants the parts manifest draws pressed: plain and ghost", () => {
    expect(toggledIn(readFileSync(PARTS, "utf8"))).toEqual(["secondary", "ghost"]);
    expect(UNDRAWN).toEqual(
      BUTTON_VARIANTS.filter((one) => one !== "secondary" && one !== "ghost"),
    );
    expect(UNDRAWN).toEqual(expect.arrayContaining(NEVER));
  });

  it("is found where a primary, an Allow or a danger button is made one", () => {
    expect(
      toggled([
        {
          name: "A.tsx",
          text: `export const A = () => <>
            <Button variant="primary" aria-pressed={on} />
            <Button variant={grants ? "grants" : undefined} data-selected />
            <Button variant="danger" aria-pressed={on} />
            <ToggleButton variant="primary" isSelected={on} />
            <Button variant="warning-soft" data-selected />
            <Button variant="soft-accent" aria-pressed={on} />
            <ToggleButton isSelected={on} />
            <ToggleButton variant="ghost" isSelected={on} />
            <Button aria-pressed={on} />
            <Button variant="ghost" aria-pressed={on} />
          </>;`,
        },
      ]),
    ).toEqual([
      "A.tsx: <Button> primary with aria-pressed",
      "A.tsx: <Button> grants with data-selected",
      "A.tsx: <Button> danger with aria-pressed",
      "A.tsx: <ToggleButton> primary with ToggleButton",
      "A.tsx: <Button> warning-soft with data-selected",
    ]);
  });

  it("is never a primary, an Allow or a danger button in the window", () => {
    expect(toggled(sources(SRC))).toEqual([]);
  });
});
