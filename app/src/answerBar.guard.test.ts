// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync, readdirSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";
import { RADIX_FROM } from "./guards.testkit";

/**
 * **Every question dialog is answered from one bar** (DS-3e's follow-up, #1210, D-1210-1).
 *
 * A question with a way out and an act or two, and nothing to fill in, ends in an `AnswerBar`
 * (`AnswerBar.tsx`). Before it, two hand-built rows drew the same buttons with two layouts:
 * `answer`, right-aligned with a gap, and `doing`, with no layout of its own, whose buttons sat
 * wherever the dialog's flow put them. This fails on a dialog that builds either row by hand
 * again: `answer` is named by no `className` but the bar's own, and `doing` by none in a file that
 * draws a Radix dialog. (`doing` stays a word elsewhere: a tab chip's `<span>` saying what a task
 * is doing, the opener page's row, a tab's own buttons. None of those is a dialog's answer, so
 * only a `<div>` row in a dialog's file counts.)
 *
 * The source is read as text, as `settings/oldFormClasses.test.ts` reads it.
 */

const SRC = join(process.cwd(), "src");

/** Where the bar is drawn, and the one file allowed to name its class. */
const BAR = "AnswerBar.tsx";

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
  files(SRC, (name) => /\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name)).map((path) => ({
    name: relative(SRC, path),
    text: readFileSync(path, "utf8"),
  }));

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

/** Whether a file draws a dialog: it imports one of Radix's two. */
export const drawsADialog = (source: string) =>
  new RegExp(String.raw`from\s+["']${RADIX_FROM}(?:alert-)?dialog["']`).test(source);

/** What a file builds by hand that the bar draws, as `file: class`. */
export function handBuilt({ name, text }: { name: string; text: string }): string[] {
  if (name === BAR) return [];
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
 * **What ends something is asked, never filled in** (D-1210-8). A confirm whose only field is the
 * typed name of what it ends is a question, so it ends in the bar, the act last; `SettingActions`
 * ends the forms that make or change something (V89j). A dialog's file that puts an `ends-it` act
 * in a form's row is drawing a confirm as a form, and this names it, as `file: ends-it`.
 */
export function endsInAForm({ name, text }: { name: string; text: string }): string[] {
  if (!drawsADialog(text)) return [];
  return formRows(text)
    .flatMap((row) => [
      ...classNames(row).filter((one) => one === "ends-it"),
      // The button family's word for it (#2904): `variant="danger"`, or a variant an expression
      // picks that can be it.
      ...[...row.matchAll(/\bvariant=(?:"danger"|\{[^}]*"danger"[^}]*\})/g)].map(() => "danger"),
    ])
    .map((one) => `${name}: ${one}`);
}

describe("a question dialog's answer bar", () => {
  it("is built by hand in no dialog", () => {
    const all = sources();
    // The bar itself and a dialog or two at least: a reader that found none would pass on nothing.
    expect(all.map(({ name }) => name)).toEqual(
      expect.arrayContaining([BAR, "QuitWarning.tsx", "DeleteWorkspace.tsx"]),
    );
    expect(all.flatMap(handBuilt)).toEqual([]);
  });

  it("ends every dialog's confirm that ends something, never a form's row", () => {
    const all = sources();
    expect(all.map(({ name }) => name)).toContain("DeleteVault.tsx");
    expect(all.flatMap(endsInAForm)).toEqual([]);
  });

  it("is still drawn by the bar, with the class the stylesheet lays out", () => {
    const bar = readFileSync(join(SRC, BAR), "utf8");
    expect(classNames(bar)).toContain("answer");
    expect(readFileSync(join(SRC, "App.css"), "utf8")).toMatch(/\n\.answer \{[^}]*display: flex/);
  });

  /**
   * **What cannot be taken back keeps its danger under the pointer** (#1210). The bar's answers
   * are the button family's (#2904), so the tint is the danger variant's own, in the family's
   * stylesheet; jsdom computes no hover, so this reads the rule.
   */
  it("tints a destructive answer with the danger surface under the pointer", () => {
    const family = readFileSync(join(SRC, "ui/button/button.css"), "utf8");
    const hover =
      /\n\[data-scope="button"\]\[data-part="root"\]\[data-variant="danger"\]\[data-hovered\] \{([^}]*)\}/.exec(
        family,
      )?.[1] ?? "";
    expect(hover).toMatch(/background-color:\s*var\(--danger-surface\)/);
    // Nothing is left styling a row no dialog draws, nor a bar's button by hand.
    const css = readFileSync(join(SRC, "App.css"), "utf8");
    expect(css).not.toMatch(/\.warning \.doing/);
    expect(css).not.toMatch(/\n\.answer button/);
  });

  it("would be caught if one came back", () => {
    // The reader above is what the guard rests on, so it is held to the shapes a dialog writes.
    const dialog = `import * as AlertDialog from "@radix-ui/react-alert-dialog";\n`;
    expect(handBuilt({ name: "X.tsx", text: `${dialog}<div className="doing" />` })).toEqual([
      "X.tsx: doing",
    ]);
    expect(handBuilt({ name: "X.tsx", text: `<div className="answer" />` })).toEqual([
      "X.tsx: answer",
    ]);
    expect(
      handBuilt({
        name: "X.tsx",
        text: `import { Dialog } from "@radix-ui/react-dialog";\n<div className={on ? "doing" : "x"}>`,
      }),
    ).toEqual(["X.tsx: doing"]);
    // A tab chip's line of what a task is doing is no dialog's answer, even beside a dialog.
    expect(
      handBuilt({ name: "TabChip.tsx", text: `${dialog}<span className="doing">x</span>` }),
    ).toEqual([]);
    // Nor is the opener page's row, which draws no dialog.
    expect(handBuilt({ name: "Opener.tsx", text: `<div className="doing">` })).toEqual([]);
    // `answer-question` is a different class, and `answer` alone is the bar's.
    expect(handBuilt({ name: "X.tsx", text: `<div className="answer-question" />` })).toEqual([]);
  });

  it("would catch a dialog's confirm drawn as a form again", () => {
    const dialog = `import * as AlertDialog from "@radix-ui/react-alert-dialog";\n`;
    const row = (inside: string) => `<SettingActions>\n${inside}\n</SettingActions>`;
    const ends = `<button type="submit" className="ends-it" tabIndex={0}>Delete</button>`;
    // DeleteVault's old shape: the act first, inside the form's row.
    expect(endsInAForm({ name: "X.tsx", text: `${dialog}${row(ends)}` })).toEqual([
      "X.tsx: ends-it",
    ]);
    // Last in the row is still a form's row, and a class held in an expression still counts.
    expect(
      endsInAForm({
        name: "X.tsx",
        text: `import { Dialog } from "@radix-ui/react-dialog";\n${row(
          `<button>Cancel</button>\n<button className={busy ? "ends-it busy" : "ends-it"}>Go</button>`,
        )}`,
      }),
    ).toEqual(["X.tsx: ends-it", "X.tsx: ends-it"]);
    // In the bar it is where it belongs.
    expect(endsInAForm({ name: "X.tsx", text: `${dialog}<AnswerBar>${ends}</AnswerBar>` })).toEqual(
      [],
    );
    // A form's row that makes something, beside a confirm drawn in the bar, is a form.
    expect(
      endsInAForm({
        name: "X.tsx",
        text: `${dialog}${row(`<button type="submit">Rename</button>`)}<AnswerBar>${ends}</AnswerBar>`,
      }),
    ).toEqual([]);
    // A row given props, even one whose expression holds a `>`, or built by hand, is still a row.
    expect(
      endsInAForm({
        name: "X.tsx",
        text: `${dialog}<SettingActions trouble={busy ? <b>x</b> : null} onX={() => a > b}>\n${ends}\n</SettingActions>`,
      }),
    ).toEqual(["X.tsx: ends-it"]);
    expect(
      endsInAForm({
        name: "X.tsx",
        text: `${dialog}<div className="ui-setting-actions">\n${ends}\n</div>`,
      }),
    ).toEqual(["X.tsx: ends-it"]);
    // A page, not a dialog, keeps its own rows: NotCloned and WorkspaceRepos draw none.
    expect(endsInAForm({ name: "NotCloned.tsx", text: row(ends) })).toEqual([]);
    // The button family's danger counts as the class did (#2904).
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
        text: `${dialog}<AnswerBar><Button variant="danger">Delete</Button></AnswerBar>`,
      }),
    ).toEqual([]);
  });
});
