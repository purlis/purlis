// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **What the design guards read the source with** (#1837): the window's files, its CSS as
 * declarations, and its JSX as opening tags. `geometry.guard.test.ts` and
 * `controls.guard.test.ts` share these, so the two guards see the same tree the same way.
 *
 * Each reader is a small scanner rather than a regex over the whole text, because the thing a
 * regex gets wrong is exactly what a guard must not: a `;` inside a data URL, a `{` inside a
 * string, a `>` inside an arrow function in a tag's props.
 */
import { readdirSync } from "node:fs";
import { join, relative } from "node:path";

export const SRC = join(process.cwd(), "src");

/** Every file under `src` whose name `keep` takes, as its path from `src`, sorted. */
export function sourceFiles(keep: (name: string) => boolean): string[] {
  const walk = (dir: string): string[] =>
    readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) return walk(path);
      return keep(entry.name) ? [relative(SRC, path)] : [];
    });
  return walk(SRC).sort();
}

/** A component file the window is drawn from, where JSX is: not a test or a test kit, which
 *  draw freely. */
export const isView = (name: string) =>
  name.endsWith(".tsx") && !/\.(test|testkit)\.tsx$/.test(name);

/** A source the window runs, components and scripts alike (a script may `setProperty`), not a
 *  test, a test kit or a declaration file. */
export const isScript = (name: string) =>
  /\.tsx?$/.test(name) && !/\.(test|testkit)\.tsx?$/.test(name) && !name.endsWith(".d.ts");

/** `text` with its comments blanked, keeping every newline, so a doc comment that names
 *  `<button>` or `padding: 7px` is not read as one. `https://` in a string is not a comment. */
export function withoutComments(text: string, kind: "css" | "ts"): string {
  const blank = (comment: string) => comment.replace(/[^\n]/g, " ");
  let stripped = text.replace(/\/\*[\s\S]*?\*\//g, blank);
  if (kind === "ts")
    stripped = stripped.replace(/(^|[^:])(\/\/[^\n]*)/gm, (_, a, c) => a + blank(c));
  return stripped;
}

/** One declaration of a stylesheet: the rules around it (outermost first), and what it sets. */
export interface Declaration {
  context: string[];
  property: string;
  value: string;
}

const squash = (text: string) => text.trim().replace(/\s+/g, " ");

/**
 * Every declaration in a stylesheet, with the selectors and at-rules it sits in, comments out.
 * Strings and brackets are skipped whole, so `url("data:…;…")` is one value; nesting and
 * `@media` are followed, so a declaration inside either is still found. A declaration outside
 * every block (an `@import`) is not one.
 */
export function declarations(css: string): Declaration[] {
  const text = withoutComments(css, "css");
  const found: Declaration[] = [];
  const context: string[] = [];
  let buffer = "";
  let depth = 0;
  const flush = () => {
    const colon = buffer.indexOf(":");
    if (context.length > 0 && colon > 0) {
      found.push({
        context: [...context],
        property: squash(buffer.slice(0, colon)).toLowerCase(),
        value: squash(buffer.slice(colon + 1)),
      });
    }
    buffer = "";
  };
  for (let i = 0; i < text.length; i++) {
    const char = text[i];
    if (char === '"' || char === "'") {
      const end = closingQuote(text, i);
      buffer += text.slice(i, end + 1);
      i = end;
    } else if (char === "(") {
      depth++;
      buffer += char;
    } else if (char === ")") {
      depth = Math.max(0, depth - 1);
      buffer += char;
    } else if (depth > 0) {
      buffer += char;
    } else if (char === "{") {
      context.push(squash(buffer));
      buffer = "";
    } else if (char === ";") {
      flush();
    } else if (char === "}") {
      if (buffer.trim()) flush();
      buffer = "";
      context.pop();
    } else {
      buffer += char;
    }
  }
  return found;
}

/** Where the string opened at `start` closes, a backslash escaping the next character. */
function closingQuote(text: string, start: number): number {
  const quote = text[start];
  for (let i = start + 1; i < text.length; i++) {
    if (text[i] === "\\") i++;
    else if (text[i] === quote) return i;
  }
  return text.length - 1;
}

/** One JSX opening tag: its name, its whole text (`<button … >`), and where it starts. */
export interface Tag {
  name: string;
  text: string;
  index: number;
}

/**
 * Every opening tag in a component's source whose name `names` matches, comments out. The tag
 * runs to the `>` outside every brace and string, so an arrow function in a prop
 * (`onClick={() => a > b}`) does not end it early.
 */
export function openingTags(source: string, names: RegExp): Tag[] {
  const text = withoutComments(source, "ts");
  const tags: Tag[] = [];
  for (const hit of text.matchAll(/<([A-Za-z][\w.]*)(?=[\s/>])/g)) {
    if (!names.test(hit[1])) continue;
    let depth = 0;
    let end = hit.index + hit[0].length;
    for (; end < text.length; end++) {
      const char = text[end];
      if (char === '"' || char === "'" || char === "`") end = closingQuote(text, end);
      else if (char === "{") depth++;
      else if (char === "}") depth--;
      else if (char === ">" && depth === 0) break;
    }
    tags.push({ name: hit[1], text: text.slice(hit.index, end + 1), index: hit.index });
  }
  return tags;
}

/** A tag's `className` as written: the string, or the expression in its braces, squashed to
 *  one line. `undefined` when the tag has none. */
export function classNameOf(tag: string): string | undefined {
  const at = /\sclassName=/.exec(tag);
  if (!at) return undefined;
  const start = at.index + at[0].length;
  const open = tag[start];
  if (open === '"' || open === "'") return squash(tag.slice(start + 1, closingQuote(tag, start)));
  if (open !== "{") return undefined;
  let depth = 0;
  for (let i = start; i < tag.length; i++) {
    const char = tag[i];
    if (char === '"' || char === "'" || char === "`") i = closingQuote(tag, i);
    else if (char === "{") depth++;
    else if (char === "}" && --depth === 0) return squash(tag.slice(start + 1, i));
  }
  return undefined;
}

/** The class names a `className` can hand the DOM: every word in it that could be one. An
 *  expression's identifiers come along too, which only matters if one is named like a class. */
export function classWords(className: string | undefined): string[] {
  return className ? className.split(/[^\w.:-]+/).filter(Boolean) : [];
}

/** The utilities a `className` can hand Tailwind, as `classWords` but with whatever sits in an
 *  arbitrary value's or a variant's brackets kept on its word: `p-[7px]`,
 *  `m-[calc(var(--space-1)*3)]`, `[padding:7px]`, `[&>*]:p-[7px]`, `p-(--pad)`. */
export function classUtilities(className: string | undefined): string[] {
  const words: string[] = [];
  let word = "";
  let depth = 0;
  for (const char of className ?? "") {
    if (char === "[") depth++;
    else if (char === "]") depth = Math.max(0, depth - 1);
    if (depth === 0 && /[\s"'`{},]/.test(char)) {
      if (word) words.push(word);
      word = "";
    } else word += char;
  }
  if (word) words.push(word);
  return words;
}

/** A Tailwind class without its variants and its `!`: what follows the last `:` outside every
 *  bracket and parenthesis, so `md:p-5`, `data-[open]:p-5`, `max-[600px]:p-5`, `*:p-5`,
 *  `group-hover/x:p-5` and `[&>*]:p-5` are each `p-5`. */
export function utilityOf(word: string): string {
  let depth = 0;
  let from = 0;
  for (let i = 0; i < word.length; i++) {
    const char = word[i];
    if (char === "[" || char === "(") depth++;
    else if (char === "]" || char === ")") depth = Math.max(0, depth - 1);
    else if (char === ":" && depth === 0) from = i + 1;
  }
  return word.slice(from).replace(/^!|!$/g, "");
}

/** A value a component writes out: a number, or a string with nothing interpolated. */
const WRITTEN = String.raw`-?\d*\.?\d+(?![\w.])|"[^"]*"|'[^']*'|` + "`[^`$]*`";
const ENTRY = new RegExp(String.raw`^\s*(\w+|"--[\w-]+"|'--[\w-]+')\s*:\s*(${WRITTEN})\s*$`);
const SET_PROPERTY = new RegExp(
  String.raw`\.style\.setProperty\(\s*("[^"]*"|'[^']*'|` +
    "`[^`$]*`" +
    String.raw`)\s*,\s*(${WRITTEN})\s*[,)]`,
  "g",
);

/** Where the bracket, brace or parenthesis opened at `start` closes, strings skipped whole. */
function closingBracket(text: string, start: number): number {
  let depth = 0;
  for (let i = start; i < text.length; i++) {
    const char = text[i];
    if (char === '"' || char === "'" || char === "`") i = closingString(text, i);
    else if ("{[(".includes(char)) depth++;
    else if ("}])".includes(char) && --depth === 0) return i;
  }
  return text.length - 1;
}

/** Where the string opened at `start` closes, a template literal's `${…}` skipped whole. */
function closingString(text: string, start: number): number {
  const quote = text[start];
  for (let i = start + 1; i < text.length; i++) {
    if (text[i] === "\\") i++;
    else if (quote === "`" && text[i] === "$" && text[i + 1] === "{")
      i = closingBracket(text, i + 1);
    else if (text[i] === quote) return i;
  }
  return text.length - 1;
}

/** `body` cut at its top-level commas. */
function topLevel(body: string): string[] {
  const parts: string[] = [];
  let from = 0;
  for (let i = 0; i < body.length; i++) {
    const char = body[i];
    if (char === '"' || char === "'" || char === "`") i = closingString(body, i);
    else if ("{[(".includes(char)) i = closingBracket(body, i);
    else if (char === ",") {
      parts.push(body.slice(from, i));
      from = i + 1;
    }
  }
  parts.push(body.slice(from));
  return parts;
}

export interface StyleEntry {
  /** How the complaint names it: `style paddingTop`, `setProperty --pad`. */
  label: string;
  property: string;
  value: string;
}

/** Each property a component sets that is written out: an entry of an inline `style` object
 *  (`{{ … }}`, `{ … } as CSSProperties`, over as many lines as it takes), and a
 *  `.style.setProperty(…)` call, as the property it sets: `paddingTop` is `padding-top`, and a
 *  custom property (`"--least"`) is its own name. A spread, or a value worked out, is not
 *  read. */
export function styleEntries(source: string): StyleEntry[] {
  const found: StyleEntry[] = [];
  const text = withoutComments(source, "ts");
  for (const hit of text.matchAll(/\bstyle=\{\s*\{/g)) {
    const open = hit.index + hit[0].length - 1;
    for (const part of topLevel(text.slice(open + 1, closingBracket(text, open)))) {
      const [, key, value] = ENTRY.exec(part) ?? [];
      if (key === undefined) continue;
      const custom = /^["']/.test(key);
      const property = custom
        ? key.slice(1, -1)
        : key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);
      found.push({ label: `style ${custom ? property : key}`, property, value });
    }
  }
  for (const [, name, value] of text.matchAll(SET_PROPERTY)) {
    const property = name.slice(1, -1);
    found.push({ label: `setProperty ${property}`, property, value });
  }
  return found;
}

/** The line `index` is on, the way an editor jumps to it. */
export function lineOf(text: string, index: number): number {
  return text.slice(0, index).split("\n").length;
}

/**
 * **The ratchet both guards share**: what differs between `found` (each file's offenders in the
 * source) and `allowed` (each file's allowance list), counted per entry, so one offender written
 * twice needs two lines.
 * - An offender not on the list is new, and fails (`words.added` says what to do instead).
 * - An entry with nothing left to name is stale, and fails until it is deleted: the list stays
 *   exact, so it can only get shorter as the source is fixed.
 * - A list longer than its file's frozen `ceiling` grew, and fails; a file with no ceiling may
 *   have no list.
 */
export function ratchet(
  found: ReadonlyMap<string, readonly string[]>,
  allowed: Readonly<Record<string, readonly string[]>>,
  ceiling: Readonly<Record<string, number>>,
  words: { added: string },
): string[] {
  const complaints: string[] = [];
  const files = new Set([...found.keys(), ...Object.keys(allowed)]);
  for (const file of [...files].sort()) {
    const left = new Map<string, number>();
    for (const entry of allowed[file] ?? []) left.set(entry, (left.get(entry) ?? 0) + 1);
    for (const entry of found.get(file) ?? []) {
      const n = left.get(entry) ?? 0;
      if (n > 0) left.set(entry, n - 1);
      else complaints.push(`${file}: ${words.added}: ${entry}`);
    }
    for (const [entry, n] of left) {
      for (let i = 0; i < n; i++) {
        complaints.push(`${file}: stale entry, delete it from the allowance: ${entry}`);
      }
    }
    const length = allowed[file]?.length ?? 0;
    if (length > (ceiling[file] ?? 0)) {
      complaints.push(`${file}: the allowance grew to ${length}, past ${ceiling[file] ?? 0}`);
    }
  }
  return complaints;
}

/**
 * **Where a Radix package is imported from**, as a pattern for the start of an import's source,
 * up to the package's own name (`dialog`, `dropdown-menu`): the package itself, or the window's
 * file of the same name in `overlays/`, through which every overlay is written
 * (`projectShown.guard.test.tsx`). A guard that finds a file's dialogs or menus by what it
 * imports reads both, so moving an import there hides nothing from it.
 */
export const RADIX_FROM = String.raw`(?:@radix-ui\/react-|(?:\.\.?\/)+overlays\/)`;
