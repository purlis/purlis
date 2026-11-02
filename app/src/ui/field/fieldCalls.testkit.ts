// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **Where the window hands a field a prop** (#3254, #4648): the reader the field guards share.
 * `refusingFields.guard.test.ts` finds each file that hands a field a refusal (`validate`,
 * `error`), and `fieldHelp.guard.test.ts` each that hands one its help, so each guard holds the
 * grounds the parts manifest reads that part on (`FIELD_ERROR_GROUNDS`, `FIELD_HELP_GROUNDS` in
 * `crates/purlis-core/src/theme/parts.rs`) to the files that draw it.
 */
import { readdirSync, readFileSync } from "node:fs";
import { join, posix, relative } from "node:path";
import ts from "typescript";

export const SRC = join(process.cwd(), "src");
export const PARTS = join(process.cwd(), "..", "crates", "purlis-core", "src", "theme", "parts.rs");

/** Every component file under `dir`, tests and stories left out, by its path from `src`. */
export function sources(dir: string): { name: string; text: string }[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return sources(path);
    return /\.tsx$/.test(entry.name) && !/\.(test|stories)\.tsx$/.test(entry.name)
      ? [{ name: relative(SRC, path), text: readFileSync(path, "utf8") }]
      : [];
  });
}

/** Whether a spread may carry one of `props`: anything but an object written in place that
 *  names none of them and spreads nothing (`{...{ [RENAMES_ON_F2]: "" }}`), or a choice between
 *  two such (`{...(said ? { error: said } : {})}` carries no help). */
function mayCarry(spread: ts.Expression, source: ts.SourceFile, props: string[]): boolean {
  let at = spread;
  while (ts.isParenthesizedExpression(at)) at = at.expression;
  if (ts.isConditionalExpression(at))
    return mayCarry(at.whenTrue, source, props) || mayCarry(at.whenFalse, source, props);
  if (!ts.isObjectLiteralExpression(at)) return true;
  return at.properties.some(
    (one) =>
      ts.isSpreadAssignment(one) ||
      ts.isShorthandPropertyAssignment(one) ||
      (one.name !== undefined &&
        !ts.isComputedPropertyName(one.name) &&
        props.includes(one.name.getText(source).replace(/["']/g, ""))),
  );
}

/** `spec` imported from `file`, as a module under `src/` without its extension; `undefined` for
 *  a package. */
function resolved(file: string, spec: string): string | undefined {
  if (!spec.startsWith(".")) return undefined;
  return posix.normalize(posix.join(posix.dirname(file), spec)).replace(/\.tsx?$/, "");
}

/** The name of the component a node is drawn in: its function's, or its variable's. */
function drawnIn(node: ts.Node): string | undefined {
  for (let at: ts.Node | undefined = node.parent; at !== undefined; at = at.parent) {
    if (ts.isFunctionDeclaration(at) && at.name !== undefined) return at.name.text;
    if (ts.isVariableDeclaration(at) && ts.isIdentifier(at.name)) return at.name.text;
  }
  return undefined;
}

/**
 * **What `files` say about `props`**: each file that hands one of them to a component `draws`
 * names (by the module that defines it, under `src/` without its extension), under any name it
 * is imported by, and each element that forwards one from a component `draws` does not list,
 * said as handing the field `what`. A call given one of `props`, or a spread that may carry one,
 * hands it where the call stands; the forwarding inside a listed wrapper does not, since its
 * callers decide.
 */
export function handed(
  files: { name: string; text: string }[],
  draws: Record<string, string[]>,
  props: string[],
  what: string,
): { found: string[]; forwarders: string[] } {
  const found = new Set<string>();
  const forwarders: string[] = [];
  for (const file of files) {
    const module = file.name.replace(/\.tsx?$/, "");
    const own = draws[module] ?? [];
    const source = ts.createSourceFile(
      file.name,
      file.text,
      ts.ScriptTarget.Latest,
      true,
      ts.ScriptKind.TSX,
    );
    // Each member this file names, as it names it: imported (under any name), or its own.
    const members = new Set<string>(own);
    for (const statement of source.statements) {
      if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier))
        continue;
      const from = resolved(file.name, statement.moduleSpecifier.text);
      const names = from === undefined ? undefined : draws[from];
      const bindings = statement.importClause?.namedBindings;
      if (names === undefined || bindings === undefined) continue;
      if (ts.isNamespaceImport(bindings))
        for (const name of names) members.add(`${bindings.name.text}.${name}`);
      else
        for (const one of bindings.elements)
          if (names.includes((one.propertyName ?? one.name).text)) members.add(one.name.text);
    }
    const visit = (node: ts.Node) => {
      if (
        (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) &&
        members.has(node.tagName.getText(source))
      ) {
        const given = node.attributes.properties.filter((one) =>
          ts.isJsxSpreadAttribute(one)
            ? mayCarry(one.expression, source, props)
            : ts.isJsxAttribute(one) && props.includes(one.name.getText(source)),
        );
        // A prop handed on from the component's own props, rather than decided here.
        const forwarded = given.some(
          (one) => ts.isJsxSpreadAttribute(one) || /\bprops\b/.test(one.getText(source)),
        );
        const inside = drawnIn(node);
        if (forwarded && inside !== undefined && own.includes(inside)) {
          // The wrapper's own forwarding: its callers are read where they stand.
        } else if (forwarded && !own.includes(inside ?? "")) {
          const tag = node.tagName.getText(source);
          if (/\bprops\b/.test(given.map((one) => one.getText(source)).join(" ")))
            forwarders.push(`${file.name}: ${inside ?? "?"} hands <${tag}> ${what}`);
          // A spread may carry one: the call is read as handing it where it stands.
          found.add(file.name);
        } else if (given.length > 0) found.add(file.name);
      }
      ts.forEachChild(node, visit);
    };
    visit(source);
  }
  return { found: [...found].sort(), forwarders };
}

/** The grounds the manifest lists in `source` under the shared list `name`. */
export function listed(source: string, name: string): string[] | undefined {
  const list = new RegExp(`\\nconst ${name}: &\\[&str\\] = &\\[([^\\]]*)\\];`).exec(source);
  return list === null ? undefined : [...list[1].matchAll(/"([^"]+)"/g)].map(([, one]) => one);
}
