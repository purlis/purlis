// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Every ground a component family is drawn on is in the parts manifest** (#2959, ADR 0103).
 *
 * The core's parts check (`purlis_core::theme::parts`) holds a family's words, its edge and its
 * look rules on each ground the family's manifest entry lists (`grounds: &[…]` in
 * `crates/purlis-core/src/theme/parts.rs`): a see-through fill shows what it is drawn on. What
 * only the window can know is which grounds those are, so that half is here, found from the
 * components rather than written by hand:
 *
 * - **a ground** is an element a stylesheet under `src/` gives a background token, at rest or in
 *   a state (`.row:hover`), by its class, its tag or an attribute it is written with (inside the
 *   classes the rule is scoped by), or a Tailwind `bg-*` utility; it is read every way a wash is
 *   (below: a fallback, an alias, a gradient), but only a layer that covers the box is a ground, so
 *   a repeating gradient's lines (a tree's guide) or a fade to `transparent` is not one (#4971);
 *   the window itself (`:root`) is the ground under everything else, and what is portaled to the
 *   body;
 * - **a button** is an element of the family: the family's `data-scope="button"` root, or a member
 *   of `ui/button` (no class marks one: the old `ui-button` is retired, #3365);
 * - every component is read: a button is on the grounds of the elements around it, up to the
 *   first one that has a background at rest, through the components that render it, the
 *   `children` they place, and the JSX a component keeps in a variable. A class a component may
 *   or may not carry is a ground it may be on, and never hides the one under it.
 *
 * The list of grounds found must be the manifest's list, exactly, so a new surface a button is
 * placed on fails here until the manifest names it, and the parts check measures it. *
 * **Where each see-through ground is laid** (#4796) is read the same way: every rule that paints a
 * token purlis ships see-through is followed to the grounds under the element it paints, and those
 * must be `WASH_LAID_ON` (in `parts.rs`), exactly, so a wash drawn on a new ground fails here until
 * the table names it and the parts check reads what stands on the wash over that ground. A wash
 * is found however it is painted (#4876): a `background`, `background-color` or
 * `background-image` naming it in any `var()` (a fallback's, a gradient's, a `color-mix()`'s),
 * through an alias a sheet sets (`--color-danger-wash`), or a Tailwind utility on the element; the
 * editor's look (`EditorView.theme`) is read as a sheet. The grounds under it are read whole too,
 * so `.pane-frame.pane-headed` paints only a frame that carries both classes, and a ground listed
 * by hand is held to where the window draws the element its rule is scoped by.
 */

/// <reference types="node" />
import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import ts from "typescript";
import { describe, expect, it } from "vitest";
import { FAMILY_THEMES } from "./families.testkit";
import { property, TOKENS, type Token } from "./theme";

const SRC = join(process.cwd(), "src");
const THEME = join(process.cwd(), "..", "crates", "purlis-core", "src", "theme");

/** Each family's grounds, as the engine's manifest lists them: from each family's own file,
 *  `parts_<family>.rs` (ADR 0103 amended 2026-10-26), a shared list named there read from that
 *  file first and then from `parts.rs`, which the file imports it from. */
function engineGrounds(): Map<string, string[]> {
  const parts = readFileSync(join(THEME, "parts.rs"), "utf8");
  const grounds = new Map<string, string[]>();
  for (const name of readdirSync(THEME).filter((one) => /^parts_\w+\.rs$/.test(one))) {
    if (name.endsWith("_tests.rs")) continue;
    const own = readFileSync(join(THEME, name), "utf8");
    for (const [family, listed] of manifestGrounds(`${parts}\n${own}`, own))
      grounds.set(family, listed);
  }
  return grounds;
}

function manifestGrounds(source: string, families = source): Map<string, string[]> {
  const tokens = (list: string) => [...list.matchAll(/"([^"]+)"/g)].map(([, token]) => token);
  const shared = new Map<string, string[]>();
  for (const [, name, list] of source.matchAll(/\nconst ([A-Z_]+): &\[&str\] = &\[([^\]]*)\];/g))
    shared.set(name, tokens(list));
  const grounds = new Map<string, string[]>();
  for (const [, name, list, named] of families.matchAll(
    /Family \{\s*name: "([^"]+)",[\s\S]*?\n\s*grounds: (?:&\[([^\]]*)\]|([A-Z_]+),)/g,
  ))
    grounds.set(name, named === undefined ? tokens(list) : (shared.get(named) ?? []));
  return grounds;
}

// --- The stylesheets: which class draws which ground ---------------------------------------

/** The button family's members (`app/src/ui/button`): each draws one button. */
const FAMILY = new Set(["Button", "IconButton", "ToggleButton"]);

const BY_PROPERTY = new Map<string, Token>(TOKENS.map((token) => [property(token), token]));
/** A token by its name in a Tailwind utility: `surface-hover` is `surface.hover`. */
const BY_DASHED = new Map<string, Token>(TOKENS.map((token) => [token.replace(/\./g, "-"), token]));

/** The parts of a selector that say a state, not what is drawn at rest. */
const STATE =
  /:(?:hover|focus-visible|focus-within|focus|active|checked)(?![\w-])|\[(?:aria-(?:selected|current|pressed|checked|expanded)|data-(?:hovered|pressed|selected|focus-visible|open|state|on|active|current))(?:[~|^$*]?="[^"]*")?\]/;

/** Of each family's parts, by scope, the only ones that are grounds (a family's `groundParts`). */
const GROUND_PARTS = new Map(
  FAMILY_THEMES.flatMap((family) => Object.entries(family.groundParts ?? {})),
);

/** Whether `subject` is a family's part that family says is no ground a button stands on. */
function noGround(subject: string): boolean {
  for (const [scope, parts] of GROUND_PARTS)
    if (
      subject.includes(`[data-scope="${scope}"]`) &&
      !parts.some((part) => subject.includes(`[data-part="${part}"]`))
    )
      return true;
  return false;
}

/** The grounds one class draws: each background token, and whether one is drawn at rest. */
type Ground = { tokens: Set<Token>; rest: boolean };

/** `text` split at its top-level `sep`s, never inside a `( )` or a `[ ]`. */
function topLevel(text: string, sep: RegExp): string[] {
  const out: string[] = [];
  let depth = 0;
  let from = 0;
  for (let at = 0; at < text.length; at += 1) {
    const one = text[at];
    if (one === "(" || one === "[") depth += 1;
    else if (one === ")" || one === "]") depth -= 1;
    else if (depth === 0 && sep.test(one)) {
      out.push(text.slice(from, at));
      from = at + 1;
    }
  }
  out.push(text.slice(from));
  return out.map((part) => part.trim()).filter((part) => part !== "");
}

/** Without each `:not(…)` and `:has(…)`, which only narrow which elements are meant: a class
 *  inside them is another element's. */
function narrowedOut(selector: string): string {
  let out = selector;
  for (let was = ""; was !== out;) {
    was = out;
    out = out.replace(/:(?:not|has)\([^()]*(?:\([^()]*\)[^()]*)*\)/g, "");
  }
  return out;
}

/** Every sheet under `src/`: the window's and the specimen's, each a sheet a button may be
 *  drawn by. */
function sheets(dir = SRC): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return sheets(path);
    return entry.name.endsWith(".css")
      ? [readFileSync(path, "utf8").replace(/\/\*[\s\S]*?\*\//g, "")]
      : [];
  });
}

/** A rule a background is set by that the reader cannot tie to a class, as `selector`; and
 *  each rule on an attribute, by the form it reaches (`[role=tab]`), which is read where an
 *  element carries it as written. */
type Unread = { classless: string[]; within: string[]; byAttribute: Map<string, string[]> };

/** The class each rule's subject carries, with the background tokens it is drawn in; and the
 *  window's own ground, the `:root`'s. */
function groundsOfClasses(css: string[]): {
  byClass: Map<string, Ground>;
  window: Set<Token>;
  unread: Unread;
} {
  const byClass = new Map<string, Ground>();
  const window = new Set<Token>();
  const unread: Unread = { classless: [], within: [], byAttribute: new Map() };
  const aliases = aliasesOf(css);
  for (const sheet of css)
    for (const [, selectors, body] of sheet.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      // Every way a ground is painted, as a wash's is (#4971), but only a layer that covers the
      // box: a gradient's line, as a tree's guide is, is drawn on the element, never under what
      // it holds (D-4876-2).
      const backs = backgroundsIn(body, aliases, true);
      if (backs.length === 0) continue;
      for (const selector of topLevel(selectors.replace(/^\s*@[^;{]*;?/, ""), /,/)) {
        if (/^(?::root|html|body)$/.test(selector)) backs.forEach((token) => window.add(token));
        const compounds = topLevel(selector, /[\s>+~]/);
        const last = compounds[compounds.length - 1] ?? "";
        const subject = narrowedOut(last);
        // Only a rule on the element alone, at rest, and on the element itself rather than one
        // of its pseudo-elements, hides the ground under it: a rule scoped by where the element
        // stands reaches only some of them.
        const rest =
          !STATE.test(narrowedOut(selector)) && compounds.length === 1 && !subject.includes("::");
        if (/:has\(/.test(last)) unread.within.push(selector);
        const classes = [...subject.matchAll(/\.([\w-]+)/g)];
        // A rule on a tag (`.vault-secrets tbody tr:hover`) or an attribute (`.tabs
        // [role="tab"]`) reaches each element of it inside the classes it is scoped by: a ground
        // each such element may draw, never one that hides another.
        const bare = subject.replace(/^:(?:where|is)\(/, "");
        const tag = /^([a-z][\w-]*)/.exec(bare)?.[1];
        const attribute = /^\[([\w-]+)(?:="([^"]*)")?\]/.exec(bare);
        // A button's own fill is no ground: nothing a button holds is another button.
        if (classes.length === 0 && tag === "button") continue;
        // A family's part it says is no ground (its `groundParts`).
        if (noGround(subject)) continue;
        const form =
          tag !== undefined
            ? `<${tag}>`
            : attribute === null
              ? undefined
              : `[${attribute[1]}=${attribute[2] ?? ""}]`;
        if (classes.length === 0 && form !== undefined && !/^(?:html|body)$/.test(tag ?? "")) {
          if (tag === undefined)
            unread.byAttribute.set(form, [...(unread.byAttribute.get(form) ?? []), selector]);
          const within = compounds
            .slice(0, -1)
            .flatMap((one) => [...narrowedOut(one).matchAll(/\.([\w-]+)/g)].map((hit) => hit[1]))
            .join(" ");
          const key = `${form}@${within}`;
          const ground = byClass.get(key) ?? { tokens: new Set<Token>(), rest: false };
          backs.forEach((token) => ground.tokens.add(token));
          byClass.set(key, ground);
          continue;
        }
        if (classes.length === 0 && !/^(?::root|html|body)$/.test(selector))
          unread.classless.push(selector);
        for (const [, name] of classes) {
          const ground = byClass.get(name) ?? { tokens: new Set<Token>(), rest: false };
          backs.forEach((token) => ground.tokens.add(token));
          ground.rest ||= rest;
          byClass.set(name, ground);
        }
      }
    }
  return { byClass, window, unread };
}

/** The ground a Tailwind utility draws: `bg-surface-hover`, or `hover:bg-surface-hover` in a
 *  state; or an arbitrary value, `bg-[var(--danger-wash)]` or `bg-(--danger-wash)` (#4876). */
function utility(
  name: string,
  aliases: Aliases = new Map(),
): (Ground & { fills: Set<Token> }) | undefined {
  const hit = /^((?:[\w-]+:)*)bg-(?:\[([^\]]*)\]|\(([^)]*)\)|([\w-]+))$/.exec(name);
  if (hit === null) return undefined;
  const [, state, arbitrary, shorthand, dashed] = hit;
  // A theme's colour is its `--color-*` (Tailwind's `@theme`), which may alias a token of another
  // name; an arbitrary value's spaces are written as `_`.
  const body =
    dashed !== undefined
      ? `background: var(--color-${dashed})`
      : `background: ${(arbitrary ?? `var(${shorthand})`).replace(/_/g, " ")}`;
  const own = dashed === undefined ? undefined : BY_DASHED.get(dashed);
  const tokens = new Set([...backgroundsIn(body, aliases), ...(own === undefined ? [] : [own])]);
  if (tokens.size === 0) return undefined;
  const fills = new Set([
    ...backgroundsIn(body, aliases, true),
    ...(own === undefined ? [] : [own]),
  ]);
  return { tokens, fills, rest: state === "" };
}

/** Each custom property a sheet sets to another's value alone, by its name: an alias a rule may
 *  paint a token by (`--color-danger-wash: var(--danger-wash)`, Tailwind's `@theme`). */
type Aliases = ReadonlyMap<string, ReadonlySet<string>>;

function aliasesOf(css: string[]): Aliases {
  const aliases = new Map<string, Set<string>>();
  for (const sheet of css)
    for (const [, name, to] of sheet.matchAll(/(--[\w-]+)\s*:\s*var\(\s*(--[\w-]+)\s*\)\s*[;}]/g))
      aliases.set(name, (aliases.get(name) ?? new Set()).add(to));
  return aliases;
}

/** **Whether one layer of a background covers the box it paints**, as a ground does (#4971): a
 *  colour, or a gradient that does not repeat, has no see-through stop and is not sized smaller
 *  than the box. A repeating gradient draws lines, as a tree's guide is (D-4876-2), and one that
 *  fades to `transparent` shows what is under it: each is a decoration, never a ground. */
function covers(layer: string, sized: boolean): boolean {
  if (/^(?:none|url\()/.test(layer.trim())) return false;
  if (!/gradient\(/.test(layer)) return true;
  return (
    !sized &&
    !/repeating-[\w-]*gradient\(/.test(layer) &&
    !/(?<![\w-])transparent(?![\w-])/.test(layer)
  );
}

/** **The tokens a rule's body paints as a background**, by every way it can name one (#4876):
 *  `background`, `background-color` or `background-image`, and each `var()` in its value, so a
 *  fallback's (`var(--a, var(--b))`), a gradient's and a `color-mix()`'s, each through the
 *  aliases a sheet sets. */
function backgroundsIn(body: string, aliases: Aliases, fills = false): Token[] {
  const found = new Set<Token>();
  // An image sized smaller than the box is drawn on part of it.
  const sized =
    /(?<![\w-])background-size\s*:\s*(?!(?:cover|auto|100%(?:\s+100%)?)\s*(?:;|$))/.test(body);
  for (const [, value] of body.matchAll(/(?<![\w-])background(?:-color|-image)?\s*:([^;]*)/g)) {
    // Only a layer that covers the box is a fill that hides what is under it (#4971).
    const layers = fills ? topLevel(value, /,/).filter((one) => covers(one, sized)) : [value];
    for (const [, name] of layers.join(",").matchAll(/var\(\s*(--[\w-]+)/g)) {
      const seen = new Set<string>();
      const follow = (one: string) => {
        if (seen.has(one)) return;
        seen.add(one);
        const token = BY_PROPERTY.get(one);
        if (token !== undefined) found.add(token);
        aliases.get(one)?.forEach(follow);
      };
      follow(name);
    }
  }
  return [...found];
}

/** **A rule that paints a see-through ground, read whole** (#4796): the element it paints carries
 *  every class and attribute its subject names (a state's aside), and stands inside every class
 *  the compounds before it name. One `:is(…)` subject is each of its alternatives. */
type WashRule = {
  token: Token;
  selector: string;
  tag?: string;
  classes: string[];
  attributes: [string, string | undefined][];
  within: string[];
  /** Whether it fills the element whole, rather than drawing a gradient on it. */
  fills?: boolean;
};

/** **Every rule in `css` that paints a background, read whole** (#4876), the way a wash is: the
 *  grounds under a see-through one. A rule that names nothing of the element (`:root`) is the
 *  window's, never an element's. */
function paintRulesOf(css: string[]): WashRule[] {
  return washRulesOf(css, new Set(BY_PROPERTY.values())).filter(
    (rule) =>
      (rule.tag !== undefined && !/^(?:html|body)$/.test(rule.tag)) ||
      rule.classes.length > 0 ||
      rule.attributes.length > 0,
  );
}

/** Whether `rule` paints its element wherever it stands and at rest, read alone: one compound,
 *  no state, nothing it narrows by. On an element that carries all it names, as written, it
 *  covers what is under the element (#4796, #4876). */
function atRest(rule: WashRule): boolean {
  return (
    topLevel(rule.selector, /[\s>+~]/).length === 1 &&
    !STATE.test(rule.selector) &&
    !/:(?:is|has|not|where)\(/.test(rule.selector)
  );
}

/** A subject's alternatives: each of a leading `:is(…)`'s, with what follows it. */
function alternativesOf(subject: string): string[] {
  if (!subject.startsWith(":is(")) return [subject];
  let depth = 0;
  for (let at = 3; at < subject.length; at += 1) {
    if (subject[at] === "(") depth += 1;
    else if (subject[at] === ")" && --depth === 0) {
      const rest = subject.slice(at + 1);
      return topLevel(subject.slice(4, at), /,/).map((one) => one + rest);
    }
  }
  return [subject];
}

/** What one compound names of the element it reaches (`.pane-top`, `[data-scope="dialog"]`). */
function subjectOf(compound: string): Omit<WashRule, "token"> {
  return {
    selector: compound,
    tag: /^([a-z][\w-]*)/.exec(compound)?.[1],
    classes: [...compound.matchAll(/\.([\w-]+)/g)].map((hit) => hit[1]),
    attributes: [...compound.matchAll(/\[([\w-]+)(?:="([^"]*)")?\]/g)].map(
      ([, name, value]): [string, string | undefined] => [name, value],
    ),
    within: [],
  };
}

/** Where the window draws the elements one anchor names: the grounds under them, whether one is
 *  drawn where its own component covers nothing, and how many are drawn. */
type Anchored = { on: Set<Token>; loose: boolean; drawn: number };

/** Every rule in `css` that paints one of `washes` as a background, read whole. */
function washRulesOf(css: string[], washes: ReadonlySet<Token>): WashRule[] {
  const rules: WashRule[] = [];
  const aliases = aliasesOf(css);
  for (const sheet of css)
    for (const [, selectors, body] of sheet.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      const backs = backgroundsIn(body, aliases).filter((token) => washes.has(token));
      const fills = backgroundsIn(body, aliases, true);
      if (backs.length === 0) continue;
      for (const selector of topLevel(selectors.replace(/^\s*@[^;{]*;?/, ""), /,/)) {
        const compounds = topLevel(selector, /[\s>+~]/);
        const last = compounds[compounds.length - 1] ?? "";
        // A pseudo-element is drawn on the element, never under what it holds.
        if (last.includes("::")) continue;
        const within = compounds
          .slice(0, -1)
          .filter((one) => !one.includes(":is("))
          .flatMap((one) => [...narrowedOut(one).matchAll(/\.([\w-]+)/g)].map((hit) => hit[1]));
        const alternatives = alternativesOf(last);
        for (const alternative of alternatives) {
          const subject = narrowedOut(alternative);
          const attributes = [...subject.matchAll(/\[([\w-]+)(?:="([^"]*)")?\]/g)]
            .filter(([whole]) => !STATE.test(whole))
            .map(([, name, value]): [string, string | undefined] => [name, value]);
          for (const token of backs)
            rules.push({
              token,
              selector,
              tag: /^([a-z][\w-]*)/.exec(subject)?.[1],
              classes: [...subject.matchAll(/\.([\w-]+)/g)].map((hit) => hit[1]),
              attributes,
              within,
              fills: fills.includes(token),
            });
        }
      }
    }
  return rules;
}

// --- The components: where each button is drawn -----------------------------------------------

/** A class an element carries, and whether it carries it always or only on a condition. */
type Carried = { name: string; always: boolean };

/** One element around a button: the grounds it may draw, and whether it draws one at rest
 *  always, which hides what is under it. */
type Frame = { tokens: Set<Token>; covers: boolean; classes: Set<string>; painted: Painting };

/** What one element is painted, read whole (#4876): each background a rule that reaches it may
 *  paint, and whether one it carries all of, as written and at rest, covers what is under it. */
type Painting = { tokens: Set<Token>; covers: boolean };

/** What one function that renders JSX places: the grounds its buttons are drawn on, whether
 *  one is drawn on what is under the function, where its `children` go, and what it renders. */
type Unit = {
  name: string;
  file: string;
  found: Set<Token>;
  loose: boolean;
  childrenAt: Frame[][];
  uses: { name: string; frames: Frame[] }[];
};

function sources(dir: string): { name: string; text: string }[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return sources(path);
    return /\.tsx$/.test(entry.name) && !/\.(test|stories)\.tsx$/.test(entry.name)
      ? [{ name: relative(SRC, path), text: readFileSync(path, "utf8") }]
      : [];
  });
}

/** The grounds `frames` (innermost last) draw under what they hold, out to the first that
 *  covers what is under it; and whether one does. */
function under(frames: Frame[]): { tokens: Set<Token>; covered: boolean } {
  const tokens = new Set<Token>();
  for (let at = frames.length - 1; at >= 0; at -= 1) {
    frames[at].tokens.forEach((token) => tokens.add(token));
    if (frames[at].covers) return { tokens, covered: true };
  }
  return { tokens, covered: false };
}

/** What `frames` draw under a see-through ground (#4796): as [`under`], but each element as the
 *  rules read whole paint it (#4876), so `.frame.headed` paints only a frame that carries both
 *  classes, out to the first that covers what is under it. */
function beneath(frames: Frame[]): { tokens: Set<Token>; covered: boolean } {
  const tokens = new Set<Token>();
  for (let at = frames.length - 1; at >= 0; at -= 1) {
    frames[at].painted.tokens.forEach((token) => tokens.add(token));
    if (frames[at].painted.covers) return { tokens, covered: true };
  }
  return { tokens, covered: false };
}

const CONDITIONAL = (node: ts.Node) =>
  ts.isConditionalExpression(node) ||
  ts.isObjectLiteralExpression(node) ||
  (ts.isBinaryExpression(node) &&
    [
      ts.SyntaxKind.AmpersandAmpersandToken,
      ts.SyntaxKind.BarBarToken,
      ts.SyntaxKind.QuestionQuestionToken,
    ].includes(node.operatorToken.kind));

/** Reads every component of the window, finding where each button is drawn. */
class Reader {
  readonly units = new Map<string, Unit[]>();
  /** The classes a button carries, which are its own fill and never a ground under it. */
  readonly buttonClasses = new Set<string>();
  /** Each family member a file names, as `file:name`, which draws a button wherever it stands. */
  private readonly family = new Set<string>();
  /** Each element that sets a background in its own `style`, which no stylesheet names. */
  readonly inline = new Set<string>();
  /** Each tag or attribute form a background rule reaches that some element carries. */
  readonly reached = new Set<string>();

  /** The grounds of the rules on a tag or an attribute, by the form each reaches, with the
   *  classes each is scoped by. */
  private readonly forms = new Map<string, { scoped: string[]; ground: Ground }[]>();

  /** Each element an anchor names (a `PAINTED` entry's `at`), with the grounds under it out to
   *  the first that covers what is under it, and how many the window draws (#4876). */
  readonly anchored = new Map<string, Anchored>();
  /** The rule each Tailwind utility that paints a see-through ground is read as, by its class. */
  readonly utilityRules = new Map<string, WashRule>();
  private readonly anchors: WashRule[];

  constructor(
    private readonly files: { name: string; text: string }[],
    private readonly byClass: Map<string, Ground>,
    private readonly given: {
      washRules?: readonly WashRule[];
      paintRules?: readonly WashRule[];
      familyWashes?: ReadonlySet<Token>;
      washes?: ReadonlySet<Token>;
      aliases?: Aliases;
      anchors?: readonly string[];
    } = {},
  ) {
    this.anchors = (given.anchors ?? []).map((at) => ({ ...subjectOf(at), token: "surface.base" }));
    for (const [key, ground] of byClass) {
      const at = key.indexOf("@");
      if (at < 0) continue;
      const scoped = key
        .slice(at + 1)
        .split(" ")
        .filter((one) => one !== "");
      const form = key.slice(0, at);
      this.forms.set(form, [...(this.forms.get(form) ?? []), { scoped, ground }]);
    }
  }

  /** One pass over every file, with each component's `children` places from the pass before. */
  read(): void {
    // What each pass lays and anchors is its own: the last, which knows every `children` place,
    // is the one read.
    this.laid.clear();
    this.anchored.clear();
    const fresh = new Map<string, Unit[]>();
    for (const [source, held] of this.parsed()) this.readFile(source, held, fresh);
    this.units.clear();
    fresh.forEach((units, name) => this.units.set(name, units));
  }

  /** Each file, parsed with its variables once for every pass. */
  private parsedFiles?: [ts.SourceFile, Held][];
  private parsed(): [ts.SourceFile, Held][] {
    this.parsedFiles ??= this.files.map((file) => {
      const source = ts.createSourceFile(
        file.name,
        file.text,
        ts.ScriptTarget.Latest,
        true,
        ts.ScriptKind.TSX,
      );
      return [source, new Held(source)];
    });
    return this.parsedFiles;
  }

  private readFile(source: ts.SourceFile, held: Held, into: Map<string, Unit[]>): void {
    // The family's members this file names: imported from the family's module, or its own.
    const own = /(^|\/)ui\/button\/Button\.tsx$/.test(source.fileName);
    for (const statement of source.statements) {
      if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier))
        continue;
      if (!/(^|\/)ui\/button(\/Button)?$/.test(statement.moduleSpecifier.text)) continue;
      const bindings = statement.importClause?.namedBindings;
      if (bindings !== undefined && ts.isNamedImports(bindings))
        for (const one of bindings.elements)
          if (FAMILY.has((one.propertyName ?? one.name).text))
            this.family.add(`${source.fileName}:${one.name.text}`);
    }
    if (own) for (const name of FAMILY) this.family.add(`${source.fileName}:${name}`);
    const unitOf = (node: ts.Node): string | undefined => {
      if (ts.isFunctionDeclaration(node) && node.name) return node.name.text;
      if (
        ts.isVariableDeclaration(node) &&
        ts.isIdentifier(node.name) &&
        node.initializer !== undefined &&
        (ts.isArrowFunction(node.initializer) ||
          ts.isFunctionExpression(node.initializer) ||
          ts.isCallExpression(node.initializer))
      )
        return node.name.text;
      if (ts.isMethodDeclaration(node) && ts.isIdentifier(node.name)) return node.name.text;
      return undefined;
    };
    const top = (node: ts.Node) => {
      const name = unitOf(node);
      if (name !== undefined && hasJsx(node)) {
        const unit: Unit = {
          name,
          file: source.fileName,
          found: new Set(),
          loose: false,
          childrenAt: [],
          uses: [],
        };
        into.set(name, [...(into.get(name) ?? []), unit]);
        const body = ts.isVariableDeclaration(node) ? node.initializer : node;
        if (body) this.walk(body, [], unit, source, held, new Set());
        return;
      }
      if (held.isJsx(node)) return;
      ts.forEachChild(node, top);
    };
    top(source);
  }

  private classesOf(expression: ts.Node, held: Held): Carried[] {
    const carried: Carried[] = [];
    const seen = new Set<ts.Node>();
    const visit = (node: ts.Node, always: boolean) => {
      if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) {
        for (const name of node.text.split(/\s+/)) if (name !== "") carried.push({ name, always });
        return;
      }
      if (ts.isTemplateExpression(node)) {
        // A piece next to a `${…}` is part of a class built at run time, never a class itself.
        const pieces = [node.head.text, ...node.templateSpans.map((span) => span.literal.text)];
        /** Whether what a `${…}` adds is set apart from the piece beside it: each string it can
         *  add is empty or starts (`after`) or ends with a space. */
        const apart = (span: ts.TemplateSpan | undefined, after: boolean) => {
          const said: string[] = [];
          const strings = (one: ts.Node) => {
            if (ts.isStringLiteral(one) || ts.isNoSubstitutionTemplateLiteral(one))
              said.push(one.text);
            else ts.forEachChild(one, strings);
          };
          if (span !== undefined) strings(span.expression);
          return (
            said.length > 0 && said.every((one) => one === "" || (after ? /^\s/ : /\s$/).test(one))
          );
        };
        pieces.forEach((piece, at) => {
          const names = piece.split(/\s+/);
          names.forEach((name, n) => {
            const glued =
              (n === 0 &&
                at > 0 &&
                !/^\s/.test(piece) &&
                !apart(node.templateSpans[at - 1], false)) ||
              (n === names.length - 1 &&
                at < pieces.length - 1 &&
                !/\s$/.test(piece) &&
                !apart(node.templateSpans[at], true));
            if (name !== "" && !glued) carried.push({ name, always });
          });
        });
        node.templateSpans.forEach((span) => visit(span.expression, false));
        return;
      }
      const list = ts.isIdentifier(node) ? held.value(node) : undefined;
      if (list !== undefined && !list.jsx) {
        if (!seen.has(list.value)) {
          seen.add(list.value);
          visit(list.value, always);
        }
        return;
      }
      ts.forEachChild(node, (child) => visit(child, always && !CONDITIONAL(node)));
    };
    visit(expression, true);
    return carried;
  }

  private walk(
    node: ts.Node,
    frames: Frame[],
    unit: Unit,
    source: ts.SourceFile,
    held: Held,
    placing: Set<ts.Node>,
  ): void {
    const again = (child: ts.Node, inside: Frame[]) =>
      this.walk(child, inside, unit, source, held, placing);
    if (ts.isJsxElement(node) || ts.isJsxSelfClosingElement(node)) {
      const open = ts.isJsxElement(node) ? node.openingElement : node;
      const tag = open.tagName.getText(source);
      const attributes = open.attributes.properties;
      const attribute = (name: string) =>
        attributes.find(
          (one): one is ts.JsxAttribute =>
            ts.isJsxAttribute(one) && one.name.getText(source) === name,
        );
      const style = attribute("style")?.initializer;
      if (style !== undefined && /\bbackground/i.test(style.getText(source)))
        this.inline.add(`${source.fileName}: <${tag}>`);
      const className = attribute("className")?.initializer;
      const carried = className === undefined ? [] : this.classesOf(className, held);
      const scope = attribute("data-scope")?.initializer;
      const part = attribute("data-part")?.initializer;
      const named = tag.split(".").pop() ?? tag;
      const isButton =
        (scope !== undefined &&
          ts.isStringLiteral(scope) &&
          scope.text === "button" &&
          (part === undefined || (ts.isStringLiteral(part) && part.text === "root"))) ||
        (FAMILY.has(named) && this.family.has(`${source.fileName}:${named}`));
      let inside = frames;
      if (isButton) {
        carried.forEach((one) => this.buttonClasses.add(one.name));
        const drawn = under(frames);
        drawn.tokens.forEach((token) => unit.found.add(token));
        if (!drawn.covered) unit.loose = true;
      } else {
        const frame: Frame = {
          tokens: new Set(),
          covers: false,
          classes: new Set(carried.map((one) => one.name)),
          painted: { tokens: new Set(), covers: false },
        };
        // A see-through ground a rule read whole may paint on this element is drawn over what
        // is under it: in this component, where one of its grounds covers what is under it,
        // and otherwise wherever the component is placed, which is read no further (#4796).
        /** Whether `rule` reaches this element: `exactly`, by what is written on it alone; and
         *  `around`, taking a class it is scoped by as one the component may be placed in. */
        const reaches = (rule: WashRule, exactly: boolean, around = false) =>
          (rule.tag === undefined || rule.tag === tag) &&
          rule.classes.every((name) => carried.some((one) => one.name === name)) &&
          (around || rule.within.every((name) => frames.some((at) => at.classes.has(name)))) &&
          rule.attributes.every(([name, value]) => {
            const written = attribute(name)?.initializer;
            if (attribute(name) === undefined) return false;
            if (value === undefined) return true;
            // One written as an expression may be the value the rule asks for.
            return written !== undefined && ts.isStringLiteral(written)
              ? written.text === value
              : !exactly;
          });
        /** Whether the element carries each class `rule` names, always. */
        const always = (rule: WashRule) =>
          rule.classes.every((name) => carried.some((one) => one.name === name && one.always));
        for (const rule of this.given.paintRules ?? []) {
          if (!reaches(rule, false)) continue;
          frame.painted.tokens.add(rule.token);
          if (rule.fills === true && atRest(rule) && reaches(rule, true) && always(rule))
            frame.painted.covers = true;
        }
        for (const one of carried) {
          ENGINE_GROUNDS.get(one.name)?.forEach((token) => frame.painted.tokens.add(token));
          const drawn = utility(one.name, this.given.aliases);
          if (drawn === undefined) continue;
          drawn.tokens.forEach((token) => frame.painted.tokens.add(token));
          if (drawn.rest && one.always && drawn.fills.size > 0) frame.painted.covers = true;
          // A see-through ground a utility paints is laid as a rule's is (#4876).
          for (const token of drawn.tokens)
            if (this.given.washes?.has(token))
              this.laidUnder(this.utilityRule(one.name, token), frames);
        }
        for (const rule of this.given.washRules ?? []) {
          if (reaches(rule, false)) this.laidUnder(rule, frames);
          // A see-through ground a family names is one what the element holds may stand on
          // (#4795): a pane's warning, a Notice placed in the pane's top.
          if (this.given.familyWashes?.has(rule.token) && reaches(rule, false, true))
            frame.tokens.add(rule.token);
        }
        for (const anchor of this.anchors)
          if (reaches(anchor, true)) this.anchor(anchor.selector, [...frames, frame]);
        // The forms a rule on a tag or an attribute reaches this element by, as it is written.
        const forms = new Set<string>(/^[a-z]/.test(tag) ? [`<${tag}>`] : []);
        for (const one of attributes) {
          if (!ts.isJsxAttribute(one)) continue;
          const name = one.name.getText(source);
          forms.add(`[${name}=]`);
          const value = one.initializer;
          if (value !== undefined && ts.isStringLiteral(value))
            forms.add(`[${name}=${value.text}]`);
        }
        for (const one of forms)
          for (const { scoped, ground } of this.forms.get(one) ?? []) {
            this.reached.add(one);
            if (scoped.every((name) => frames.some((at) => at.classes.has(name))))
              ground.tokens.forEach((token) => frame.tokens.add(token));
          }
        for (const one of carried) {
          const drawn = utility(one.name, this.given.aliases);
          const ground =
            this.byClass.get(one.name) ??
            (drawn === undefined || drawn.fills.size === 0
              ? undefined
              : { tokens: drawn.fills, rest: drawn.rest });
          if (ground === undefined || this.buttonClasses.has(one.name)) continue;
          ground.tokens.forEach((token) => frame.tokens.add(token));
          if (ground.rest && one.always) frame.covers = true;
        }
        if (frame.tokens.size > 0 || frame.classes.size > 0 || frame.painted.tokens.size > 0)
          inside = [...frames, frame];
      }
      const component = /^[A-Z]/.test(tag) && !isButton ? tag.split(".").pop() : undefined;
      if (component !== undefined) unit.uses.push({ name: component, frames: inside });
      // What a component is given is placed where it places its `children`, or around them.
      const places: Frame[][] =
        component === undefined
          ? []
          : (this.units.get(component)?.flatMap((one) => one.childrenAt) ?? []);
      const given = places.length === 0 ? [inside] : places.map((at) => [...inside, ...at]);
      for (const one of attributes) for (const at of given) again(one, at);
      if (ts.isJsxElement(node))
        for (const child of node.children) for (const at of given) again(child, at);
      return;
    }
    if (ts.isIdentifier(node)) {
      if (node.text === "children" && placed(node)) unit.childrenAt.push(frames);
      const one = held.value(node);
      if (one?.jsx === true && !placing.has(one.value)) {
        placing.add(one.value);
        again(one.value, frames);
        placing.delete(one.value);
      }
      return;
    }
    if (ts.isPropertyAccessExpression(node) && node.name.text === "children" && placed(node)) {
      unit.childrenAt.push(frames);
      return;
    }
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression))
      unit.uses.push({ name: node.expression.text, frames });
    if (held.isJsx(node)) return;
    ts.forEachChild(node, (child) => again(child, frames));
  }

  /** **The grounds each see-through ground's element is drawn on** (#4796), by the rule that
   *  paints it: each one a component draws under it, out to the first that covers what is under
   *  it; and whether one is drawn where its own component covers nothing, and so on whatever
   *  the component is placed on, which is read no further (a dialog's content is portaled). */
  readonly laid = new Map<WashRule, { on: Set<Token>; loose: boolean }>();

  /** The rule a utility that paints `token` is read as, one for each class. */
  private utilityRule(name: string, token: Token): WashRule {
    const key = `${name} ${token}`;
    const rule = this.utilityRules.get(key) ?? {
      token,
      selector: `.${name.replace(/[^\w-]/g, (one) => `\\${one}`)}`,
      classes: [name],
      attributes: [],
      within: [],
    };
    this.utilityRules.set(key, rule);
    return rule;
  }

  private anchor(at: string, frames: Frame[]): void {
    const drawn = beneath(frames);
    const one = this.anchored.get(at) ?? { on: new Set<Token>(), loose: false, drawn: 0 };
    drawn.tokens.forEach((token) => one.on.add(token));
    one.loose ||= !drawn.covered;
    one.drawn += 1;
    this.anchored.set(at, one);
  }

  private laidUnder(rule: WashRule, frames: Frame[]): void {
    const drawn = beneath(frames);
    const laid = this.laid.get(rule) ?? { on: new Set<Token>(), loose: false };
    if (drawn.covered) drawn.tokens.forEach((token) => laid.on.add(token));
    else laid.loose = true;
    this.laid.set(rule, laid);
  }

  /** Every ground a button is drawn on: each unit's own, and through the units it renders,
   *  out to the window's ground for what no unit's ground covers. */
  grounds(window: Set<Token>): Map<Token, string[]> {
    const loose = new Set<string>();
    for (const [name, units] of this.units) if (units.some((one) => one.loose)) loose.add(name);
    for (let changed = true; changed;) {
      changed = false;
      for (const [name, units] of this.units)
        if (
          !loose.has(name) &&
          units.some((unit) =>
            unit.uses.some((use) => loose.has(use.name) && !under(use.frames).covered),
          )
        ) {
          loose.add(name);
          changed = true;
        }
    }
    const found = new Map<Token, string[]>([...window].map((token) => [token, [":root"]]));
    const add = (token: Token, where: string) =>
      found.set(token, [...(found.get(token) ?? []), where]);
    for (const units of this.units.values())
      for (const unit of units) {
        const where = `${unit.file}: ${unit.name}`;
        unit.found.forEach((token) => add(token, where));
        for (const use of unit.uses)
          if (loose.has(use.name))
            under(use.frames).tokens.forEach((token) => add(token, `${where} > ${use.name}`));
      }
    return found;
  }
}

/** A variable's value as a name in one file sees it: JSX, read where it is placed, or any
 *  other value, which a `className` may name. */
type Value = { jsx: boolean; value: ts.Node };

/** Whether `node` is a scope a `const`, a `let` or a parameter is seen in. */
function isScope(node: ts.Node): boolean {
  return (
    ts.isSourceFile(node) ||
    ts.isBlock(node) ||
    ts.isModuleBlock(node) ||
    ts.isCaseBlock(node) ||
    ts.isCatchClause(node) ||
    ts.isForStatement(node) ||
    ts.isForInStatement(node) ||
    ts.isForOfStatement(node) ||
    ts.isFunctionLike(node)
  );
}

/** Whether `node` names a property, an attribute or a declared binding rather than a variable
 *  it reads: `said` in `mark.said`, `<Badge said="…">`, `{ said: 1 }` or `const { said } = …`
 *  (#3783). */
function isPropertyName(node: ts.Identifier): boolean {
  const parent = node.parent;
  if (ts.isPropertyAccessExpression(parent)) return parent.name === node;
  if (ts.isQualifiedName(parent)) return parent.right === node;
  if (ts.isBindingElement(parent) || ts.isImportSpecifier(parent) || ts.isExportSpecifier(parent))
    return parent.name === node || parent.propertyName === node;
  if (
    ts.isPropertyAssignment(parent) ||
    ts.isJsxAttribute(parent) ||
    ts.isPropertyDeclaration(parent) ||
    ts.isPropertySignature(parent) ||
    ts.isMethodDeclaration(parent) ||
    ts.isMethodSignature(parent) ||
    ts.isGetAccessorDeclaration(parent) ||
    ts.isSetAccessorDeclaration(parent) ||
    ts.isEnumMember(parent)
  )
    return parent.name === node;
  return (
    ts.isLabeledStatement(parent) ||
    ts.isBreakOrContinueStatement(parent) ||
    ts.isJsxNamespacedName(parent)
  );
}

/** The nearest scope around `node`, `node` itself not counted; or with `hoisted`, the nearest
 *  function or file, where a `var` is seen. */
function scopeAround(node: ts.Node, hoisted = false): ts.Node {
  let at = node.parent;
  while (!(hoisted ? ts.isSourceFile(at) || ts.isFunctionLike(at) : isScope(at))) at = at.parent;
  return at;
}

/**
 * **Each variable of one file, in the scope it is declared in** (#3783): a name is read as the
 * variable the nearest scope around it declares, so a component's `said` is never another's,
 * and a parameter, a function, a class or an import of the same name hides what is outside it.
 */
class Held {
  private readonly scopes = new Map<ts.Node, Map<string, Value | null>>();
  /** Each variable declaration with a value, by its node. */
  private readonly declared = new Map<ts.Node, Value>();

  constructor(source: ts.SourceFile) {
    const visit = (node: ts.Node) => {
      if (ts.isVariableDeclaration(node)) {
        const list = node.parent;
        const scope = scopeAround(
          node,
          ts.isVariableDeclarationList(list) && (list.flags & ts.NodeFlags.BlockScoped) === 0,
        );
        if (ts.isIdentifier(node.name) && node.initializer !== undefined) {
          const value = { jsx: hasJsx(node.initializer), value: node.initializer };
          this.declared.set(node, value);
          this.declare(scope, node.name.text, value);
        } else this.shadow(scope, node.name);
      } else if (ts.isParameter(node)) this.shadow(node.parent, node.name);
      else if ((ts.isFunctionDeclaration(node) || ts.isClassDeclaration(node)) && node.name)
        this.declare(scopeAround(node), node.name.text, null);
      else if ((ts.isFunctionExpression(node) || ts.isClassExpression(node)) && node.name)
        this.declare(node, node.name.text, null);
      else if (ts.isImportClause(node) || ts.isNamespaceImport(node) || ts.isImportSpecifier(node))
        if (node.name !== undefined) this.declare(source, node.name.text, null);
      ts.forEachChild(node, visit);
    };
    visit(source);
  }

  /** Whether `node` declares a variable whose value is JSX, which is read where it is placed. */
  isJsx(node: ts.Node): boolean {
    return this.declared.get(node)?.jsx === true;
  }

  /** The value of the variable `name` reads, if it reads one this file gives a value. */
  value(name: ts.Identifier): Value | undefined {
    if (isPropertyName(name)) return undefined;
    for (let at: ts.Node | undefined = name.parent; at !== undefined; at = at.parent) {
      const seen = isScope(at) ? this.scopes.get(at)?.get(name.text) : undefined;
      if (seen !== undefined) return seen ?? undefined;
    }
    return undefined;
  }

  private declare(scope: ts.Node, name: string, value: Value | null): void {
    const names = this.scopes.get(scope) ?? new Map<string, Value | null>();
    names.set(name, value);
    this.scopes.set(scope, names);
  }

  /** Each name a parameter or a destructuring declares, with no value to follow. */
  private shadow(scope: ts.Node, name: ts.BindingName): void {
    if (ts.isIdentifier(name)) this.declare(scope, name.text, null);
    else
      for (const element of name.elements)
        if (!ts.isOmittedExpression(element)) this.shadow(scope, element.name);
  }
}

/** Whether `node` is placed in JSX as it is (`{children}`, `{props.children ?? none}`). */
function placed(node: ts.Node): boolean {
  let at: ts.Node = node;
  while (
    ts.isParenthesizedExpression(at.parent) ||
    ts.isConditionalExpression(at.parent) ||
    ts.isBinaryExpression(at.parent) ||
    ts.isCallExpression(at.parent) ||
    (ts.isPropertyAccessExpression(at.parent) && at.parent.expression === at)
  )
    at = at.parent;
  return ts.isJsxExpression(at.parent);
}

function hasJsx(node: ts.Node): boolean {
  if (ts.isJsxElement(node) || ts.isJsxSelfClosingElement(node) || ts.isJsxFragment(node))
    return true;
  return ts.forEachChild(node, (child) => hasJsx(child) || undefined) === true;
}

/** Reads `files` against `css`: every ground a button of the family is drawn on, each with
 *  the components it was found in; and what the reader could not read. */
function readGrounds(
  files: { name: string; text: string }[],
  css: string[],
  washes: ReadonlySet<Token> = new Set(),
  familyWashes: ReadonlySet<Token> = new Set(),
  anchors: readonly string[] = [],
): {
  found: Map<Token, string[]>;
  laid: Map<WashRule, { on: Set<Token>; loose: boolean } | undefined>;
  anchored: Map<string, Anchored>;
  unread: Unread;
  inline: string[];
} {
  const { byClass, window, unread } = groundsOfClasses(css);
  // What a helper's spread attributes or the theme engine's layout words draw, by class.
  for (const [name, tokens] of ENGINE_GROUNDS) {
    const ground = byClass.get(name) ?? { tokens: new Set<Token>(), rest: false };
    tokens.forEach((token) => ground.tokens.add(token));
    byClass.set(name, ground);
  }
  const rules = washRulesOf(css, washes);
  const reader = new Reader(files, byClass, {
    washRules: rules,
    paintRules: paintRulesOf(css),
    familyWashes,
    washes,
    aliases: aliasesOf(css),
    anchors,
  });
  // The first pass finds the buttons' classes and each component's `children` places, the next
  // ones read what is placed in them; a third settles a component placed in another's children.
  for (let pass = 0; pass < 3; pass += 1) reader.read();
  // A rule on an attribute no element carries as written is one the reader cannot tie to one:
  // the window sets it as it runs, or spells it in an expression.
  for (const [form, selectors] of unread.byAttribute)
    if (!reader.reached.has(form)) unread.classless.push(...selectors);
  const laid = new Map(
    [...rules, ...reader.utilityRules.values()].map((rule) => [rule, reader.laid.get(rule)]),
  );
  return {
    found: reader.grounds(window),
    laid,
    anchored: reader.anchored,
    unread,
    inline: [...reader.inline].sort(),
  };
}

function buttonGroundsWhere(
  files: { name: string; text: string }[],
  css: string[],
): Map<Token, string[]> {
  return readGrounds(files, css).found;
}

/** Every ground a button of the family is drawn on, in order. */
function buttonGrounds(files: { name: string; text: string }[], css: string[]): string[] {
  return [...buttonGroundsWhere(files, css).keys()].sort();
}

/**
 * The background rules the reader cannot tie to an element the components write, each with why
 * no button is drawn on what it paints. A new one fails the test below until it is read or
 * listed here.
 */
/**
 * **The grounds a family draws on an element whose attributes a helper hands it** (#2914), each
 * listed by the class the element carries: the reader ties a rule to an element by attributes
 * written out, and a family's helpers (`tabsPart`, `panelRoot`, `editorToolbar`) spread theirs.
 * Each family lists its own (`spreadGrounds`, in `ui/<family>/theme.testkit.ts`).
 */
const ENGINE_GROUNDS: ReadonlyMap<string, readonly Token[]> = new Map(
  FAMILY_THEMES.flatMap((family) => family.spreadGrounds ?? []),
);

const CLASSLESS: readonly string[] = [
  // The band a chat row takes for a moment once it is revealed (`revealTask.ts` sets the
  // attribute as it runs); the row is a `.ui-row`, which holds no button.
  ".chats-section [data-revealed]",
  // Selected words, not a box anything stands on.
  ':root[data-theme-draws~="selection.background"] :not(.cm-editor, .cm-editor *)::selection',
  // The line between the bottom panel's halves, a handle the panel library draws, which holds
  // nothing.
  ".panel-halves > [data-separator]",
  // And each family's own (`classless`, in `ui/<family>/theme.testkit.ts`).
  ...FAMILY_THEMES.flatMap((family) => family.classless ?? []),
];

/**
 * The background rules that reach an element by what it holds (`:has(…)`). Each is read on the
 * element it paints, whatever it holds, so it counts as a ground there and never hides the one
 * under it; a class inside the `:has(…)` is another element's and is not read as a ground. Each
 * is listed so a new one is looked at.
 */
const WITHIN: readonly string[] = [
  ".chats-section li:has(> .ui-row[data-revealed])",
  // A pane's consent (`paneFrame.css`, #3142): it holds an Allow, and is on the warning wash
  // whatever its tone, a translucent wash over the terminal's fill, which the button family lists.
  '.pane-top\n  [data-scope="notice"][data-part="root"][data-placement="pane"]:has([data-variant="grants"])',
  // And its mark, a pseudo-element no button is drawn on.
  '.pane-top\n  [data-scope="notice"][data-part="root"][data-placement="pane"]:has(\n    [data-variant="grants"]\n  )::before',
];

// --- The tests ------------------------------------------------------------------------------

describe("the grounds a button is drawn on", () => {
  const css = (rules: string) => [`:root { background: var(--surface-base); }\n${rules}`];
  const file = (name: string, text: string) => ({ name, text });

  it("are each element's around it, out to the first with a background at rest", () => {
    const found = buttonGrounds(
      [
        file(
          "A.tsx",
          `export function A() {
             return <div className="raised"><div className="row"><button data-scope="button" data-part="root" /></div></div>;
           }`,
        ),
      ],
      css(
        `.raised { background: var(--surface-raised); } .row:hover { background: var(--surface-hover); }`,
      ),
    );
    expect(found).toEqual(["surface.base", "surface.hover", "surface.raised"]);
  });

  it("follow a component to where it is placed, and children to where they are placed", () => {
    const found = buttonGrounds(
      [
        file(
          "Fix.tsx",
          `export function Fix() { return <button data-scope="button" data-part="root">Fix</button>; }`,
        ),
        file(
          "Box.tsx",
          `export function Box({ children }) { return <section className="trouble">{children}</section>; }`,
        ),
        file(
          "Pane.tsx",
          `export function Pane() {
             const line = <div className="line"><Fix /></div>;
             return <Box><div className="sunken">{line}</div></Box>;
           }`,
        ),
      ],
      css(
        `.trouble { background: var(--danger-surface); } .sunken { background: var(--surface-sunken); }`,
      ),
    );
    expect(found).toEqual(["surface.base", "surface.sunken"]);
  });

  it("count a class carried on a condition without hiding the ground under it", () => {
    const found = buttonGrounds(
      [
        file(
          "N.tsx",
          `export function N({ alone }) {
             const classes = ["line", alone ? "trouble" : ""].join(" ");
             return <div className="raised"><div className={classes}><button data-scope="button" data-part="root" className={alone ? "sm" : ""} /></div></div>;
           }`,
        ),
      ],
      css(
        `.raised { background: var(--surface-raised); } .trouble { background: var(--danger-surface); }`,
      ),
    );
    expect(found).toEqual(["danger.surface", "surface.base", "surface.raised"].sort());
  });

  it("read a rule on a tag or an attribute inside the classes it is scoped by", () => {
    const found = buttonGrounds(
      [
        file(
          "T.tsx",
          `export function T() {
             return <><table className="secrets"><tr><td><button data-scope="button" data-part="root" /></td></tr></table>
               <table className="other"><tr><td><button data-scope="button" data-part="root" /></td></tr></table>
               <div role="tab"><button data-scope="button" data-part="root" /></div></>;
           }`,
        ),
      ],
      css(
        `.secrets tr:hover { background: var(--layer-selected); } .tabs-only [role="tab"] { background: var(--tab-active); } [role="tab"]:hover { background: var(--surface-hover); }`,
      ),
    );
    expect(found).toEqual(["layer.selected", "surface.base", "surface.hover"]);
  });

  it("never take a class inside :has() as the element a rule paints", () => {
    const found = buttonGrounds(
      [
        file(
          "H.tsx",
          `export function H() { return <li className="row"><button data-scope="button" data-part="root" /></li>; }`,
        ),
      ],
      css(`li:has(> .row) { background: var(--surface-raised); }`),
    );
    expect(found).toEqual(["surface.base", "surface.raised"]);
  });

  it("count a see-through ground a family names, where a class around its component scopes it", () => {
    // A pane's warning (#4795): the Notice is its own component, placed in the pane's top, so the
    // class the rule is scoped by is around the component, not in it.
    const files = [
      file(
        "Note.tsx",
        `export function Note({ at, children }) {
           return <div data-scope="note" data-placement={at}>{children}</div>;
         }`,
      ),
      file(
        "Pane.tsx",
        `export function Pane() {
           return <div className="top"><Note at="pane"><button data-scope="button">Allow</button></Note></div>;
         }`,
      ),
    ];
    const sheet = css(
      `.top { background: var(--terminal-background); }
       .top :is([data-scope="note"][data-placement="pane"]) { background: var(--wash-warning); }`,
    );
    const washes = new Set<Token>(["wash.warning"]);
    const named = readGrounds(files, sheet, washes, washes).found;
    expect([...named.keys()].sort()).toEqual([
      "surface.base",
      "terminal.background",
      "wash.warning",
    ]);
    // A see-through ground no family names is not one a button is read on.
    const unnamed = readGrounds(files, sheet, washes, new Set()).found;
    expect([...unnamed.keys()].sort()).toEqual(["surface.base", "terminal.background"]);
  });

  it("count each member of the button family as a button, and its parts as its own", () => {
    const found = buttonGrounds(
      [
        file(
          "ui/button/Button.tsx",
          `export function Button({ children }) {
             return <span data-scope="button" data-part="label">{children}</span>;
           }`,
        ),
        file(
          "Away.tsx",
          `import { Button, IconButton as Icon } from "./ui/button";
           export function Away() {
             return <div className="far"><Button>Back</Button><Icon /></div>;
           }`,
        ),
      ],
      css(`.far { background: var(--terminal-background); }`),
    );
    expect(found).toEqual(["surface.base", "terminal.background"]);
  });

  it("never count a button's own fill, or a utility it carries, as a ground", () => {
    const found = buttonGrounds(
      [
        file(
          "B.tsx",
          `export function B() { return <div className="bg-surface-sunken"><button data-scope="button" data-part="root" className="fix" /></div>; }`,
        ),
      ],
      css(
        `.fix { background: var(--accent-surface); } [data-scope="button"][data-part="root"] { background: var(--control-base); }`,
      ),
    );
    expect(found).toEqual(["surface.base", "surface.sunken"]);
  });

  it("read a ground however it is painted: a fallback, an alias, a utility, a solid gradient (#4971)", () => {
    const found = buttonGrounds(
      [
        file(
          "G.tsx",
          `export function G() {
             return <>
               <div className="fallback"><button data-scope="button" data-part="root" /></div>
               <div className="aliased"><button data-scope="button" data-part="root" /></div>
               <div className="solid"><button data-scope="button" data-part="root" /></div>
               <div className="bg-panel"><button data-scope="button" data-part="root" /></div>
               <div className="bg-(--well)"><button data-scope="button" data-part="root" /></div>
             </>;
           }`,
        ),
      ],
      css(
        `@theme { --color-panel: var(--surface-overlay); }
         :root { --pane-ground: var(--surface-sunken); --well: var(--danger-surface); }
         .fallback { background: var(--unset, var(--surface-raised)); }
         .aliased { background-color: var(--pane-ground); }
         .solid { background-image: linear-gradient(var(--surface-deep), var(--surface-deep)), none; }`,
      ),
    );
    expect(found).toEqual([
      "danger.surface",
      "surface.base",
      "surface.deep",
      "surface.overlay",
      "surface.raised",
      "surface.sunken",
    ]);
  });

  it("never read a decoration as a ground, nor let one hide the ground under it (#4971)", () => {
    const found = buttonGrounds(
      [
        file(
          "T.tsx",
          `export function T() {
             return <div className="raised"><div className="guided"><div className="fade">
               <div className="bg-[repeating-linear-gradient(to_right,var(--tree-guide)_0_1px,transparent_1px_8px)]">
                 <button data-scope="button" data-part="root" />
               </div>
             </div></div></div>;
           }`,
        ),
      ],
      css(
        `.raised { background: var(--surface-raised); }
         .guided {
           background-image: repeating-linear-gradient(to right, var(--tree-guide) 0 1px, transparent 1px 8px);
         }
         .fade { background: linear-gradient(to bottom, var(--surface-hover), transparent); }`,
      ),
    );
    expect(found).toEqual(["surface.base", "surface.raised"]);
  });

  it("never follow a property's name, or an attribute's, as a variable (#3783)", () => {
    const found = buttonGrounds(
      [
        file(
          "M.tsx",
          `export function Menu() {
             const more = <button data-scope="button" data-part="root" />;
             return <div className="raised">{more}</div>;
           }
           export function Tag({ mark }) {
             return <span className="badge" more="1">{mark.more}</span>;
           }`,
        ),
      ],
      css(
        `.raised { background: var(--surface-raised); } .badge { background: var(--accent-surface); }`,
      ),
    );
    expect(found).toEqual(["surface.base", "surface.raised"]);
  });

  it("look a held value up in its own component, never in another's of the same name (#3783)", () => {
    const found = buttonGrounds(
      [
        file(
          "S.tsx",
          `export function Quiet() {
             const said = <em>quiet</em>;
             return <span className="badge">{said}</span>;
           }
           export function Loud() {
             const said = <button data-scope="button" data-part="root" />;
             return <div className="raised">{said}</div>;
           }`,
        ),
      ],
      css(
        `.raised { background: var(--surface-raised); } .badge { background: var(--accent-surface); }`,
      ),
    );
    expect(found).toEqual(["surface.base", "surface.raised"]);
  });

  it("never follow a name a parameter hides (#3783)", () => {
    const found = buttonGrounds(
      [
        file(
          "P.tsx",
          `const more = <button data-scope="button" data-part="root" />;
           export function Menu() { return <div className="raised">{more}</div>; }
           export function Tag({ more }) { return <span className="badge">{more}</span>; }
           export function Row(more) { return <span className="badge">{more}</span>; }`,
        ),
      ],
      css(
        `.raised { background: var(--surface-raised); } .badge { background: var(--accent-surface); }`,
      ),
    );
    expect(found).toEqual(["surface.base", "surface.raised"]);
  });

  it("never follow a name a function, a property or a getter of the same name hides (#3783)", () => {
    const found = buttonGrounds(
      [
        file(
          "F.tsx",
          `const said = <button data-scope="button" data-part="root" />;
           export function Menu() { return <div className="raised">{said}</div>; }
           export function Badge() {
             function said() { return "x"; }
             return <span className="badge">{said()}</span>;
           }
           export function Tag() {
             return <span className="badge">{f({ said: 1 })}{f({ get said() { return 1; } })}</span>;
           }`,
        ),
      ],
      css(
        `.raised { background: var(--surface-raised); } .badge { background: var(--accent-surface); }`,
      ),
    );
    expect(found).toEqual(["surface.base", "surface.raised"]);
  });

  it("still follow a value read in a default, and a var in its whole function (#3783)", () => {
    const found = buttonGrounds(
      [
        file(
          "V.tsx",
          `const more = <button data-scope="button" data-part="root" />;
           export function Card({ icon = more }) { return <div>{icon}</div>; }
           export function Page() { return <section className="sunken"><Card /></section>; }
           export function Menu({ open }) {
             if (open) { var shown = <button data-scope="button" data-part="root" />; }
             return <div className="raised">{shown}</div>;
           }`,
        ),
      ],
      css(
        `.raised { background: var(--surface-raised); } .sunken { background: var(--surface-sunken); }`,
      ),
    );
    expect(found).toEqual(["surface.base", "surface.raised", "surface.sunken"]);
  });

  it("look a class list up in its own component too (#3783)", () => {
    const found = buttonGrounds(
      [
        file(
          "C.tsx",
          `export function Plain() {
             const classes = "badge";
             return <span className={classes}>quiet</span>;
           }
           export function Loud() {
             const classes = "raised";
             return <div className={classes}><button data-scope="button" data-part="root" /></div>;
           }`,
        ),
      ],
      css(
        `.raised { background: var(--surface-raised); } .badge { background: var(--accent-surface); }`,
      ),
    );
    expect(found).toEqual(["surface.base", "surface.raised"]);
  });

  /** The window, read once for the tests below, with the see-through grounds the families name. */
  const SEE_THROUGH = shippedSeeThrough();
  const familyWashes = new Set(
    [...engineGrounds().values()]
      .flat()
      .filter((token): token is Token => SEE_THROUGH.has(token as Token)),
  );
  const read = readGrounds(sources(SRC), sheets(), SEE_THROUGH, familyWashes);

  it("are, in the window, exactly the grounds the button family's manifest lists", () => {
    const listed = engineGrounds().get("button");
    expect(listed, "the button family's grounds in parts_button.rs").toBeDefined();
    const where = read.found;
    const found = [...where.keys()].sort();
    expect(found).toContain("surface.base");
    expect(found).toContain("danger.surface");
    // A ground the manifest does not list is named with the components a button is drawn on it in.
    const unlisted = found
      .filter((token) => !listed?.includes(token))
      .map((token) => `${token}: ${(where.get(token) ?? []).slice(0, 3).join("; ")}`);
    expect(unlisted).toEqual([]);
    expect([...(listed ?? [])].sort()).toEqual(found);
  });

  /**
   * **What the reader cannot tie to a component fails here** until it is read or listed below
   * with why it is no ground a button is drawn on: a background in an element's own `style`, a
   * background rule whose element is named by an attribute or a tag rather than a class, and a
   * background rule that only reaches an element by what it holds (`:has(…)`).
   */

  it("are never set in an element's own style", () => {
    expect(read.inline).toEqual([]);
  });

  it("are set by a class, or the rule is listed with why no button is drawn on it", () => {
    expect([...new Set(read.unread.classless)].sort()).toEqual([...CLASSLESS].sort());
  });

  it("are never reached only by what an element holds, or the rule is listed with why", () => {
    expect([...new Set(read.unread.within)].sort()).toEqual([...WITHIN].sort());
  });
});

// --- Where each see-through ground is laid (#4796) ------------------------------------------

/**
 * **The tokens purlis ships see-through**: each one an appearance of a built-in theme writes with
 * an alpha under `ff`. A ground in one of these shows what it is drawn over, so the parts check
 * reads what stands on it over each ground `WASH_LAID_ON` (in `parts.rs`) says it is laid on.
 */
function shippedSeeThrough(): Set<Token> {
  const found = new Set<Token>();
  for (const theme of ["purlis", "purlis-classic"]) {
    const read = JSON.parse(readFileSync(join(THEME, "built-in", theme, "theme.json"), "utf8")) as {
      appearances: { tokens?: Record<string, unknown> }[];
    };
    for (const appearance of read.appearances)
      for (const [token, value] of Object.entries(appearance.tokens ?? {}))
        if (typeof value === "string" && /^#[0-9a-f]{6}(?![fF]{2})[0-9a-f]{2}$/i.test(value))
          found.add(token as Token);
  }
  return found;
}

/** `WASH_LAID_ON` as `parts.rs` writes it: each see-through ground, with the grounds the window
 *  lays it on. */
function engineWashes(): Map<string, string[]> {
  const parts = readFileSync(join(THEME, "parts.rs"), "utf8");
  const table = /\nconst WASH_LAID_ON: [^=]*=\s*&\[([\s\S]*?)\];/.exec(parts)?.[1] ?? "";
  return new Map(
    [...table.matchAll(/\(\s*"([^"]+)",\s*&\[([^\]]*)\]\s*\)/g)].map(([, wash, under]) => [
      wash,
      [...under.matchAll(/"([^"]+)"/g)].map(([, token]) => token).sort(),
    ]),
  );
}

/** A rule's name in `PAINTED`: the see-through token and the selector, its spaces as one. */
const named = (rule: WashRule) => `${rule.token} ${rule.selector.replace(/\s+/g, " ").trim()}`;

/** What a rule that paints a see-through ground is, where the reader cannot find it all:
 *
 * - a ground, `on` the grounds named, which the window draws under the element `at` names (the
 *   class or the attributes the rule is scoped by), and the reader holds them to it (#4876);
 * - a part's own fill, which nothing of another family stands on and the family's own look is
 *   held to (`own`);
 * - a wash in the editor (`editor/look.ts`), which CodeMirror draws in its own elements: laid on
 *   the grounds named, innermost first, the last the editor's own, and nothing of a family stands
 *   on it; `check.rs` reads the editor's words on it over that stack (#4876).
 */
type Painted =
  { on: readonly Token[]; at: string } | { own: string } | { editor: readonly Token[] };

/** Every element a `PAINTED` ground is held to. */
function anchorsOf(painted: ReadonlyMap<string, Painted>): string[] {
  return [...new Set([...painted.values()].flatMap((one) => ("at" in one ? [one.at] : [])))].sort();
}

/** Each listed ground the window does not draw under the element it is held to (#4876). */
function misplaced(
  painted: ReadonlyMap<string, Painted>,
  anchored: ReadonlyMap<string, Anchored>,
): string[] {
  const said: string[] = [];
  for (const [name, listed] of painted) {
    if (!("at" in listed)) continue;
    const at = anchored.get(listed.at);
    const on = [...listed.on].sort().join(", ");
    const drawn = [...(at?.on ?? [])].sort().join(", ");
    if (at === undefined || at.drawn === 0)
      said.push(`${name}: listed on ${on}, but no component draws ${listed.at}`);
    else if (at.loose)
      said.push(
        `${name}: listed on ${on}, but ${listed.at} is drawn on ${drawn === "" ? "" : `${drawn} and `}nothing the reader finds`,
      );
    else if (drawn !== on)
      said.push(`${name}: listed on ${on}, but ${listed.at} is drawn on ${drawn}`);
  }
  return said.sort();
}

/** **The editor's look as a stylesheet** (#4876): each `EditorView.theme({…})` under `src/`,
 *  its `&` the editor's root (`.cm-editor`), as CodeMirror writes it into the page. */
function editorLooks(dir = SRC): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return editorLooks(path);
    if (!/\.tsx?$/.test(entry.name) || /\.(test|stories)\.tsx?$/.test(entry.name)) return [];
    const text = readFileSync(path, "utf8");
    if (!text.includes("EditorView.theme(")) return [];
    const source = ts.createSourceFile(entry.name, text, ts.ScriptTarget.Latest, true);
    const rules: string[] = [];
    const visit = (node: ts.Node) => {
      const look = ts.isCallExpression(node) ? node.arguments[0] : undefined;
      if (
        ts.isCallExpression(node) &&
        node.expression.getText(source) === "EditorView.theme" &&
        look !== undefined &&
        ts.isObjectLiteralExpression(look)
      )
        for (const rule of look.properties) {
          if (!ts.isPropertyAssignment(rule) || !ts.isObjectLiteralExpression(rule.initializer))
            continue;
          if (!ts.isStringLiteral(rule.name) && !ts.isIdentifier(rule.name)) continue;
          const body = rule.initializer.properties.flatMap((one) =>
            ts.isPropertyAssignment(one) &&
            (ts.isIdentifier(one.name) || ts.isStringLiteral(one.name)) &&
            (ts.isStringLiteral(one.initializer) ||
              ts.isNoSubstitutionTemplateLiteral(one.initializer))
              ? [
                  `${one.name.text.replace(/[A-Z]/g, (up) => `-${up.toLowerCase()}`)}: ${one.initializer.text};`,
                ]
              : [],
          );
          const selector = rule.name.text
            .replace(/&(?!\.cm-editor(?![\w-]))/g, ".cm-editor")
            .replace(/&/g, "");
          rules.push(`${selector} { ${body.join(" ")} }`);
        }
      ts.forEachChild(node, visit);
    };
    visit(source);
    return [rules.join("\n")];
  });
}

/** The ground the editor's look paints its root (`&`) with. */
function editorGround(): Token[] {
  return paintRulesOf(editorLooks())
    .filter((rule) => rule.selector === ".cm-editor")
    .map((rule) => rule.token);
}

/** **The core's token pairs** (`check.rs`), as `ground over under over …`: each pair's ground,
 *  then what it is read over, innermost first. */
function corePairs(): Set<string> {
  const check = readFileSync(join(THEME, "check.rs"), "utf8");
  return new Set(
    [...check.matchAll(/\bp\(\s*"[^"]+",\s*"([^"]+)",[^)]*\)((?:\s*\.over\("[^"]+"\))*)/g)].map(
      ([, ground, overs]) =>
        [ground, ...[...overs.matchAll(/"([^"]+)"/g)].map(([, one]) => one)].join(" over "),
    ),
  );
}

/** Each wash the editor paints that the core reads nothing over as it is laid, or that is laid
 *  on something other than the editor's own ground. */
function unpaired(
  painted: ReadonlyMap<string, Painted>,
  pairs: ReadonlySet<string>,
  ground: readonly Token[],
): string[] {
  const said: string[] = [];
  for (const [name, listed] of painted) {
    if (!("editor" in listed)) continue;
    const token = name.split(" ")[0];
    const stack = listed.editor.join(" over ");
    if (!pairs.has(`${token} over ${stack}`))
      said.push(`${name}: check.rs reads nothing on ${token} over ${stack}`);
    const last = listed.editor[listed.editor.length - 1];
    if (!ground.includes(last))
      said.push(`${name}: laid on ${last}, but the editor's ground is ${ground.join(", ")}`);
  }
  return said.sort();
}

/**
 * **The see-through grounds the window draws, as the stylesheets paint them**: each rule's
 * element is followed out through the component that draws it to the first ground that covers
 * what is under it (a ground a rule paints on an element by its attributes alone, at rest, covers
 * too), and the grounds found are the ones it is laid on. Where the reader cannot follow it all
 * (it is drawn by a component it reads no further out of, or under a class a component around it
 * carries), `painted` says what it is laid on. Each list is what the window lays the token on.
 */
function laidGrounds(
  laid: Map<WashRule, { on: Set<Token>; loose: boolean } | undefined>,
  painted: ReadonlyMap<string, Painted>,
): { laid: Map<string, string[]>; unlisted: string[]; stale: string[] } {
  const byName = new Map<string, { token: Token; on: Set<Token>; read: boolean }>();
  for (const [rule, found] of laid) {
    const one = byName.get(named(rule)) ?? { token: rule.token, on: new Set<Token>(), read: true };
    found?.on.forEach((token) => one.on.add(token));
    if (found === undefined || found.loose) one.read = false;
    byName.set(named(rule), one);
  }
  const out = new Map<string, Set<Token>>();
  const unlisted: string[] = [];
  const stale = [...painted.keys()].filter((name) => !byName.has(name));
  for (const [name, { token, on, read }] of byName) {
    const listed = painted.get(name);
    // A part's own fill, and a wash in the editor, are no ground a family stands on.
    if (listed !== undefined && ("own" in listed || "editor" in listed)) continue;
    if (listed === undefined && !read) {
      unlisted.push(
        `${name}: drawn on ${on.size === 0 ? "nothing the reader finds" : [...on].join(", ")}`,
      );
      continue;
    }
    if (listed !== undefined && read)
      stale.push(`${name}: the reader finds it on ${[...on].join(", ")}`);
    const grounds = out.get(token) ?? new Set<Token>();
    on.forEach((ground) => grounds.add(ground));
    listed?.on.forEach((ground) => grounds.add(ground));
    out.set(token, grounds);
  }
  return {
    laid: new Map([...out].map(([token, on]) => [token, [...on].sort()])),
    unlisted: unlisted.sort(),
    stale: stale.sort(),
  };
}

/** Where `laid` and `table` part: each see-through ground laid on a ground the table does not
 *  name, and each the table names that nothing lays it on. */
function unsaid(laid: Map<string, string[]>, table: Map<string, string[]>): string[] {
  const said: string[] = [];
  for (const [wash, on] of laid)
    for (const ground of on)
      if (!table.get(wash)?.includes(ground))
        said.push(`${wash} over ${ground}: not in WASH_LAID_ON`);
  for (const [wash, on] of table)
    for (const ground of on)
      if (!laid.get(wash)?.includes(ground)) said.push(`${wash} over ${ground}: never drawn`);
  return said.sort();
}

/**
 * **The rules that paint a see-through ground the reader cannot follow all the way**, each with
 * what it is: the grounds it is laid on, or a part's own fill with why nothing else stands on it.
 * A new one fails until it is read or listed here, and one the reader comes to read fails until
 * it is taken off.
 */
/** A dialog's root, which its content is laid on (`dialog.css`), wherever it is portaled. */
const DIALOG = '[data-scope="dialog"][data-part="root"]';

const PAINTED: ReadonlyMap<string, Painted> = new Map<string, Painted>([
  // A refusal's or an unrun command's box, and the approval of what an extension runs as you:
  // in a dialog (`App.css`), whose content is portaled to the overlay surface (`dialog.css`).
  ['danger.wash [data-scope="dialog"] .honest.mid-turn', { on: ["surface.overlay"], at: DIALOG }],
  ['danger.wash [data-scope="dialog"] .honest.approve', { on: ["surface.overlay"], at: DIALOG }],
  ['danger.wash [data-scope="dialog"] .runs-as-you', { on: ["surface.overlay"], at: DIALOG }],
  // A pane's warnings (`paneFrame.css`), over the terminal's fill under the pane's top: trouble,
  // a warning, and a consent, each a Notice the pane's band places.
  [
    'danger.wash .pane-top [data-scope="notice"][data-part="root"][data-placement="pane"][data-variant="trouble"]',
    { on: ["terminal.background"], at: ".pane-top" },
  ],
  [
    'wash.warning .pane-top :is(.notice-pane-box, [data-scope="notice"][data-part="root"][data-placement="pane"])',
    { on: ["terminal.background"], at: ".pane-top" },
  ],
  [
    'wash.warning .pane-top [data-scope="notice"][data-part="root"][data-placement="pane"]:has([data-variant="grants"])',
    { on: ["terminal.background"], at: ".pane-top" },
  ],
  // A part's own fill, its words and marks the family's own parts on it.
  [
    'accent.wash [data-scope="activity"][data-part="item"][data-selected]',
    { own: "a chosen activity item; its icon and count are its own" },
  ],
  [
    'accent.wash [data-scope="view-nav"][data-part="item"][data-selected]',
    { own: "a chosen view's item; its words are its own" },
  ],
  [
    'accent.wash [data-scope="steps"][data-part="item"][data-here] [data-part="press"]',
    { own: "the step you are on; its mark and name are its own" },
  ],
  [
    'accent.wash [data-part="workspace"][data-scope="titlebar"][data-selected]',
    { own: "the chosen workspace chip; its count is laid on the bar's own ground (PART_LAID_ON)" },
  ],
  [
    'accent.wash-hover [data-part="workspace"][data-scope="titlebar"][data-selected][data-hovered]',
    { own: "the same, under the pointer" },
  ],
  [
    'wash.warning [data-scope="secret-ask"][data-part="control"]',
    { own: "a control character's name in a chat's words (#5080); its words are its own" },
  ],
  [
    'wash.warning [data-part="count"][data-scope="titlebar"]',
    { own: "a chip's waiting count, on the bar's own ground (PART_LAID_ON)" },
  ],
  [
    'wash.warning [data-part="row-count"][data-scope="titlebar"]',
    { own: "a switcher row's waiting count" },
  ],
  [
    'wash.warning [data-scope="badge"][data-part="root"][data-variant="waiting"]',
    { own: "a waiting badge; its words are its own" },
  ],
  [
    'wash.warning [data-scope="count"][data-part="root"][data-variant="waiting"]',
    { own: "a waiting count; its number is its own" },
  ],
  [
    'wash.warning [data-scope="status-item"][data-part="root"][data-variant="waiting"]',
    { own: "the title bar's waiting pill (NeedsYou.tsx); its hand and count are its own" },
  ],
  // The editor's washes (`editor/look.ts`), in CodeMirror's own elements: a changed line's on
  // the editor's ground, a changed word's on its line's, and a selection's on the editor's.
  ["diff.deleted .cm-editor.cm-merge-a .cm-changedLine", { editor: ["terminal.background"] }],
  ["diff.deleted .cm-deletedChunk", { editor: ["terminal.background"] }],
  ["diff.inserted .cm-editor.cm-merge-b .cm-changedLine", { editor: ["terminal.background"] }],
  ["diff.inserted .cm-inlineChangedLine", { editor: ["terminal.background"] }],
  [
    "diff.deleted-text .cm-editor.cm-merge-a .cm-changedText",
    { editor: ["diff.deleted", "terminal.background"] },
  ],
  [
    "diff.deleted-text .cm-editor .cm-deletedChunk .cm-deletedText",
    { editor: ["diff.deleted", "terminal.background"] },
  ],
  [
    "diff.deleted-text .cm-editor.cm-merge-b .cm-deletedText",
    { editor: ["diff.deleted", "terminal.background"] },
  ],
  [
    "diff.inserted-text .cm-editor.cm-merge-b .cm-changedText",
    { editor: ["diff.inserted", "terminal.background"] },
  ],
  [
    "terminal.selection .cm-editor.cm-focused .cm-selectionBackground",
    { editor: ["terminal.background"] },
  ],
  ["terminal.selection .cm-selectionBackground", { editor: ["terminal.background"] }],
  [
    'overlay.scrim [data-scope="dialog"][data-part="scrim"]',
    { own: "the dim behind a dialog; what it holds is the dialog's opaque ground" },
  ],
]);

describe("where each see-through ground is laid", () => {
  const SEE_THROUGH = shippedSeeThrough();
  /** The window's stylesheets and the editor's look, which CodeMirror writes into the page. */
  const LOOKS = [...sheets(), ...editorLooks()];
  const window = readGrounds(sources(SRC), LOOKS, SEE_THROUGH, new Set(), anchorsOf(PAINTED));
  const read = laidGrounds(window.laid, PAINTED);

  it("is what the window draws: purlis ships its washes see-through", () => {
    for (const token of ["danger.wash", "wash.warning", "accent.wash", "overlay.scrim"])
      expect(SEE_THROUGH).toContain(token);
    expect(SEE_THROUGH).not.toContain("surface.overlay");
  });

  it("is read for every rule that paints one, or the rule is listed with what it is", () => {
    expect(read.unlisted).toEqual([]);
    expect(read.stale).toEqual([]);
  });

  it("is, in the window, exactly what WASH_LAID_ON says", () => {
    const table = engineWashes();
    expect(table.get("danger.wash"), "WASH_LAID_ON in parts.rs").toBeDefined();
    expect(unsaid(read.laid, table)).toEqual([]);
  });

  /** A component that draws `inside` on a raised ground, and the stylesheet that paints it. */
  const raised = (inside: string) => ({
    files: [
      {
        name: "Raised.tsx",
        text: `export function Raised() { return <div className="raised">${inside}</div>; }`,
      },
    ],
    css: [
      ".raised { background: var(--surface-raised); }",
      ".box { background: var(--danger-wash); }",
    ],
  });

  it("reads a wash over the ground its component draws under it", () => {
    const { files, css } = raised('<p className="box">Refused.</p>');
    const fixture = readGrounds(files, css, SEE_THROUGH);
    const found = laidGrounds(fixture.laid, new Map());
    expect(found.unlisted).toEqual([]);
    expect(found.laid.get("danger.wash")).toEqual(["surface.raised"]);
  });

  it("fails a danger-wash box over surface.raised until WASH_LAID_ON names it", () => {
    // The window is read once, above; the box is read on its own and added to it, without
    // reading the whole window a second time (#4991: that read timed out on CI). This is the
    // window with the box in it while the two never meet: nothing in the window carries `raised`
    // or `box` or draws `Raised`, no window rule paints a bare `div` or `p`, and every window
    // rule that paints a wash names a class or an attribute the box's frames do not carry.
    const { files, css } = raised('<p className="box">Refused.</p>');
    const box = readGrounds(files, css, SEE_THROUGH);
    const laid = laidGrounds(new Map([...window.laid, ...box.laid]), PAINTED).laid;
    expect(unsaid(laid, engineWashes())).toContain(
      "danger.wash over surface.raised: not in WASH_LAID_ON",
    );
    const named = new Map(engineWashes());
    named.set("danger.wash", [...(named.get("danger.wash") ?? []), "surface.raised"]);
    expect(unsaid(laid, named)).toEqual([]);
  });

  /** **Each way a wash can be painted** (#4876), as a box over a raised ground: the markup, and
   *  the rule that paints it. The window's own sheets are read with them, so the alias is the
   *  one Tailwind's `@theme` sets (`styles.css`). */
  const FORMS: [string, string, string[]][] = [
    ["a Tailwind utility", '<p className="bg-danger-wash">Refused.</p>', []],
    ["a Tailwind arbitrary value", '<p className="bg-[var(--danger-wash)]">Refused.</p>', []],
    [
      "an alias",
      '<p className="box">Refused.</p>',
      [".box { background: var(--color-danger-wash); }"],
    ],
    [
      "a fallback",
      '<p className="box">Refused.</p>',
      [".box { background-color: var(--danger-wash, transparent); }"],
    ],
    [
      "a gradient",
      '<p className="box">Refused.</p>',
      [".box { background-image: linear-gradient(to bottom, var(--danger-wash), transparent); }"],
    ],
    [
      "a mix",
      '<p className="box">Refused.</p>',
      [".box { background: color-mix(in oklab, var(--danger-wash), transparent 10%); }"],
    ],
  ];

  it.each(FORMS)(
    "fails a danger-wash box painted by %s over surface.raised until WASH_LAID_ON names it",
    (_, inside, paint) => {
      // As above (#4991), the box is read on its own and added to the window read once; it is
      // read with the window's sheets, for the alias `@theme` sets and the utility it draws, and
      // only the rules it reaches are added, so the window's own rules keep the window's reading.
      const { files, css } = raised(inside);
      const box = readGrounds(files, [...LOOKS, css[0], ...paint], SEE_THROUGH);
      const reached = [...box.laid].filter(([, found]) => found !== undefined);
      const laid = laidGrounds(new Map([...window.laid, ...reached]), PAINTED).laid;
      expect(unsaid(laid, engineWashes())).toContain(
        "danger.wash over surface.raised: not in WASH_LAID_ON",
      );
      const named = new Map(engineWashes());
      named.set("danger.wash", [...(named.get("danger.wash") ?? []), "surface.raised"]);
      expect(unsaid(laid, named)).toEqual([]);
    },
  );

  /** A pane whose frame carries `frame`, its top holding a note another component draws, and the
   *  rule that paints the note by the top it is placed in. */
  const pane = (frame: string) => ({
    files: [
      {
        name: "Note.tsx",
        text: 'export function Note({ children }) { return <div data-scope="note">{children}</div>; }',
      },
      {
        name: "Pane.tsx",
        text: `export function Pane() { return <div className="${frame}"><div className="top"><Note>Stopped.</Note></div></div>; }`,
      },
    ],
    css: [
      ".frame.headed { background: var(--terminal-background); }",
      '.top [data-scope="note"] { background: var(--danger-wash); }',
    ],
  });
  const noted = new Map<string, Painted>([
    ['danger.wash .top [data-scope="note"]', { on: ["terminal.background"], at: ".top" }],
  ]);

  it("holds a listed ground to where the window draws the element the rule is scoped by", () => {
    const { files, css } = pane("frame headed");
    const fixture = readGrounds(files, css, SEE_THROUGH, new Set(), anchorsOf(noted));
    expect(laidGrounds(fixture.laid, noted).unlisted).toEqual([]);
    expect(misplaced(noted, fixture.anchored)).toEqual([]);
  });

  it("fails a listed ground once that element is drawn somewhere else", () => {
    // Read whole: `.frame.headed` paints the frame that carries both classes, never another.
    const { files, css } = pane("frame viewed");
    const fixture = readGrounds(files, css, SEE_THROUGH, new Set(), anchorsOf(noted));
    expect(misplaced(noted, fixture.anchored)).toEqual([
      'danger.wash .top [data-scope="note"]: listed on terminal.background, but .top is drawn on nothing the reader finds',
    ]);
    const onRaised = readGrounds(
      [
        files[0],
        {
          name: "Pane.tsx",
          text: 'export function Pane() { return <div className="raised"><div className="top" /></div>; }',
        },
      ],
      [...css, ".raised { background: var(--surface-raised); }"],
      SEE_THROUGH,
      new Set(),
      anchorsOf(noted),
    );
    expect(misplaced(noted, onRaised.anchored)).toEqual([
      'danger.wash .top [data-scope="note"]: listed on terminal.background, but .top is drawn on surface.raised',
    ]);
    // An element the window never draws holds nothing up.
    const nowhere = readGrounds([files[0]], css, SEE_THROUGH, new Set(), anchorsOf(noted));
    expect(misplaced(noted, nowhere.anchored)).toEqual([
      'danger.wash .top [data-scope="note"]: listed on terminal.background, but no component draws .top',
    ]);
  });

  it("holds each listed ground to where the window draws the element the rule is scoped by", () => {
    expect(misplaced(PAINTED, window.anchored)).toEqual([]);
  });

  it("fails a pane's top moved into a view's pane", () => {
    // `.pane-top` and the frame it sits in are drawn by one component, so `LayoutPanes.tsx` is
    // read on its own rather than the whole window again (#4991: a second read timed out on CI).
    // Unmoved, that file holds the top where `PAINTED` lists it, so the read is enough to tell.
    const panes = sources(SRC).find((one) => one.name === "LayoutPanes.tsx");
    if (panes === undefined) throw new Error("LayoutPanes.tsx is not in the window");
    const topOf = (text: string) =>
      misplaced(
        PAINTED,
        readGrounds([{ ...panes, text }], LOOKS, SEE_THROUGH, new Set(), anchorsOf(PAINTED))
          .anchored,
      ).filter((line) => line.includes(" .pane-top is drawn on"));
    expect(topOf(panes.text)).toEqual([]);
    const moved = panes.text.replace(/"pane-frame pane-headed"/g, '"pane-frame pane-viewed"');
    expect(moved).not.toEqual(panes.text);
    expect(topOf(moved).join("\n")).toContain(
      "listed on terminal.background, but .pane-top is drawn on",
    );
  });

  it("reads the washes the editor paints in JS (editor/look.ts) as a stylesheet", () => {
    const names = [...window.laid.keys()].map(named);
    expect(names).toContain("diff.inserted .cm-editor.cm-merge-b .cm-changedLine");
    expect(names).toContain("diff.deleted-text .cm-editor.cm-merge-a .cm-changedText");
  });

  it("holds each wash the editor paints to the grounds the core's pairs read it over", () => {
    expect(unpaired(PAINTED, corePairs(), editorGround())).toEqual([]);
    // A stack the core reads nothing over is named.
    expect(
      unpaired(
        new Map<string, Painted>([["diff.inserted .x", { editor: ["surface.raised"] }]]),
        corePairs(),
        editorGround(),
      ),
    ).toEqual([
      "diff.inserted .x: check.rs reads nothing on diff.inserted over surface.raised",
      "diff.inserted .x: laid on surface.raised, but the editor's ground is terminal.background",
    ]);
  });

  it("takes a ground a rule paints on an element by its attributes, at rest, as covering", () => {
    const fixture = readGrounds(
      [
        {
          name: "Card.tsx",
          text: 'export function Card() { return <div className="raised"><div data-scope="card" data-part="root"><p className="box">Refused.</p></div></div>; }',
        },
      ],
      [
        ...raised("").css,
        '[data-scope="card"][data-part="root"] { background: var(--surface-overlay); }',
      ],
      SEE_THROUGH,
    );
    expect(laidGrounds(fixture.laid, new Map()).laid.get("danger.wash")).toEqual([
      "surface.overlay",
    ]);
  });

  it("takes a gradient as a ground a wash may be on, never a cover", () => {
    const fixture = readGrounds(
      [
        {
          name: "Tree.tsx",
          text: 'export function Tree() { return <div className="raised"><div className="lined"><p className="box">Refused.</p></div></div>; }',
        },
      ],
      [
        ...raised("").css,
        ".lined { background-image: linear-gradient(var(--tree-guide) 0 1px, transparent 1px); }",
      ],
      SEE_THROUGH,
    );
    expect(laidGrounds(fixture.laid, new Map()).laid.get("danger.wash")).toEqual([
      "surface.raised",
      "tree.guide",
    ]);
  });

  it("reads an attribute written as an expression as a ground it may be on, never a cover", () => {
    const fixture = readGrounds(
      [
        {
          name: "Card.tsx",
          text: 'export function Card({ part }: { part: string }) { return <div className="raised"><div data-scope="card" data-part={part}><p className="box">Refused.</p></div></div>; }',
        },
      ],
      [
        ...raised("").css,
        '[data-scope="card"][data-part="root"] { background: var(--surface-overlay); }',
      ],
      SEE_THROUGH,
    );
    expect(laidGrounds(fixture.laid, new Map()).laid.get("danger.wash")).toEqual([
      "surface.overlay",
      "surface.raised",
    ]);
  });

  it("asks for a rule whose element its own component lays on nothing", () => {
    const fixture = readGrounds(
      [
        {
          name: "Box.tsx",
          text: 'export function Box() { return <p className="box">Refused.</p>; }',
        },
      ],
      raised("").css,
      SEE_THROUGH,
    );
    const found = laidGrounds(fixture.laid, new Map());
    expect(found.unlisted).toEqual(["danger.wash .box: drawn on nothing the reader finds"]);
    expect(
      laidGrounds(
        fixture.laid,
        new Map<string, Painted>([["danger.wash .box", { on: ["surface.overlay"], at: DIALOG }]]),
      ).laid.get("danger.wash"),
    ).toEqual(["surface.overlay"]);
  });

  it("asks for a rule it finds no element of, and lets a part's own fill be", () => {
    const fixture = readGrounds([], raised("").css, SEE_THROUGH);
    expect(laidGrounds(fixture.laid, new Map()).unlisted).toEqual([
      "danger.wash .box: drawn on nothing the reader finds",
    ]);
    expect(
      laidGrounds(fixture.laid, new Map([["danger.wash .box", { own: "why" }]])).unlisted,
    ).toEqual([]);
  });

  it("fails a listed rule it reads, and a listed rule no sheet has", () => {
    const { files, css } = raised('<p className="box">Refused.</p>');
    const fixture = readGrounds(files, css, SEE_THROUGH);
    expect(
      laidGrounds(
        fixture.laid,
        new Map([
          ["danger.wash .box", { on: ["surface.overlay"], at: DIALOG }],
          ["danger.wash .gone", { on: ["surface.base"], at: DIALOG }],
        ]),
      ).stale,
    ).toEqual(["danger.wash .box: the reader finds it on surface.raised", "danger.wash .gone"]);
  });
});
