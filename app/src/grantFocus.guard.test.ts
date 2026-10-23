// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync, readdirSync } from "node:fs";
import { join, relative } from "node:path";
import ts from "typescript";
import { describe, expect, it } from "vitest";

/**
 * **A grant never takes the keyboard as it is drawn** (#2980, spec #2897): a confirm row, an ask
 * and a dialog start on their safe choice (Cancel, Keep blocked, Not now), so a key pressed by
 * habit never answers yes. The grant stays a button the keyboard reaches and presses on purpose.
 *
 * The dialog family holds it by construction (`ui/dialog/Dialog.tsx`: a grant asked to start on
 * starts on Cancel), and its tests prove it. This reads every grant call site in the window, every
 * `Button` whose variant can be `grants` and every dialog whose main action can be one, and fails
 * where one is given the keyboard as it is drawn: `autoFocus`, a ref the code focuses, or the
 * dialog's `initialFocus`.
 *
 * It also holds every alert dialog to Cancel, except where a ruling puts the keyboard on another
 * answer, none of them a grant ({@link RULED}).
 */

const SRC = join(process.cwd(), "src");

/**
 * **The alert dialogs that open on another answer, by ruling**: none is a grant, each answers a
 * question about a chat or a task the person asked to end.
 */
const RULED: Record<string, string> = {
  // Smart close, or Close, for a chat with turns behind it (ADR 0064); Cancel where purlis
  // cannot say.
  "EndingChat.tsx": "the default way to end a chat",
  // Stop and get its report, where a report is offered (V100-18).
  "TaskEnd.tsx": "stop a task and get its report",
};

function files(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return files(path);
    return /\.tsx$/.test(entry.name) && !/\.(test|stories)\.tsx$/.test(entry.name) ? [path] : [];
  });
}

type Element = ts.JsxOpeningElement | ts.JsxSelfClosingElement;

/** Every JSX element of `source` named one of `tags`. */
function elements(source: ts.SourceFile, tags: readonly string[]): Element[] {
  const found: Element[] = [];
  const visit = (node: ts.Node) => {
    if (
      (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) &&
      tags.includes(node.tagName.getText(source))
    )
      found.push(node);
    ts.forEachChild(node, visit);
  };
  visit(source);
  return found;
}

/** The attribute `name` of `element`, as written, or `undefined`. */
function attribute(element: Element, name: string, source: ts.SourceFile): string | undefined {
  for (const one of element.attributes.properties)
    if (ts.isJsxAttribute(one) && one.name.getText(source) === name)
      return one.initializer?.getText(source) ?? "true";
  return undefined;
}

/** The ref of a grant: `ref={x}` on the button, or `ref: x` in a dialog's answer. */
const refIn = (written: string | undefined) =>
  written?.match(/^\{\s*([\w.]+)\s*\}$/)?.[1] ?? written?.match(/\bref:\s*([\w.]+)/)?.[1];

/** Where `file` gives a grant the keyboard as it is drawn, as sentences. */
export function grantsFocused(file: string, text: string): string[] {
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const said: string[] = [];
  const focused = (ref: string) =>
    new RegExp(`\\b${ref.replace(".", "\\.")}\\.current\\??\\.focus\\(`).test(text) ||
    new RegExp(`initialFocus=\\{\\s*${ref.replace(".", "\\.")}\\s*\\}`).test(text);
  for (const button of elements(source, ["Button"])) {
    if (!/"grants"/.test(attribute(button, "variant", source) ?? "")) continue;
    const line = source.getLineAndCharacterOfPosition(button.getStart(source)).line + 1;
    if (attribute(button, "autoFocus", source) !== undefined)
      said.push(`${file}:${line}: a grant drawn with autoFocus`);
    const ref = refIn(attribute(button, "ref", source));
    if (ref !== undefined && focused(ref))
      said.push(`${file}:${line}: a grant whose ref ${ref} is given the keyboard`);
  }
  for (const dialog of elements(source, ["Dialog", "AlertDialog"])) {
    const main = attribute(dialog, "main", source) ?? "";
    if (!/"grants"/.test(main)) continue;
    const line = source.getLineAndCharacterOfPosition(dialog.getStart(source)).line + 1;
    const start = attribute(dialog, "initialFocus", source) ?? "";
    const ref = refIn(main);
    if (start === '"main"' || (ref !== undefined && focused(ref)))
      said.push(`${file}:${line}: a dialog that starts on its grant`);
  }
  return said;
}

/** Where `file` opens an alert dialog on another answer than Cancel. */
export function alertsNotOnCancel(file: string, text: string): string[] {
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  return elements(source, ["AlertDialog"])
    .filter((alert) => {
      const start = attribute(alert, "initialFocus", source);
      // Cancel, the dialog itself while it is read, or the field a confirm by name is typed in.
      return start !== undefined && !['"cancel"', '"dialog"', '"field"'].includes(start);
    })
    .map((alert) => {
      const line = source.getLineAndCharacterOfPosition(alert.getStart(source)).line + 1;
      return `${file}:${line}`;
    });
}

const sources = () =>
  files(SRC).map((path) => ({ file: relative(SRC, path), text: readFileSync(path, "utf8") }));

describe("a grant never takes the keyboard as it is drawn (#2980)", () => {
  it("is found where a grant is focused by autoFocus, a ref or a dialog's initialFocus", () => {
    expect(
      grantsFocused(
        "Row.tsx",
        `function Row() {
          const yes = useRef(null);
          useEffect(() => yes.current?.focus(), []);
          return <><Button variant={on ? "grants" : "secondary"} ref={yes}>Allow</Button>
            <Button variant="grants" autoFocus>Trust it</Button></>;
        }`,
      ),
    ).toEqual([
      "Row.tsx:4: a grant whose ref yes is given the keyboard",
      "Row.tsx:5: a grant drawn with autoFocus",
    ]);
    expect(
      grantsFocused(
        "Ask.tsx",
        `<Dialog initialFocus="main" main={{ children: "Allow", variant: "grants" }} />`,
      ),
    ).toEqual(["Ask.tsx:1: a dialog that starts on its grant"]);
    // A grant the keyboard reaches on purpose, beside a Cancel that has it, is the rule.
    expect(
      grantsFocused(
        "Fine.tsx",
        `function Fine() {
          const no = useRef(null);
          useEffect(() => no.current?.focus(), []);
          return <><Button ref={no}>Cancel</Button><Button variant="grants">Allow</Button></>;
        }`,
      ),
    ).toEqual([]);
  });

  it("holds at every grant call site in the window", () => {
    const sites = sources().filter(({ text }) => /"grants"/.test(text));
    expect(sites.length).toBeGreaterThan(5);
    expect(sites.flatMap(({ file, text }) => grantsFocused(file, text))).toEqual([]);
  });

  it("opens every alert dialog on Cancel, but for the answers a ruling names", () => {
    const opened = sources().flatMap(({ file, text }) => alertsNotOnCancel(file, text));
    expect(opened.map((at) => at.replace(/:\d+$/, "")).sort()).toEqual(Object.keys(RULED).sort());
  });
});
