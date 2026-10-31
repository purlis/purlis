// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Where a field's error is drawn** (#3254). Only a field that can refuse its value, a
 * `TextField` or a `TextArea` given a `validate` or an `error`, keeps its error's line under its
 * box (`refusable` in `Field.tsx`), and the line's words, in the danger colour, are read on what
 * the field stands on. The theme check holds them at the window's text size on the grounds the
 * parts manifest lists for the error (`FIELD_ERROR_GROUNDS` in
 * `crates/purlis-core/src/theme/parts.rs`), not on every ground a field may stand on.
 *
 * So every file that draws a field that can refuse is listed here with the grounds its field
 * stands on and where, and the manifest's list is exactly theirs and the overlay surface the
 * whole copy of a refusal cut short is drawn on (#3033). A field reached through a wrapper that
 * forwards its refusal (`DRAWS_A_FIELD`) is read where the wrapper is called. A new field that
 * can refuse fails until its file is listed, and a ground it brings until the manifest holds the
 * error's words on it. The grounds are a hand claim: a field that moves to another ground updates
 * its line here.
 */

/// <reference types="node" />
import { readdirSync, readFileSync } from "node:fs";
import { join, posix, relative } from "node:path";
import ts from "typescript";
import { describe, expect, it } from "vitest";

const SRC = join(process.cwd(), "src");
const PARTS = join(process.cwd(), "..", "crates", "purlis-core", "src", "theme", "parts.rs");

/** Every file that draws a field that can refuse, the grounds that field stands on, and where. */
const REFUSING: Record<string, { grounds: string[]; where: string }> = {
  "AnswerQuestion.tsx": {
    grounds: ["surface.raised", "surface.overlay"],
    where:
      "AnswerBox: under a question's line in the Activity tab's panel, and in the answer dialog (AnswerPanel.tsx)",
  },
  "Panels.tsx": {
    grounds: ["surface.base"],
    where: "AddTodo: at the foot of the todos region panel, the window's own ground",
  },
  "PersonaMarkPicker.tsx": {
    grounds: ["surface.raised"],
    where: "HexColour: in a persona's card, a view tab's panel (Views.tsx)",
  },
  "RaiseLimit.tsx": {
    grounds: ["surface.overlay", "surface.base"],
    where:
      "RaiseLimit: a limit's new value, in the Start a chat dialog and under a move's line on the pane",
  },
};

/** The ground the whole copy of a refusal cut short is drawn on, wherever its field stands. */
const WHOLE_COPY = "surface.overlay";

function sources(dir: string): { name: string; text: string }[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return sources(path);
    return /\.tsx$/.test(entry.name) && !/\.(test|stories)\.tsx$/.test(entry.name)
      ? [{ name: relative(SRC, path), text: readFileSync(path, "utf8") }]
      : [];
  });
}

/**
 * **The components that draw a field and may hand it a refusal**, by the module that defines
 * them (under `src/`, without its extension): the field family's own two, and each wrapper that
 * forwards a `validate` or an `error` to one (a setting's `Field` and its `ListField`, the
 * palette's `ListComboBox`). A call of any of them given a `validate`, an `error` or a spread of
 * props it cannot read draws a field that can refuse, where the call stands; the forwarding inside
 * a wrapper does not, since its callers decide. A component that forwards a refusal and is not
 * listed here fails (`forwarders`), so a new wrapper cannot hide its callers.
 */
const DRAWS_A_FIELD: Record<string, string[]> = {
  "ui/field/Field": ["TextField", "TextArea"],
  "settings/components": ["Field", "ListField"],
  "ui/select/ComboBox": ["ListComboBox"],
};

/** The props that hand a field a refusal. */
const REFUSALS = ["validate", "error"];

/** Whether a spread may carry a refusal: anything but an object written in place that names
 *  neither prop and spreads nothing (`{...{ [RENAMES_ON_F2]: "" }}`). */
function mayRefuse(spread: ts.Expression, source: ts.SourceFile): boolean {
  let at = spread;
  while (ts.isParenthesizedExpression(at)) at = at.expression;
  if (!ts.isObjectLiteralExpression(at)) return true;
  return at.properties.some(
    (one) =>
      ts.isSpreadAssignment(one) ||
      ts.isShorthandPropertyAssignment(one) ||
      (one.name !== undefined &&
        !ts.isComputedPropertyName(one.name) &&
        REFUSALS.includes(one.name.getText(source).replace(/["']/g, ""))),
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

/** What `files` say about refusals: each file that draws a field that can refuse, and each
 *  element that forwards a refusal from a component not listed in `DRAWS_A_FIELD`. */
function read(files: { name: string; text: string }[]): {
  refusing: string[];
  forwarders: string[];
} {
  const refusing = new Set<string>();
  const forwarders: string[] = [];
  for (const file of files) {
    const module = file.name.replace(/\.tsx?$/, "");
    const own = DRAWS_A_FIELD[module] ?? [];
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
      const names = from === undefined ? undefined : DRAWS_A_FIELD[from];
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
        const handed = node.attributes.properties.filter((one) =>
          ts.isJsxSpreadAttribute(one)
            ? mayRefuse(one.expression, source)
            : ts.isJsxAttribute(one) && REFUSALS.includes(one.name.getText(source)),
        );
        // A refusal handed on from the component's own props, rather than decided here.
        const forwarded = handed.some(
          (one) => ts.isJsxSpreadAttribute(one) || /\bprops\b/.test(one.getText(source)),
        );
        const inside = drawnIn(node);
        if (forwarded && inside !== undefined && own.includes(inside)) {
          // The wrapper's own forwarding: its callers are read where they stand.
        } else if (forwarded && !own.includes(inside ?? "")) {
          const tag = node.tagName.getText(source);
          if (/\bprops\b/.test(handed.map((one) => one.getText(source)).join(" ")))
            forwarders.push(`${file.name}: ${inside ?? "?"} hands <${tag}> a refusal`);
          // A spread may carry one: the call is read as refusing where it stands.
          refusing.add(file.name);
        } else if (handed.length > 0) refusing.add(file.name);
      }
      ts.forEachChild(node, visit);
    };
    visit(source);
  }
  return { refusing: [...refusing].sort(), forwarders };
}

/** The grounds the manifest lists for a field's error. */
function listed(source: string): string[] | undefined {
  const list = /\nconst FIELD_ERROR_GROUNDS: &\[&str\] = &\[([^\]]*)\];/.exec(source);
  return list === null ? undefined : [...list[1].matchAll(/"([^"]+)"/g)].map(([, one]) => one);
}

describe("a field that can refuse", () => {
  it("is found where a field is given a validate or an error, under any name it is imported by", () => {
    expect(
      read([
        {
          name: "A.tsx",
          text: `import { TextField } from "./ui/field/Field";
            export const A = () => <TextField validate={check} />;`,
        },
        {
          name: "views/B.tsx",
          text: `import { TextArea as Box } from "../ui/field/Field";
            export const B = () => <Box error={said} />;`,
        },
        {
          name: "C.tsx",
          text: `import { TextField } from "./ui/field/Field";
            export const C = () => <TextField isInvalid={wrong} />;`,
        },
        {
          name: "D.tsx",
          text: `export const D = () => <TextField validate={check} />;`,
        },
        {
          name: "I.tsx",
          text: `import { TextField } from "./ui/field/Field";
            export const I = () => <TextField {...{ [KEY]: "", title: "x" }} />;`,
        },
        {
          name: "J.tsx",
          text: `import { TextField } from "./ui/field/Field";
            export const J = () => <TextField {...{ validate: check }} />;`,
        },
      ]).refusing,
    ).toEqual(["A.tsx", "J.tsx", "views/B.tsx"]);
  });

  it("is found where a wrapper that forwards a refusal is given one, or a spread", () => {
    expect(
      read([
        {
          name: "views/E.tsx",
          text: `import { Field } from "../settings/components";
            export const E = () => <Field kind="text" validate={check} />;`,
        },
        {
          name: "settings/F.tsx",
          text: `import { Field as Row } from "./components";
            export const F = () => <Row kind="text" {...given} />;`,
        },
        {
          name: "G.tsx",
          text: `import * as Pick from "./ui/select/ComboBox";
            export const G = () => <Pick.ListComboBox error={said} />;`,
        },
        {
          name: "H.tsx",
          text: `import { Field } from "./settings/components";
            export const H = () => <Field kind="text" value={v} />;`,
        },
        {
          name: "settings/components.tsx",
          text: `import { TextField } from "../ui/field/Field";
            export function Field(props) { return <TextField validate={props.validate} />; }`,
        },
      ]),
    ).toEqual({ refusing: ["G.tsx", "settings/F.tsx", "views/E.tsx"], forwarders: [] });
  });

  it("fails a wrapper that forwards a refusal and is not listed", () => {
    expect(
      read([
        {
          name: "Mine.tsx",
          text: `import { TextField } from "./ui/field/Field";
            export function Mine(props) { return <TextField validate={props.validate} />; }`,
        },
      ]).forwarders,
    ).toEqual(["Mine.tsx: Mine hands <TextField> a refusal"]);
  });

  it("is never forwarded by a component the list does not name", () => {
    expect(read(sources(SRC)).forwarders).toEqual([]);
  });

  it("is drawn only in the files listed, each with the grounds it stands on", () => {
    expect(read(sources(SRC)).refusing).toEqual(Object.keys(REFUSING).sort());
  });

  it("has its error held on exactly the grounds the parts manifest lists for it", () => {
    const grounds = new Set([WHOLE_COPY, ...Object.values(REFUSING).flatMap((one) => one.grounds)]);
    expect(listed(readFileSync(PARTS, "utf8"))?.sort()).toEqual([...grounds].sort());
  });
});
