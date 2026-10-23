// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync, readdirSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * **Every dialog is answered from its footer** (#2910, which replaced the answer bar of #1210,
 * D-1210-1).
 *
 * A dialog's answers are the dialog family's props (`ui/dialog/Dialog.tsx`): its way out, one more
 * and its main action, drawn in its footer in their places. This fails on a dialog that builds an
 * answer row by hand again, `answer` or `doing`, and on a dialog that puts an act that cannot be
 * taken back in a form's `SettingActions`: a confirm whose only field is the typed name of what
 * it ends is a question, and its delete is the footer's main action (D-1210-8).
 *
 * The source is read as text, as `settings/oldFormClasses.test.ts` reads it.
 */

const SRC = join(process.cwd(), "src");

/** Every file under `dir` whose name `keep` takes. */
function files(dir: string, keep: (name: string) => boolean): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return files(path, keep);
    return keep(entry.name) ? [path] : [];
  });
}

/** Every source file the window is drawn from: not tests, which name the classes to find them. */
const sources = () =>
  files(SRC, (name) => /\.tsx?$/.test(name) && !/\.(test|stories)\.tsx?$/.test(name)).map(
    (path) => ({ name: relative(SRC, path), text: readFileSync(path, "utf8") }),
  );

/** The class names a `className` attribute can hold: every string in its value. */
export function classNames(source: string): string[] {
  const values = [
    ...source.matchAll(/className=(?:"([^"]*)"|\{([^{}]*(?:\{[^{}]*\}[^{}]*)*)\})/g),
  ].flatMap(([, plain, expression]) =>
    plain !== undefined
      ? [plain]
      : [...(expression ?? "").matchAll(/"([^"]*)"|'([^']*)'|`([^`]*)`/g)].map(([, a, b, c]) =>
          (a ?? b ?? c ?? "").replace(/\$\{[^}]*\}/g, " "),
        ),
  );
  return values.flatMap((value) => value.split(/\s+/)).filter(Boolean);
}

/** The opening `<div …>` tags in `source`: a row of buttons is a `div`, a line of text is not. */
const divs = (source: string) => [...source.matchAll(/<div\b[^>]*>/g)].map(([tag]) => tag);

/** Whether a file draws a dialog: it imports the dialog family. */
export const drawsADialog = (source: string) =>
  /from\s+["'](?:\.\.?\/)+(?:ui\/)?dialog\/Dialog["']/.test(source) ||
  /from\s+["']\.\/Dialog["']/.test(source);

/** What a file builds by hand that the footer draws, as `file: class`. */
export function handBuilt({ name, text }: { name: string; text: string }): string[] {
  const named = classNames(text);
  return [
    ...named.filter((one) => one === "answer"),
    ...(drawsADialog(text)
      ? divs(text)
          .flatMap(classNames)
          .filter((one) => one === "doing")
      : []),
  ].map((one) => `${name}: ${one}`);
}

/**
 * The text of every form's row in `source`: a `<SettingActions …>` element, whatever its props
 * (an expression's `>` included), or a `div` that names its class by hand.
 */
const formRows = (source: string) =>
  [
    ...source.matchAll(
      /<SettingActions\b(?:[^>{}]|\{(?:[^{}]|\{[^{}]*\})*\})*>([\s\S]*?)<\/SettingActions>/g,
    ),
    ...source.matchAll(/<div\b[^>]*\bui-setting-actions\b[^>]*>([\s\S]*?)<\/div>/g),
  ].map(([, row]) => row ?? "");

/**
 * **What ends something is asked, never filled in** (D-1210-8): a dialog's file that puts a
 * `danger` act, or the old `ends-it`, in a form's row is drawing a confirm as a form, and this
 * names it, as `file: danger`.
 */
export function endsInAForm({ name, text }: { name: string; text: string }): string[] {
  if (!drawsADialog(text)) return [];
  return formRows(text)
    .flatMap((row) => [
      ...classNames(row).filter((one) => one === "ends-it"),
      ...[...row.matchAll(/\bvariant=(?:"danger"|\{[^}]*"danger"[^}]*\})/g)].map(() => "danger"),
    ])
    .map((one) => `${name}: ${one}`);
}

describe("a dialog's answers", () => {
  it("are built by hand in no dialog", () => {
    const all = sources();
    // The family and a dialog or two at least: a reader that found none would pass on nothing.
    expect(all.map(({ name }) => name)).toEqual(
      expect.arrayContaining(["ui/dialog/Dialog.tsx", "QuitWarning.tsx", "DeleteWorkspace.tsx"]),
    );
    expect(all.filter(({ text }) => drawsADialog(text)).length).toBeGreaterThan(40);
    expect(all.flatMap(handBuilt)).toEqual([]);
  });

  it("end every confirm that ends something in the footer, never a form's row", () => {
    const all = sources();
    expect(all.map(({ name }) => name)).toContain("DeleteVault.tsx");
    expect(all.flatMap(endsInAForm)).toEqual([]);
  });

  it("are drawn by no stylesheet rule of a hand-built row", () => {
    const css = readFileSync(join(SRC, "App.css"), "utf8");
    expect(css).not.toMatch(/\n\.answer \{/);
    expect(css).not.toMatch(/\.warning \.doing/);
    expect(css).not.toMatch(/\n\.answer button/);
  });

  it("would be caught if a hand-built row came back", () => {
    const dialog = `import { AlertDialog } from "./ui/dialog/Dialog";\n`;
    expect(handBuilt({ name: "X.tsx", text: `${dialog}<div className="doing" />` })).toEqual([
      "X.tsx: doing",
    ]);
    expect(handBuilt({ name: "X.tsx", text: `<div className="answer" />` })).toEqual([
      "X.tsx: answer",
    ]);
    // A tab chip's line of what a task is doing is no dialog's answer, even beside a dialog.
    expect(
      handBuilt({ name: "TabChip.tsx", text: `${dialog}<span className="doing">x</span>` }),
    ).toEqual([]);
    // Nor is the opener page's row, which draws no dialog.
    expect(handBuilt({ name: "Opener.tsx", text: `<div className="doing">` })).toEqual([]);
    // `answer-question` is a different class.
    expect(handBuilt({ name: "X.tsx", text: `<div className="answer-question" />` })).toEqual([]);
  });

  it("would catch a dialog's confirm drawn as a form again", () => {
    const dialog = `import { Dialog } from "../ui/dialog/Dialog";\n`;
    const row = (inside: string) => `<SettingActions>\n${inside}\n</SettingActions>`;
    expect(
      endsInAForm({
        name: "X.tsx",
        text: `${dialog}${row(`<Button variant="danger" type="submit">Delete</Button>`)}`,
      }),
    ).toEqual(["X.tsx: danger"]);
    expect(
      endsInAForm({
        name: "X.tsx",
        text: `${dialog}${row(`<Button variant={busy ? "danger" : "secondary"}>Go</Button>`)}`,
      }),
    ).toEqual(["X.tsx: danger"]);
    expect(
      endsInAForm({
        name: "X.tsx",
        text: `${dialog}<div className="ui-setting-actions">\n<button className="ends-it">Go</button>\n</div>`,
      }),
    ).toEqual(["X.tsx: ends-it"]);
    // A page, not a dialog, keeps its own rows.
    expect(
      endsInAForm({ name: "NotCloned.tsx", text: row(`<Button variant="danger">Delete</Button>`) }),
    ).toEqual([]);
    // A body button that only makes something is a form's own row.
    expect(
      endsInAForm({ name: "X.tsx", text: `${dialog}${row(`<Button>Copy code</Button>`)}` }),
    ).toEqual([]);
  });
});
