// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A row that opens a submenu is a plain row** (#3255). The ways to end something are inside
 * the submenu (*Stop ▸*), never the row that opens it (`docs/ui-primitives.md`, "Menus"), so a
 * danger row is never drawn open. The parts manifest says so (`DRAWN_ONLY` in
 * `crates/purlis-core/src/theme/parts.rs`), and the theme check holds no colour on the open
 * row's ground for a look it never draws. This reads every component and fails naming a
 * `Submenu`, under whatever name it is imported, whose row takes a variant, is not a `MenuItem`
 * written in place, or is handed props through a spread it cannot read.
 */

/// <reference types="node" />
import { readdirSync, readFileSync } from "node:fs";
import { join, posix, relative } from "node:path";
import ts from "typescript";
import { describe, expect, it } from "vitest";

const SRC = join(process.cwd(), "src");
const PARTS = join(process.cwd(), "..", "crates", "purlis-core", "src", "theme", "parts.rs");

function sources(dir: string): { name: string; text: string }[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return sources(path);
    return /\.tsx$/.test(entry.name) && !/\.(test|stories)\.tsx$/.test(entry.name)
      ? [{ name: relative(SRC, path), text: readFileSync(path, "utf8") }]
      : [];
  });
}

/** **The spreads on a submenu's row that are read**, by file and the name spread, each with what
 *  it holds: none of them can hand the row a variant. A new one fails until it is listed. */
const SPREADS_READ: Record<string, Record<string, string>> = {
  "Menus.tsx": {
    rest: "useRested's focus handlers (onFocus, onBlur), which wake the row's tooltip",
  },
};

/** The names a file draws the menu family's `Submenu` and `MenuItem` by: imported under any
 *  name, through a namespace (`Menus.Submenu`), or its own, in the family's module. */
function namesOf(file: string, source: ts.SourceFile): { submenu: Set<string>; item: Set<string> } {
  const submenu = new Set<string>();
  const item = new Set<string>();
  if (/^ui\/menu\/Menu\.tsx$/.test(file)) {
    submenu.add("Submenu");
    item.add("MenuItem");
  }
  for (const statement of source.statements) {
    if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier))
      continue;
    const spec = statement.moduleSpecifier.text;
    if (!spec.startsWith(".")) continue;
    const from = posix.normalize(posix.join(posix.dirname(file), spec)).replace(/\.tsx?$/, "");
    if (from !== "ui/menu/Menu") continue;
    const bindings = statement.importClause?.namedBindings;
    if (bindings === undefined) continue;
    if (ts.isNamespaceImport(bindings)) {
      submenu.add(`${bindings.name.text}.Submenu`);
      item.add(`${bindings.name.text}.MenuItem`);
    } else
      for (const one of bindings.elements) {
        const name = (one.propertyName ?? one.name).text;
        if (name === "Submenu") submenu.add(one.name.text);
        if (name === "MenuItem") item.add(one.name.text);
      }
  }
  return { submenu, item };
}

/** Each `Submenu` in `files` whose row is drawn in a variant, is no `MenuItem` written in place,
 *  or hands its row props this cannot read (a spread). */
function rowsApart(files: { name: string; text: string }[]): string[] {
  const found: string[] = [];
  for (const file of files) {
    const source = ts.createSourceFile(
      file.name,
      file.text,
      ts.ScriptTarget.Latest,
      true,
      ts.ScriptKind.TSX,
    );
    const { submenu, item } = namesOf(file.name, source);
    const visit = (node: ts.Node) => {
      if (
        (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) &&
        submenu.has(node.tagName.getText(source))
      ) {
        const attributes = node.attributes.properties;
        const row = attributes
          .filter(ts.isJsxAttribute)
          .find((one) => one.name.getText(source) === "row")?.initializer;
        const drawn =
          row !== undefined && ts.isJsxExpression(row) && row.expression !== undefined
            ? row.expression
            : undefined;
        const opened =
          drawn !== undefined && ts.isJsxElement(drawn)
            ? drawn.openingElement
            : drawn !== undefined && ts.isJsxSelfClosingElement(drawn)
              ? drawn
              : undefined;
        const where = `${file.name}: <${node.tagName.getText(source)}>`;
        if (attributes.some(ts.isJsxSpreadAttribute)) found.push(`${where} props spread`);
        if (opened === undefined || !item.has(opened.tagName.getText(source)))
          found.push(`${where} row not a MenuItem written in place`);
        else {
          const props = opened.attributes.properties;
          const variant = props
            .filter(ts.isJsxAttribute)
            .find((one) => one.name.getText(source) === "variant");
          if (variant !== undefined) found.push(`${where} row ${variant.getText(source)}`);
          const read = SPREADS_READ[file.name] ?? {};
          for (const spread of props.filter(ts.isJsxSpreadAttribute))
            if (!(spread.expression.getText(source) in read))
              found.push(`${where} row props spread`);
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(source);
  }
  return found;
}

describe("a row that opens a submenu", () => {
  it("is the one menu row the parts manifest draws open", () => {
    const parts = readFileSync(PARTS, "utf8");
    expect(parts).toContain('("menu", State::Open, &["default"])');
  });

  it("is found where it takes a variant, is no MenuItem in place, or is spread", () => {
    expect(
      rowsApart([
        {
          name: "A.tsx",
          text: `import { Submenu, MenuItem } from "./ui/menu/Menu";
            export const A = () => <>
            <Submenu row={<MenuItem id="a">A</MenuItem>}>x</Submenu>
            <Submenu row={<MenuItem id="b" variant="danger">B</MenuItem>}>x</Submenu>
            <Submenu row={held}>x</Submenu>
            <Submenu row={<DangerRow />}>x</Submenu>
            <Submenu row={<MenuItem {...rowProps} />}>x</Submenu>
          </>;`,
        },
        {
          name: "views/B.tsx",
          text: `import { Submenu as Sub, MenuItem as Item } from "../ui/menu/Menu";
            import * as Menus from "../ui/menu/Menu";
            export const B = () => <>
            <Sub row={<Item variant="danger">B</Item>}>x</Sub>
            <Menus.Submenu row={<Menus.MenuItem variant="danger">B</Menus.MenuItem>}>x</Menus.Submenu>
            <Sub {...given}>x</Sub>
          </>;`,
        },
      ]),
    ).toEqual([
      'A.tsx: <Submenu> row variant="danger"',
      "A.tsx: <Submenu> row not a MenuItem written in place",
      "A.tsx: <Submenu> row not a MenuItem written in place",
      "A.tsx: <Submenu> row props spread",
      'views/B.tsx: <Sub> row variant="danger"',
      'views/B.tsx: <Menus.Submenu> row variant="danger"',
      "views/B.tsx: <Sub> props spread",
      "views/B.tsx: <Sub> row not a MenuItem written in place",
    ]);
  });

  it("is never a danger row in the window", () => {
    const files = sources(SRC);
    expect(files.some((file) => file.text.includes("<Submenu"))).toBe(true);
    expect(rowsApart(files)).toEqual([]);
  });
});
