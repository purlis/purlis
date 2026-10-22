// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The suggested answer and the consent answers are never toggles** (#2958, ADR 0097 §6).
 *
 * A toggle (`aria-pressed`, `data-selected`) is a plain, small or ghost button, drawn in the
 * family's toggle look when it is on: a primary, an Allow or a danger button is never one. This
 * reads every component and fails naming the element that makes one.
 */

/// <reference types="node" />
import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import ts from "typescript";
import { describe, expect, it } from "vitest";

const SRC = join(process.cwd(), "src");
/** The variants a toggle is never drawn in: the suggested answer, and the consent ones. */
const NEVER = ["primary", "grants", "danger"];
/** What makes a button a toggle, beside being the family's `ToggleButton`. */
const TOGGLES = ["aria-pressed", "data-selected"];

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
        const classes = strings(named("className")?.initializer).flatMap((one) => one.split(/\s+/));
        const variants = strings(
          named("data-variant")?.initializer ?? named("variant")?.initializer,
        );
        const variant = NEVER.find(
          (one) => classes.includes(`ui-button-${one}`) || variants.includes(one),
        );
        const toggle =
          node.tagName.getText(source) === "ToggleButton"
            ? "ToggleButton"
            : TOGGLES.find((one) => named(one) !== undefined);
        if (variant !== undefined && toggle !== undefined)
          found.push(`${file.name}: <${node.tagName.getText(source)}> ${variant} with ${toggle}`);
      }
      ts.forEachChild(node, visit);
    };
    visit(source);
  }
  return found;
}

describe("a toggle", () => {
  it("is found where a primary, an Allow or a danger button is made one", () => {
    expect(
      toggled([
        {
          name: "A.tsx",
          text: `export const A = () => <>
            <button className="ui-button ui-button-primary" aria-pressed={on} />
            <button className={\`ui-button \${grants ? "ui-button-grants" : ""}\`} data-selected />
            <Button variant="danger" aria-pressed={on} />
            <ToggleButton variant="primary" isSelected={on} />
            <ToggleButton isSelected={on} />
            <button className="ui-button" aria-pressed={on} />
            <button className="ui-button ui-button-ghost" aria-pressed={on} />
          </>;`,
        },
      ]),
    ).toEqual([
      "A.tsx: <button> primary with aria-pressed",
      "A.tsx: <button> grants with data-selected",
      "A.tsx: <Button> danger with aria-pressed",
      "A.tsx: <ToggleButton> primary with ToggleButton",
    ]);
  });

  it("is never a primary, an Allow or a danger button in the window", () => {
    expect(toggled(sources(SRC))).toEqual([]);
  });
});
