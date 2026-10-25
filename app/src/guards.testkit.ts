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
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, posix, relative } from "node:path";

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

/** How a guard reads the window's other files (`StyleSource.read`): a file's text as planted
 *  (`planted`, by its path from `src`) or as it is, `undefined` when there is none. Each file is
 *  read once per reader, so make one per pass. */
export function windowFiles(
  planted: Readonly<Record<string, string>> = {},
): (file: string) => string | undefined {
  const known = new Map<string, string | undefined>();
  return (file) => {
    if (file in planted) return planted[file];
    if (!known.has(file)) {
      let text: string | undefined;
      try {
        const path = join(SRC, file);
        text = statSync(path).isFile() ? readFileSync(path, "utf8") : undefined;
      } catch {
        text = undefined;
      }
      known.set(file, text);
    }
    return known.get(file);
  };
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
const WRITTEN = String.raw`-?\d*\.?\d+(?:e[+-]?\d+)?(?![\w.])|"[^"]*"|'[^']*'|` + "`[^`$]*`";
const ENTRY = /^\s*(\w+|"--[\w-]+"|'--[\w-]+')\s*:\s*([\s\S]+?)\s*$/;
const WRITTEN_ALONE = new RegExp(String.raw`^(?:${WRITTEN})$`);
/** A string written out, with nothing interpolated. */
const STRING = String.raw`"[^"]*"|'[^']*'|` + "`[^`$]*`";
const STRING_ALONE = new RegExp(String.raw`^(?:${STRING})$`);
/** A `setProperty(…)` call, on `.style`, `.style?.` or any other declaration a script holds, up
 *  to its own `(`. */
const SET_PROPERTY = /\??\.setProperty(?:\?\.)?\(/g;
/** A name in script. */
const NAME = String.raw`[A-Za-z_$][\w$]*`;
/** Between a name and its member: `.`, `?.`, `!.`. */
const MEMBER = /\s*!?\s*\??\.\s*/;
/** A name and its members, as an expression hands them over: `held`, `styles.box`,
 *  `frame?.style`, `frame!.style`. */
const ROOT = new RegExp(String.raw`^${NAME}`);
/** One step of a reference: `.box`, `?.box`, `!.box`, `["box"]`, `['box']`, `` [`box`] ``. */
const STEP = new RegExp(
  String.raw`^(?:${MEMBER.source}(${NAME})|\s*!?\s*(?:\?\.)?\s*\[\s*(?:"([^"]*)"|'([^']*)'|` +
    "`([^`$]*)`" +
    String.raw`)\s*\])`,
);

/** A reference as its root and members (`styles["box"].row` is `styles`, `box`, `row`), or
 *  `undefined` when the expression is not one. */
function referenceOf(written: string): string[] | undefined {
  const root = ROOT.exec(written);
  if (!root) return undefined;
  const path = [root[0]];
  let rest = written.slice(root[0].length);
  while (rest.trim()) {
    const step = STEP.exec(rest);
    if (!step) return undefined;
    path.push(step[1] ?? step[2] ?? step[3] ?? step[4]);
    rest = rest.slice(step[0].length);
  }
  return path;
}

/** How far a style is followed, hop by hop, before the guard gives up and says so. */
export const FOLLOW_LIMIT = 24;
/** A call of a name or a member, up to its own `(`: `boxStyle(on)`, `React.useMemo(…)`. */
const CALL = new RegExp(String.raw`^(${NAME}(?:\s*\.\s*${NAME})*)\s*(?:<[^<>()]*>)?\s*\(`);
/** A variable declared, `const held = …`, `let held: CSSProperties = …`, or destructured,
 *  `const { style, box: held } = …`, with its type if it is given one. */
const DECLARES = new RegExp(
  String.raw`\b(?:const|let|var)\s+(${NAME}|\{[^{}=]*\})\s*(?::\s*([^=;]+?))?\s*=(?![=>])\s*`,
  "g",
);
/** A function declared by name, up to the `(` of its parameters. */
const FUNCTION = new RegExp(String.raw`\bfunction\s*\*?\s*(${NAME})\s*(?:<[^<>()]*>)?\s*\(`, "g");
/** An import a style can come through: `import { box, pad as p } from "./x"`,
 *  `import * as styles from "./x"`, `import box from "./x"`. */
const IMPORTS = new RegExp(
  String.raw`\bimport\s+(?:type\s+)?(${NAME})?\s*,?\s*(?:\*\s*as\s+(${NAME})|\{([^{}]*)\})?\s*from\s*["']([^"']+)["']`,
  "g",
);
/** A file's exports under another name, or from another file: `export { box as panel }`,
 *  `export { box } from "./x"`, `export * from "./x"`. */
const EXPORTS = /\bexport\s*(?:\{([^{}]*)\}|\*)\s*(?:from\s*["']([^"']+)["'])?/g;
/** A style object typed or cast as one. */
const TYPED = /\bCSSProperties\b/;
const CAST = /\b(?:as|satisfies)\s+(?:React\.)?CSSProperties\s*$/;

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

/** A stretch of a file's text, `from` up to `to`. */
interface Span {
  from: number;
  to: number;
}

/** Each index of `text` from `from` up to `to` outside every string and bracket, until `visit`
 *  says stop by giving a value, which this gives back. */
function atTopLevel<T>(
  text: string,
  { from, to }: Span,
  visit: (i: number) => T | undefined,
): T | undefined {
  for (let i = from; i < to; i++) {
    const char = text[i];
    const seen = visit(i);
    if (seen !== undefined) return seen;
    if (char === '"' || char === "'" || char === "`") i = closingString(text, i);
    else if ("{[(".includes(char)) i = closingBracket(text, i);
  }
  return undefined;
}

/** `span` cut at its top-level commas. */
function partsOf(text: string, span: Span): Span[] {
  const parts: Span[] = [];
  let from = span.from;
  atTopLevel(text, span, (i) => {
    if (text[i] === ",") {
      parts.push({ from, to: i });
      from = i + 1;
    }
    return undefined;
  });
  parts.push({ from, to: span.to });
  return parts;
}

/** `span` without the space around it. */
function trimmed(text: string, { from, to }: Span): Span {
  while (from < to && /\s/.test(text[from])) from++;
  while (to > from && /\s/.test(text[to - 1])) to--;
  return { from, to };
}

/** An expression up to its own end: the first `;` or `,`, or a bracket it did not open. */
function expressionEnd(text: string, from: number): number {
  const end = atTopLevel(text, { from, to: text.length }, (i) =>
    ";,)]}".includes(text[i]) ? i : undefined,
  );
  return end ?? text.length;
}

/** An assigned expression up to its own end: the first `,`, `)`, `]` or `}` outside it. */
function statement(text: string): string {
  const end = atTopLevel(text, { from: 0, to: text.length }, (i) =>
    ",)]}".includes(text[i]) ? i : undefined,
  );
  return text.slice(0, end);
}

/** A property without a vendor's prefix: `-webkit-border-radius` is `border-radius`. */
export const unprefixed = (property: string) => property.replace(/^-(?:webkit|moz|ms|o)-/, "");

/** Where the ternary whose `?` is at `at` splits: the index of its own `:`, nested ternaries
 *  and strings skipped. */
function ternaryColon(text: string, at: number, to = text.length): number {
  let depth = 0;
  for (let i = at + 1; i < to; i++) {
    const char = text[i];
    if (char === '"' || char === "'" || char === "`") i = closingString(text, i);
    else if ("{[(".includes(char)) i = closingBracket(text, i);
    else if (isTernary(text, i)) depth++;
    else if (char === ":" && depth-- === 0) return i;
  }
  return -1;
}

/** Whether the `?` at `i` asks a ternary's question: not `?.` (unless a number follows) and not
 *  `??`. */
const isTernary = (text: string, i: number) =>
  text[i] === "?" &&
  text[i + 1] !== "?" &&
  text[i - 1] !== "?" &&
  !(text[i + 1] === "." && !/\d/.test(text[i + 2] ?? ""));

/**
 * The expressions `span` can give as its value: each branch of a ternary (`on ? held : undefined`
 * gives `held` and `undefined`), each side of `||` and `??`, the last of `&&`, and what is in
 * parentheses or before `as` and `satisfies`. A function (`() => …`) gives none: it is not a
 * style until it is called.
 */
function givenBy(text: string, span: Span): Span[] {
  const { from, to } = trimmed(text, span);
  if (from >= to) return [];
  const at = (find: (i: number) => boolean) =>
    atTopLevel(text, { from, to }, (i) => (find(i) ? i : undefined));
  if (at((i) => text.startsWith("=>", i)) !== undefined) return [];
  const question = at((i) => isTernary(text, i));
  if (question !== undefined) {
    const colon = ternaryColon(text, question, to);
    if (colon < 0) return [];
    return [
      ...givenBy(text, { from: question + 1, to: colon }),
      ...givenBy(text, { from: colon + 1, to }),
    ];
  }
  const either = at((i) => /^(?:\|\||\?\?)/.test(text.slice(i, i + 2)));
  if (either !== undefined) {
    return [...givenBy(text, { from, to: either }), ...givenBy(text, { from: either + 2, to })];
  }
  let both: number | undefined;
  at((i) => {
    if (text.startsWith("&&", i)) both = i;
    return false;
  });
  if (both !== undefined) return givenBy(text, { from: both + 2, to });
  const cast = at((i) => /^\s(?:as|satisfies)\s/.test(text.slice(i, i + 11)));
  if (cast !== undefined) return givenBy(text, { from, to: cast });
  if (text[to - 1] === "!") return givenBy(text, { from, to: to - 1 });
  if (text[from] === "(" && closingBracket(text, from) === to - 1) {
    return givenBy(text, { from: from + 1, to: to - 1 });
  }
  return [{ from, to }];
}

/** One file as the style reader reads it: its text with comments out, the names it declares,
 *  the functions it declares, and what it imports and exports. */
interface Scope {
  file: string | undefined;
  text: string;
  /** Each variable's initializer, with the member of it a destructuring takes. */
  holders: Map<string, Held[]>;
  /** Where each function's parameters open; its body is found when it is called. */
  functions: Map<string, number[]>;
  /** Each name imported: the file it is imported from and its name there (`*` for the whole
   *  file, `default`). */
  imports: Map<string, { from: string; name: string }>;
  /** Each name exported as another (`export { box as panel }`): `panel` is `box`. */
  exportsAs: Map<string, string>;
  /** The files every export of which this one exports (`export * from "./x"`). */
  stars: string[];
  /** What `export default` gives. */
  defaultExport: Span | undefined;
  /** Where the parameters of `export default function` open. */
  defaultFunction: number | undefined;
  /** Each parameter's default value (`({ pad = "7px" }) =>`, `function f(pad = "7px")`), found
   *  the first time a name in the file is followed (`parametersOf`). */
  parameters?: Map<string, Held[]>;
}

/** An arrow's parameters, up to its `=>`: `(…) =>`, `(…): Type =>`. */
const ARROW_PARAMETERS = /\)\s*(?::[^=;{}()]*?)?=>/g;
/** A function's parameters, up to their own `(`: `function f(`, `function (`, `function* f<T>(`. */
const FUNCTION_PARAMETERS = new RegExp(
  String.raw`\bfunction\b\s*\*?\s*(?:${NAME})?\s*(?:<[^<>()]*>)?\s*\(`,
  "g",
);

/** Where the bracket that closes at `close` opened, strings skipped whole. */
function openingBracket(text: string, close: number): number {
  let depth = 0;
  for (let i = close; i >= 0; i--) {
    const char = text[i];
    if (char === '"' || char === "'" || char === "`") {
      i--;
      while (i > 0 && !(text[i] === char && text[i - 1] !== "\\")) i--;
    } else if (")]}".includes(char)) depth++;
    else if ("([{".includes(char) && --depth === 0) return i;
  }
  return 0;
}

/** The default values of a file's arrows' and functions' parameters, by name, read once. */
function parametersOf(scope: Scope): Map<string, Held[]> {
  if (scope.parameters) return scope.parameters;
  const found = new Map<string, Held[]>();
  const { text } = scope;
  const hold = (name: string, held: Held) => found.set(name, [...(found.get(name) ?? []), held]);
  const lists: Span[] = [];
  for (const hit of text.matchAll(ARROW_PARAMETERS)) {
    const close = hit.index;
    lists.push({ from: openingBracket(text, close) + 1, to: close });
  }
  for (const hit of text.matchAll(FUNCTION_PARAMETERS)) {
    const open = hit.index + hit[0].length - 1;
    lists.push({ from: open + 1, to: closingBracket(text, open) });
  }
  for (const list of lists) defaultsIn(text, list, false, hold);
  scope.parameters = found;
  return found;
}

/** Each name in a parameter list or a pattern in it that is given a default (`pad = "7px"`,
 *  `{ pad = "7px" }`, `{ edge: pad = "7px" }`, `[pad = "7px"]`). `keyed` is an object pattern,
 *  where `a: b` names `b`; in a list, `a: T` is a type. */
function defaultsIn(
  text: string,
  span: Span,
  keyed: boolean,
  hold: (name: string, held: Held) => void,
) {
  for (const part of partsOf(text, span)) {
    let { from: at } = trimmed(text, part);
    const { to } = trimmed(text, part);
    if (text.startsWith("...", at)) at += 3;
    if (keyed) {
      const key = new RegExp(String.raw`^${NAME}\s*:\s*`).exec(text.slice(at, to));
      if (key) at += key[0].length;
    }
    if (text[at] === "{" || text[at] === "[") {
      const close = closingBracket(text, at);
      defaultsIn(text, { from: at + 1, to: close }, text[at] === "{", hold);
      continue;
    }
    const name = new RegExp(String.raw`^${NAME}`).exec(text.slice(at, to));
    if (!name) continue;
    const equals = atTopLevel(text, { from: at + name[0].length, to }, (i) =>
      text[i] === "=" && !"=>".includes(text[i + 1]) && !"=!<>".includes(text[i - 1])
        ? i
        : undefined,
    );
    if (equals !== undefined) hold(name[0], new Held(text, equals + 1, undefined));
  }
}

/** The body of the function whose parameters open at `open`: its block, past any return type
 *  (`function f(): { padding: string } { … }`). */
function bodyAfter(text: string, open: number): Span | undefined {
  let at = closingBracket(text, open) + 1;
  while (/\s/.test(text[at] ?? "")) at++;
  if (text[at] === ":") at = typeEnd(text, at + 1);
  const brace = text.indexOf("{", at);
  if (brace < 0) return undefined;
  return { from: brace, to: closingBracket(text, brace) + 1 };
}

/** Where a type annotation that starts at `from` ends: at the `{` that follows a whole type, its
 *  groups (`{ … }`, `[…]`, `(…)`, `<…>`) skipped and its words joined by `|`, `&` and `=>`. */
function typeEnd(text: string, from: number): number {
  let at = from;
  let whole = false;
  while (at < text.length) {
    const char = text[at];
    if (/\s/.test(char)) at++;
    else if (char === "{" && whole) return at;
    else if ("{[(".includes(char)) {
      at = closingBracket(text, at) + 1;
      whole = true;
    } else if (char === '"' || char === "'" || char === "`") {
      at = closingString(text, at) + 1;
      whole = true;
    } else if (char === "<") {
      let depth = 0;
      for (; at < text.length; at++) {
        if (text[at] === "<") depth++;
        else if (text[at] === ">" && text[at - 1] !== "=" && --depth === 0) break;
      }
      at++;
      whole = true;
    } else if (text.startsWith("=>", at) || char === "|" || char === "&") {
      at += char === "=" ? 2 : 1;
      whole = false;
    } else {
      const word = /^[\w$.]+/.exec(text.slice(at, at + 256));
      at += word ? word[0].length : 1;
      whole = word !== null;
    }
  }
  return at;
}

/** A variable's initializer, found where it ends only when it is followed, since most of a file's
 *  variables never hold a style. */
class Held {
  private end: number | undefined;
  constructor(
    private readonly text: string,
    private readonly from: number,
    private readonly type: string | undefined,
    readonly member?: string,
  ) {}

  get span(): Span {
    this.end ??= expressionEnd(this.text, this.from);
    return { from: this.from, to: this.end };
  }

  /** Whether it is typed or cast as `CSSProperties`. */
  get typed(): boolean {
    if (this.member !== undefined) return false;
    return TYPED.test(this.type ?? "") || CAST.test(this.text.slice(this.from, this.span.to));
  }
}

function scopeOf(file: string | undefined, source: string): Scope {
  const text = withoutComments(source, "ts");
  const scope: Scope = {
    file,
    text,
    holders: new Map(),
    functions: new Map(),
    imports: new Map(),
    exportsAs: new Map(),
    stars: [],
    defaultExport: undefined,
    defaultFunction: undefined,
  };
  const hold = (name: string, held: Held) =>
    scope.holders.set(name, [...(scope.holders.get(name) ?? []), held]);
  for (const hit of text.matchAll(DECLARES)) {
    const from = hit.index + hit[0].length;
    if (!hit[1].startsWith("{")) {
      hold(hit[1], new Held(text, from, hit[2]));
      continue;
    }
    for (const part of hit[1].slice(1, -1).split(",")) {
      const [, member, local] =
        new RegExp(String.raw`^\s*(${NAME})\s*(?::\s*(${NAME}))?\s*$`).exec(part) ?? [];
      if (member !== undefined) hold(local ?? member, new Held(text, from, undefined, member));
    }
  }
  for (const hit of text.matchAll(FUNCTION)) {
    const open = hit.index + hit[0].length - 1;
    scope.functions.set(hit[1], [...(scope.functions.get(hit[1]) ?? []), open]);
  }
  for (const [, named, all, some, from] of text.matchAll(IMPORTS)) {
    if (named !== undefined) scope.imports.set(named, { from, name: "default" });
    if (all !== undefined) scope.imports.set(all, { from, name: "*" });
    for (const [name, local] of specifiers(some)) scope.imports.set(local, { from, name });
  }
  for (const [, some, from] of text.matchAll(EXPORTS)) {
    if (some === undefined) {
      if (from !== undefined) scope.stars.push(from);
      continue;
    }
    for (const [name, local] of specifiers(some)) {
      if (from !== undefined) scope.imports.set(local, { from, name });
      else scope.exportsAs.set(local, name);
    }
  }
  const defaultFunction = new RegExp(
    String.raw`\bexport\s+default\s+(?:async\s+)?function\b\s*\*?\s*(?:${NAME})?\s*(?:<[^<>()]*>)?\s*\(`,
  ).exec(text);
  if (defaultFunction) {
    scope.defaultFunction = defaultFunction.index + defaultFunction[0].length - 1;
  }
  const byDefault = /\bexport\s+default\s+(?!(?:async\s+)?function\b|class\b)/.exec(text);
  if (byDefault) {
    const from = byDefault.index + byDefault[0].length;
    scope.defaultExport = { from, to: expressionEnd(text, from) };
  }
  return scope;
}

/** The names a `{ … }` of an import or export lists, as `[its name there, its name here]`. */
function specifiers(list: string | undefined): [string, string][] {
  return (list ?? "").split(",").flatMap((one) => {
    const [, name, local] =
      new RegExp(String.raw`^\s*(?:type\s+)?(${NAME})(?:\s+as\s+(${NAME}))?\s*$`).exec(one) ?? [];
    return name === undefined ? [] : [[name, local ?? name] as [string, string]];
  });
}

/** Where a style's source is read from, to follow a style it imports: its path from `src`, and
 *  how to read another file there (`undefined` when there is none). */
export interface StyleSource {
  file?: string;
  read?: (file: string) => string | undefined;
}

/**
 * The file a relative import names, as a path from `src`: as written (`./s.ts`), with the script
 * extension TypeScript resolves (`./s` or `./s.js` is `s.ts` or `s.tsx`), or the folder's
 * `index`. `undefined` for a package's import, or with no way to read files. A relative import
 * the guard cannot open, one above `src` or one not found, fails: the guard never skips a style
 * it was handed.
 */
function importedFile(from: string | undefined, spec: string, source: StyleSource) {
  if (from === undefined || !spec.startsWith(".") || !source.read) return undefined;
  const base = posix.normalize(posix.join(posix.dirname(from), spec));
  if (base.startsWith("..")) {
    throw new Error(
      `${from}: a style comes from "${spec}", outside src, which the guard cannot read`,
    );
  }
  const stem = base.replace(/\.(?:[cm]?jsx?)$/, "");
  const tries = /\.tsx?$/.test(base)
    ? [base]
    : [`${stem}.ts`, `${stem}.tsx`, `${stem}/index.ts`, `${stem}/index.tsx`, base];
  const file = tries.find((one) => source.read?.(one) !== undefined);
  if (file === undefined) {
    throw new Error(`${from}: a style comes from "${spec}", which the guard cannot open`);
  }
  return file;
}

/** The files each reader has given, parsed once per reader: one guard pass shares a reader
 *  (`windowFiles`), so a file many others import is parsed once in it. */
const parsedBy = new WeakMap<(file: string) => string | undefined, Map<string, Scope>>();

/** One key a style sets, as the follower finds it: how the complaint names it, the key as
 *  written, the property it sets, and where its value's expression is. */
interface StyleKey {
  label: string;
  key: string;
  property: string;
  scope: Scope;
  value: Span;
}

/** A value the follower found, and where it came from when it is not written where it is used. */
interface Given {
  value: string;
  origin?: string;
}

/** A value written out, negated: `7` is `-7`, `"-7px"` is `"7px"`. */
function negated(value: string): string {
  const flip = (written: string) => (written.startsWith("-") ? written.slice(1) : `-${written}`);
  return /^["'`]/.test(value) ? value[0] + flip(value.slice(1, -1)) + value[0] : flip(value);
}

/** A value being followed (`StyleFollower.values`): the values written out it has found, and
 *  the names it has followed, so a cycle ends. */
interface Values {
  found: string[];
  seen: Set<string>;
}

/**
 * **The style follower**: from where a style is handed (`style={…}`, a spread, the objects of
 * `Object.assign`), each object literal it can be, however the file hands it on: a variable that
 * holds it (`style={held}`), a member of a local object (`style={styles.box}`) or of a
 * destructuring (`const { box } = styles`), a branch of a ternary or a side of `||`, `??` and
 * `&&`, what a local function returns (`style={boxStyle(on)}`, `useMemo(() => ({ … }))`), and
 * what a relative import gives (`import { box } from "./styles"`, `import * as s`, a default
 * export, and what that file re-exports). Each object literal is read once, so a style handed
 * on twice counts once.
 */
class StyleFollower {
  private readonly done = new Set<string>();
  readonly home: Scope;

  constructor(
    source: string,
    private readonly from: StyleSource,
    private readonly found: (key: StyleKey) => void,
  ) {
    this.home = scopeOf(from.file, source);
  }

  /** Past `FOLLOW_LIMIT` hops the guard stops, and says where, rather than pass what it did not
   *  read. */
  private tooFar(scope: Scope, label: string, depth: number) {
    if (depth <= FOLLOW_LIMIT) return;
    throw new Error(
      `${scope.file ?? "a source"}: ${label.trim()} is handed on more than ${FOLLOW_LIMIT} times; hand it on fewer times so the guard can read it`,
    );
  }

  /** Each key of what `name` holds, as `style <name>.<key>`. */
  held(name: string) {
    this.name(this.home, name, [], `style ${name}.`, 0);
  }

  /** Each key the expression in `span` hands a style, named by how it is written there:
   *  `style ` for an object written in it, `style held.`, `style styles.box.`, `style box().`. */
  site(span: Span, scope = this.home) {
    for (const given of givenBy(scope.text, span)) {
      const written = scope.text.slice(given.from, given.to);
      const call = CALL.exec(written);
      const reference = referenceOf(written);
      const label = written.startsWith("{")
        ? "style "
        : call
          ? `style ${call[1].replace(/\s/g, "")}().`
          : `style ${reference ? reference.join(".") : written}.`;
      this.follow(scope, given, [], label, 0);
    }
  }

  /**
   * The values written out that the expression in `span` can give, followed as a style is: one
   * written there (`"7px"`, `0`), a branch of a ternary or a side of `||`, `??` and `&&`, a
   * variable that holds one (`const pad = "7px"; … padding: pad`), a member of a local object
   * (`sizes.pad`), a parameter's default (`({ pad = "7px" })`), what a local function returns
   * (`padOf(on)`), the operand of a unary `-` (`-pad`), and what a relative import gives. Each use
   * is followed on its own, so a value held once and used twice is read twice. A value not written
   * where it is used says where it came from (`held as pad`, `returned by padOf()`).
   */
  values(span: Span, scope = this.home, label = "a value"): Given[] {
    const values: Given[] = [];
    const seen = new Set<string>();
    for (const given of givenBy(scope.text, span)) {
      const written = scope.text.slice(given.from, given.to).replace(/\s+/g, "");
      const value: Values = { found: [], seen };
      this.follow(scope, given, [], label, 0, value);
      const call = WRITTEN_ALONE.test(written) ? null : CALL.exec(written);
      const origin = WRITTEN_ALONE.test(written)
        ? undefined
        : call && written.endsWith(")")
          ? `returned by ${call[1]}()`
          : `held as ${written}`;
      for (const one of value.found) values.push(origin ? { value: one, origin } : { value: one });
    }
    return values;
  }

  private follow(
    scope: Scope,
    span: Span,
    path: string[],
    label: string,
    depth: number,
    value?: Values,
  ) {
    this.tooFar(scope, label, depth);
    const { text } = scope;
    for (const given of givenBy(text, span)) {
      const written = text.slice(given.from, given.to);
      if (value && WRITTEN_ALONE.test(written)) {
        if (path.length === 0) value.found.push(written);
        continue;
      }
      const minus = value && /^-\s*(?=[A-Za-z_$(])/.exec(written);
      if (value && minus) {
        const operand: Values = { found: [], seen: value.seen };
        const rest = { from: given.from + minus[0].length, to: given.to };
        this.follow(scope, rest, path, label, depth + 1, operand);
        value.found.push(...operand.found.map(negated));
        continue;
      }
      if (written.startsWith("{") && closingBracket(text, given.from) === given.to - 1) {
        if (path.length > 0) this.member(scope, given, path, label, depth, value);
        else if (!value) this.object(scope, given.from, label);
        continue;
      }
      const call = CALL.exec(written);
      if (call && closingBracket(text, given.from + call[0].length - 1) === given.to - 1) {
        const callee = call[1].replace(/\s/g, "");
        const args = { from: given.from + call[0].length, to: given.to - 1 };
        if (/^(?:React\.)?useMemo$/.test(callee)) {
          const [first] = partsOf(text, args);
          for (const back of this.returns(scope, first)) {
            this.follow(scope, back, path, label, depth + 1, value);
          }
        } else {
          for (const [where, back] of this.called(scope, callee, label, depth)) {
            this.follow(where, back, path, label, depth + 1, value);
          }
        }
        continue;
      }
      const reference = referenceOf(written);
      if (reference) {
        const [root, ...members] = reference;
        this.name(scope, root, [...members, ...path], label, depth + 1, value);
      }
    }
  }

  /** The object literal opened at `open`, each key it sets, and each spread in it followed. */
  private object(scope: Scope, open: number, label: string) {
    const id = `${scope.file ?? ""}:${open}`;
    if (this.done.has(id)) return;
    this.done.add(id);
    const close = closingBracket(scope.text, open);
    for (const part of partsOf(scope.text, { from: open + 1, to: close })) {
      const written = scope.text.slice(part.from, part.to);
      const spread = /^\s*\.\.\./.exec(written);
      if (spread) {
        this.site({ from: part.from + spread[0].length, to: part.to }, scope);
        continue;
      }
      const [, key, expression] = ENTRY.exec(written) ?? [];
      if (key === undefined) continue;
      const custom = /^["']/.test(key);
      const property = propertyOf(custom ? key.slice(1, -1) : key);
      // The expression ends the entry, so its last match is where it starts.
      const from = part.from + written.lastIndexOf(expression);
      this.found({
        label: `${label}${custom ? property : key}`,
        key,
        property,
        scope,
        value: { from, to: from + expression.length },
      });
    }
  }

  /** The member `path[0]` of the object literal in `span`, followed down the rest of `path`. */
  private member(
    scope: Scope,
    span: Span,
    path: string[],
    label: string,
    depth: number,
    value?: Values,
  ) {
    const [want, ...rest] = path;
    const { text } = scope;
    for (const part of partsOf(text, { from: span.from + 1, to: span.to - 1 })) {
      const written = text.slice(part.from, part.to);
      const spread = /^\s*\.\.\./.exec(written);
      if (spread) {
        this.follow(
          scope,
          { from: part.from + spread[0].length, to: part.to },
          path,
          label,
          depth + 1,
          value,
        );
        continue;
      }
      const keyed = new RegExp(String.raw`^\s*(?:(${NAME})|"([^"]*)"|'([^']*)')\s*:`).exec(written);
      if (keyed) {
        if ((keyed[1] ?? keyed[2] ?? keyed[3]) === want) {
          this.follow(
            scope,
            { from: part.from + keyed[0].length, to: part.to },
            rest,
            label,
            depth + 1,
            value,
          );
        }
        continue;
      }
      if (written.trim() === want) this.name(scope, want, rest, label, depth + 1, value);
    }
  }

  /** What `name` holds in `scope`, down `path`: a variable's value, or what it imports. A style
   *  is followed down each (name, path) once per follower; a value, once per use. */
  private name(
    scope: Scope,
    name: string,
    path: string[],
    label: string,
    depth: number,
    value?: Values,
  ) {
    const id = `${scope.file ?? ""}#${name}#${path.join(".")}#${label}`;
    const done = value ? value.seen : this.done;
    if (done.has(id)) return;
    done.add(id);
    const held = [...(scope.holders.get(name) ?? []), ...(parametersOf(scope).get(name) ?? [])];
    for (const one of held) {
      const down = one.member ? [one.member, ...path] : path;
      this.follow(scope, one.span, down, label, depth, value);
    }
    const imported = scope.imports.get(name);
    if (!imported) return;
    const there = this.scope(scope, imported.from);
    if (!there) return;
    if (imported.name !== "*") this.exported(there, imported.name, path, label, depth, value);
    else if (path.length > 0) this.exported(there, path[0], path.slice(1), label, depth, value);
  }

  /** What a file exports as `name`, down `path`. */
  private exported(
    scope: Scope,
    name: string,
    path: string[],
    label: string,
    depth: number,
    value?: Values,
  ) {
    if (name === "default") {
      if (scope.defaultExport) {
        this.follow(scope, scope.defaultExport, path, label, depth + 1, value);
      }
      return;
    }
    const local = scope.exportsAs.get(name) ?? name;
    if (scope.holders.has(local) || scope.imports.has(local)) {
      this.name(scope, local, path, label, depth + 1, value);
      return;
    }
    for (const star of scope.stars) {
      const there = this.scope(scope, star);
      if (there) this.exported(there, name, path, label, depth + 1, value);
    }
  }

  /** What calling `name` in `scope` returns: a function's, an arrow's, an imported one's, a
   *  default-exported one's, or one called on a `* as` import (`s.make()`). */
  private called(scope: Scope, name: string, label: string, depth: number): [Scope, Span][] {
    this.tooFar(scope, label, depth);
    const backs: [Scope, Span][] = [];
    const [first, member, ...more] = name.split(".");
    if (member !== undefined) {
      const all = scope.imports.get(first);
      const there = more.length === 0 && all?.name === "*" && this.scope(scope, all.from);
      return there ? this.called(there, member, label, depth + 1) : [];
    }
    for (const open of scope.functions.get(name) ?? []) {
      const body = bodyAfter(scope.text, open);
      if (body) for (const back of this.returnsIn(scope.text, body)) backs.push([scope, back]);
    }
    for (const held of scope.holders.get(name) ?? []) {
      if (held.member) continue;
      for (const back of this.returns(scope, held.span)) backs.push([scope, back]);
    }
    const imported = scope.imports.get(name);
    const there = imported && imported.name !== "*" && this.scope(scope, imported.from);
    if (imported && there) {
      if (imported.name === "default") {
        const open = there.defaultFunction;
        const body = open === undefined ? undefined : bodyAfter(there.text, open);
        if (body) for (const back of this.returnsIn(there.text, body)) backs.push([there, back]);
        const given =
          there.defaultExport &&
          there.text.slice(there.defaultExport.from, there.defaultExport.to).trim();
        if (given && ROOT.exec(given)?.[0] === given) {
          backs.push(...this.called(there, given, label, depth + 1));
        } else if (there.defaultExport) {
          for (const back of this.returns(there, there.defaultExport)) backs.push([there, back]);
        }
      } else {
        const local = there.exportsAs.get(imported.name) ?? imported.name;
        backs.push(...this.called(there, local, label, depth + 1));
      }
    }
    return backs;
  }

  /** What the function written in `span` returns: an arrow's expression, or each `return`. */
  private returns(scope: Scope, span: Span): Span[] {
    const { text } = scope;
    const { from, to } = trimmed(text, span);
    const wrapped = CALL.exec(text.slice(from, to));
    if (
      wrapped &&
      /^(?:React\.)?useCallback$/.test(wrapped[1].replace(/\s/g, "")) &&
      closingBracket(text, from + wrapped[0].length - 1) === to - 1
    ) {
      const [fn] = partsOf(text, { from: from + wrapped[0].length, to: to - 1 });
      return this.returns(scope, fn);
    }
    const head = /^(?:async\s+)?/.exec(text.slice(from, to))?.[0].length ?? 0;
    let at = from + head;
    if (text.startsWith("function", at)) {
      const open = text.indexOf("(", at);
      const body = open < 0 ? undefined : bodyAfter(text, open);
      return body ? this.returnsIn(text, body) : [];
    }
    if (text[at] === "(") at = closingBracket(text, at) + 1;
    else {
      const param = new RegExp(String.raw`^${NAME}`).exec(text.slice(at, to));
      if (!param) return [];
      at += param[0].length;
    }
    const arrow = /^\s*(?::[^=]*?)?\s*=>\s*/.exec(text.slice(at, to));
    if (!arrow) return [];
    const body = at + arrow[0].length;
    if (text[body] === "{")
      return this.returnsIn(text, { from: body, to: closingBracket(text, body) + 1 });
    return [{ from: body, to }];
  }

  /** Each `return`'s expression in a function's block. */
  private returnsIn(text: string, body: Span): Span[] {
    const block = text.slice(body.from, body.to);
    return [...block.matchAll(/\breturn\b\s*/g)].map((hit) => {
      const from = body.from + hit.index + hit[0].length;
      return { from, to: Math.min(expressionEnd(text, from), body.to - 1) };
    });
  }

  /** The file `spec` names from `scope`, read and parsed once per reader. */
  private scope(scope: Scope, spec: string): Scope | undefined {
    const file = importedFile(scope.file, spec, this.from);
    const read = this.from.read;
    if (file === undefined || !read) return undefined;
    const parsed = parsedBy.get(read) ?? new Map<string, Scope>();
    parsedBy.set(read, parsed);
    const known = parsed.get(file);
    if (known) return known;
    const fresh = scopeOf(file, read(file) ?? "");
    parsed.set(file, fresh);
    return fresh;
  }
}

/** The names a file holds an element's style under: `const st = el.style`, `el["style"]`,
 *  `(el.style)`, `const { style } = el`, `const { style: s } = el`, and a name that holds one of
 *  those. */
function styleAliases(scope: Scope): Set<string> {
  const aliases = new Set<string>();
  // Only a file that holds `.style` itself, or destructures `style`, can hold an alias.
  if (
    !/\.style\s*(?:\bas\b|[;,)!?]|$)|\[\s*["'`]style["'`]\s*\]|\{[^{}=]*\bstyle\b[^{}=]*\}\s*(?::[^=]*)?=/m.test(
      scope.text,
    )
  ) {
    return aliases;
  }
  for (let grew = true; grew;) {
    grew = false;
    for (const [name, held] of scope.holders) {
      if (aliases.has(name)) continue;
      const alias = held.some(({ span, member }) => {
        if (member !== undefined) return member === "style";
        let value = scope.text
          .slice(span.from, span.to)
          .replace(/\s+as\s+[\w.]+\s*$/, "")
          .replace(/\[\s*["'`]style["'`]\s*\]/g, ".style")
          .replace(/[\s!?]/g, "");
        while (value.startsWith("(") && value.endsWith(")")) value = value.slice(1, -1);
        return /\.style$/.test(value) || aliases.has(value);
      });
      if (alias) {
        aliases.add(name);
        grew = true;
      }
    }
  }
  return aliases;
}

export interface StyleEntry {
  /** How the complaint names it: `style paddingTop`, `style held.paddingTop`, `style.padding`,
   *  `style.cssText padding`, `style attribute padding`, `setProperty --pad`. */
  label: string;
  property: string;
  value: string;
  /** Where a value not written where it is used came from: `held as pad`, `returned by padOf()`. */
  origin?: string;
}

/** The property a style key sets: `paddingTop` is `padding-top`; a custom property
 *  (`--least`) and a property already spelled as CSS (`padding-top`) are their own name. */
const propertyOf = (key: string) =>
  key.startsWith("--")
    ? key
    : key.replace(/^ms(?=[A-Z])/, "Ms").replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);

/**
 * Each property a component or a script sets that is written out, as the property it sets
 * (`paddingTop` is `padding-top`, a custom property `"--least"` is its own name):
 * - an entry of every style object a `style={…}` expression hands over, however the file hands
 *   it on (`StyleFollower`): written in it (`{{ … }}`, `{ … } as CSSProperties`,
 *   `style={on ? { … } : { … }}`), held in a variable (`style={held}`), a member of a local
 *   object (`style={styles.box}`), a branch of a ternary (`style={on ? held : undefined}`),
 *   what a local function returns (`style={boxStyle(on)}`), and what a relative import gives;
 *   and of each object it spreads (`{ ...held, gap: 0 }`);
 * - an entry of a style object typed or cast as `CSSProperties`, and of each object
 *   `Object.assign(el.style, …)` writes, written in the call or handed to it (`held`);
 * - an assignment to an element's style (`el.style.padding = "7px"`,
 *   `el.style["padding-top"] = "7px"`, ``el.style[`padding`] = "7px"``), and a whole style written
 *   as CSS text (`el.style.cssText = "…"`, `el.style.cssText += "…"`,
 *   `el.setAttribute("style", "…")`), each declaration in it; an element's style held under
 *   another name (`const st = el.style`, `const { style } = el`) is read as `el.style`;
 * - a `setProperty(…)` call (`el.style.setProperty(…)`, `el.style?.setProperty(…)`, `st.setProperty(…)`).
 *
 * A value is read where it is written out, and followed as a style is (`StyleFollower.values`):
 * each branch of a ternary (`padding: on ? "7px" : 0`), a variable that holds it
 * (`const pad = "7px"; … padding: pad`), what a local function returns, and what a relative
 * import gives (#2684). A vendor-prefixed key (`WebkitBorderRadius`,
 * `msOverflow`) is its prefixed property (`-webkit-border-radius`), which a guard reads
 * `unprefixed`. `from` names the file and how to read the window's other files, so a style
 * imported from one of them is followed; without it, imports are not.
 */
export function styleEntries(source: string, from: StyleSource = {}): StyleEntry[] {
  const found: StyleEntry[] = [];
  const follower: StyleFollower = new StyleFollower(source, from, (key) => {
    for (const given of follower.values(key.value, key.scope, key.label)) {
      found.push({ label: key.label, property: key.property, ...given });
    }
  });
  const { text, holders } = follower.home;
  for (const hit of text.matchAll(/\bstyle=\{/g)) {
    const open = hit.index + hit[0].length - 1;
    styleSite(follower, open);
  }
  if (TYPED.test(text)) {
    for (const [name, held] of holders) {
      if (held.some(({ typed }) => typed)) follower.held(name);
    }
  }
  const aliases = [...styleAliases(follower.home)].map((name) =>
    name.replace(/[\\^$.*+?()[\]{}|]/g, "\\$&"),
  );
  const receiver = String.raw`(?:\.style|\[\s*["'${"`"}]style["'${"`"}]\s*\]${aliases.length ? `|(?<![\\w$.])(?:${aliases.join("|")})(?![\\w$])` : ""})\s*(?:!\s*)?`;
  const assigns = new RegExp(
    String.raw`\bObject\.assign\(\s*(?:[\w$.?!]*(?:\bstyle|\[\s*["'${"`"}]style["'${"`"}]\s*\])${aliases.length ? `|(?:${aliases.join("|")})` : ""})\s*,`,
    "g",
  );
  for (const hit of text.matchAll(assigns)) {
    const close = closingBracket(text, hit.index + hit[0].indexOf("("));
    for (const part of partsOf(text, { from: hit.index + hit[0].length, to: close })) {
      follower.site(part);
    }
  }
  const assigned = new RegExp(
    String.raw`${receiver}(?:\.\s*(\w+)|\[\s*(${STRING})\s*\])\s*=(?![=>])\s*([^;\n]+)`,
    "g",
  );
  for (const hit of text.matchAll(assigned)) {
    const [whole, key, quoted, expression] = hit;
    if (key === "cssText") continue;
    const named = key ?? quoted.slice(1, -1);
    const at = hit.index + whole.length - expression.length;
    const label = `style.${named}`;
    const span = { from: at, to: at + statement(expression).length };
    for (const given of follower.values(span, follower.home, label)) {
      found.push({ label, property: propertyOf(named), ...given });
    }
  }
  const styleText = new RegExp(
    String.raw`(?:${receiver}\.\s*(cssText)\s*\+?=(?![=>])|\.setAttribute\(\s*["'${"`"}]style["'${"`"}]\s*,)\s*(${STRING})`,
    "g",
  );
  for (const [, cssText, written] of text.matchAll(styleText)) {
    const quote = written[0];
    const label = cssText ? "style.cssText" : "style attribute";
    for (const { property, value } of declarations(`x{${written.slice(1, -1)}}`)) {
      found.push({ label: `${label} ${property}`, property, value: quote + value + quote });
    }
  }
  for (const hit of text.matchAll(SET_PROPERTY)) {
    const open = hit.index + hit[0].length - 1;
    const [name, given] = partsOf(text, { from: open + 1, to: closingBracket(text, open) });
    const written = trimmed(text, name);
    const named = text.slice(written.from, written.to);
    if (!given || !STRING_ALONE.test(named)) continue;
    const property = named.slice(1, -1);
    const label = `setProperty ${property}`;
    for (const one of follower.values(given, follower.home, label)) {
      found.push({ label, property, ...one });
    }
  }
  return found;
}

/** The style expression a `style={` hands at `open` (its `{`): what it gives, followed, and
 *  every object written anywhere in it. */
function styleSite(follower: StyleFollower, open: number) {
  const { text } = follower.home;
  const end = closingBracket(text, open);
  follower.site({ from: open + 1, to: end });
  for (let i = open + 1; i < end; i++) {
    const char = text[i];
    if (char === '"' || char === "'" || char === "`") i = closingString(text, i);
    else if (char === "{") {
      follower.site({ from: i, to: closingBracket(text, i) + 1 });
      i = closingBracket(text, i);
    }
  }
}

/**
 * The keys one tag's `style={…}` sets, whatever their values, as `style <key>` or
 * `style <how it was handed>.<key>`, followed as `styleEntries` follows a style. `tag` is a tag
 * `openingTags(source, …)` found; `from` names the file, to follow its imports.
 */
export function tagStyleKeys(
  source: string,
  tag: Tag,
  from: StyleSource = {},
): { label: string; key: string }[] {
  const at = /\sstyle=\{/.exec(tag.text);
  if (!at) return [];
  const keys: { label: string; key: string }[] = [];
  const follower = new StyleFollower(source, from, ({ label, key }) => keys.push({ label, key }));
  styleSite(follower, tag.index + at.index + at[0].length - 1);
  return keys;
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
 * up to the package's own name (`checkbox`, `radio-group`): the package itself. Every overlay is
 * a family's, on React Aria (#2906), so no window file stands in for a Radix package.
 */
export const RADIX_FROM = String.raw`@radix-ui\/react-`;
