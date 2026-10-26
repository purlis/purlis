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
 *   classes the rule is scoped by), or a Tailwind `bg-*` utility; the window itself (`:root`) is
 *   the ground under everything else, and what is portaled to the body;
 * - **a button** is an element of the family: one with the `ui-button` class, or the family's
 *   `data-scope="button"` wrapper;
 * - every component is read: a button is on the grounds of the elements around it, up to the
 *   first one that has a background at rest, through the components that render it, the
 *   `children` they place, and the JSX a component keeps in a variable. A class a component may
 *   or may not carry is a ground it may be on, and never hides the one under it.
 *
 * The list of grounds found must be the manifest's list, exactly, so a new surface a button is
 * placed on fails here until the manifest names it, and the parts check measures it.
 */

/// <reference types="node" />
import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import ts from "typescript";
import { describe, expect, it } from "vitest";
import { property, TOKENS, type Token } from "./theme";

const SRC = join(process.cwd(), "src");
const PARTS = join(process.cwd(), "..", "crates", "purlis-core", "src", "theme", "parts.rs");

/** Each family's grounds as the manifest lists them, read out of the core's source: written out
 *  (`grounds: &[…]`), or named by a shared list (`grounds: CONTROL_GROUNDS`, `const … = &[…];`). */
function manifestGrounds(source: string): Map<string, string[]> {
  const tokens = (list: string) => [...list.matchAll(/"([^"]+)"/g)].map(([, token]) => token);
  const shared = new Map<string, string[]>();
  for (const [, name, list] of source.matchAll(/\nconst ([A-Z_]+): &\[&str\] = &\[([^\]]*)\];/g))
    shared.set(name, tokens(list));
  const grounds = new Map<string, string[]>();
  for (const [, name, list, named] of source.matchAll(
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
  for (const sheet of css)
    for (const [, selectors, body] of sheet.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      const backs = [
        ...body.matchAll(/(?<![\w-])background(?:-color)?\s*:\s*var\((--[\w-]+)\)/g),
      ].flatMap(([, name]) => {
        const token = BY_PROPERTY.get(name);
        return token === undefined ? [] : [token];
      });
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
        // Nor is the family's own stylesheet, on its wrapper's attributes.
        if (/\[data-scope="button"\]/.test(subject)) continue;
        // Of the dialog family's parts (#2910), only its root is a ground a button stands on: the
        // scrim is drawn under the root, and the footer's rule holds nothing.
        if (/\[data-scope="dialog"\]/.test(subject) && !/\[data-part="root"\]/.test(subject))
          continue;
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
 *  state. */
function utility(name: string): Ground | undefined {
  const hit = /^((?:[\w-]+:)*)bg-([\w-]+)$/.exec(name);
  const token = hit === null ? undefined : BY_DASHED.get(hit[2]);
  if (hit === null || token === undefined) return undefined;
  return { tokens: new Set([token]), rest: hit[1] === "" };
}

// --- The components: where each button is drawn -----------------------------------------------

/** A class an element carries, and whether it carries it always or only on a condition. */
type Carried = { name: string; always: boolean };

/** One element around a button: the grounds it may draw, and whether it draws one at rest
 *  always, which hides what is under it. */
type Frame = { tokens: Set<Token>; covers: boolean; classes: Set<string> };

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

  constructor(
    private readonly files: { name: string; text: string }[],
    private readonly byClass: Map<string, Ground>,
  ) {
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
    const fresh = new Map<string, Unit[]>();
    for (const file of this.files) {
      const source = ts.createSourceFile(
        file.name,
        file.text,
        ts.ScriptTarget.Latest,
        true,
        ts.ScriptKind.TSX,
      );
      this.readFile(source, fresh);
    }
    this.units.clear();
    fresh.forEach((units, name) => this.units.set(name, units));
  }

  private readFile(source: ts.SourceFile, into: Map<string, Unit[]>): void {
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
    /** Each variable whose value is JSX, by name, read where it is placed; and every other
     *  variable's value, which a `className` may name. */
    const held = new Map<string, ts.Node>();
    const classLists = new Map<string, ts.Node[]>();
    const visitHeld = (node: ts.Node) => {
      if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer) {
        const name = node.name.text;
        if (hasJsx(node.initializer)) held.set(name, node.initializer);
        else classLists.set(name, [...(classLists.get(name) ?? []), node.initializer]);
      }
      ts.forEachChild(node, visitHeld);
    };
    visitHeld(source);
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
        if (body) this.walk(body, [], unit, source, held, classLists, new Set());
        return;
      }
      if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && held.has(node.name.text))
        return;
      ts.forEachChild(node, top);
    };
    top(source);
  }

  private classesOf(expression: ts.Node, classLists: Map<string, ts.Node[]>): Carried[] {
    const carried: Carried[] = [];
    const seen = new Set<string>();
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
      if (ts.isIdentifier(node) && classLists.has(node.text) && !seen.has(node.text)) {
        seen.add(node.text);
        for (const one of classLists.get(node.text) ?? []) visit(one, always);
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
    held: Map<string, ts.Node>,
    classLists: Map<string, ts.Node[]>,
    placing: Set<string>,
  ): void {
    const again = (child: ts.Node, inside: Frame[]) =>
      this.walk(child, inside, unit, source, held, classLists, placing);
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
      const carried = className === undefined ? [] : this.classesOf(className, classLists);
      const scope = attribute("data-scope")?.initializer;
      const part = attribute("data-part")?.initializer;
      const named = tag.split(".").pop() ?? tag;
      const isButton =
        carried.some((one) => one.name === "ui-button") ||
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
        };
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
          const ground = this.byClass.get(one.name) ?? utility(one.name);
          if (ground === undefined || this.buttonClasses.has(one.name)) continue;
          ground.tokens.forEach((token) => frame.tokens.add(token));
          if (ground.rest && one.always) frame.covers = true;
        }
        if (frame.tokens.size > 0 || frame.classes.size > 0) inside = [...frames, frame];
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
      const value = held.get(node.text);
      if (value !== undefined && !placing.has(node.text)) {
        placing.add(node.text);
        again(value, frames);
        placing.delete(node.text);
      }
      return;
    }
    if (ts.isPropertyAccessExpression(node) && node.name.text === "children" && placed(node)) {
      unit.childrenAt.push(frames);
      return;
    }
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression))
      unit.uses.push({ name: node.expression.text, frames });
    if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && held.has(node.name.text))
      return;
    ts.forEachChild(node, (child) => again(child, frames));
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
): { found: Map<Token, string[]>; unread: Unread; inline: string[] } {
  const { byClass, window, unread } = groundsOfClasses(css);
  // What a helper's spread attributes or the theme engine's layout words draw, by class.
  for (const [name, tokens] of ENGINE_GROUNDS) {
    const ground = byClass.get(name) ?? { tokens: new Set<Token>(), rest: false };
    tokens.forEach((token) => ground.tokens.add(token));
    byClass.set(name, ground);
  }
  const reader = new Reader(files, byClass);
  // The first pass finds the buttons' classes and each component's `children` places, the next
  // ones read what is placed in them; a third settles a component placed in another's children.
  for (let pass = 0; pass < 3; pass += 1) reader.read();
  // A rule on an attribute no element carries as written is one the reader cannot tie to one:
  // the window sets it as it runs, or spells it in an expression.
  for (const [form, selectors] of unread.byAttribute)
    if (!reader.reached.has(form)) unread.classless.push(...selectors);
  return { found: reader.grounds(window), unread, inline: [...reader.inline].sort() };
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
 * written out, and `tabsPart`, `panelRoot` and `editorToolbar` spread theirs. The chat strip's
 * shade is the engine's too, under `tabs.layout.attach: strip` (`attached` in
 * `crates/purlis-core/src/theme/parts.rs`).
 */
const ENGINE_GROUNDS: ReadonlyMap<string, readonly Token[]> = new Map([
  // The chat strip (`tabsPart("strip")`): none docked, the chat strip's shade on its own.
  ["bar", ["layer.chat"]],
  // The content panel the tabs attach to (`panelRoot("content")`), Purlis's panel fill.
  ["panes", ["surface.raised"]],
  // The editor's toolbar over a file (`editorToolbar`): the code's own surface.
  ["piece-files-head", ["terminal.background"]],
]);

const CLASSLESS: readonly string[] = [
  // The band a chat row takes for a moment once it is revealed (`revealTask.ts` sets the
  // attribute as it runs); the row is a `.ui-row`, which holds no button.
  ".chats-section [data-revealed]",
  // A selected or current table row's cells: the one table whose rows are current (a wanted
  // skill's row, `SkillTab.tsx`, set by an expression) holds no button.
  '.ui-table tbody tr:is([aria-selected="true"], [aria-current]:not([aria-current="false"])) > *',
  // Selected words, not a box anything stands on.
  ':root[data-theme-draws~="selection.background"] :not(.cm-editor, .cm-editor *)::selection',
  // The handle between two panes, which the panel library draws, and which holds nothing.
  "[data-separator]",
  "[data-separator]:hover",
  "[data-separator][data-dragging]",
  // A region panel's ground (`ui/panel`, #2913), set by the family's attributes on a library's
  // panel: `surface.base`, the window's own, which the button family already lists.
  '[data-scope="panel"][data-part="root"]',
  // The editor's toolbar (`ui/tabs`, #2914): its attributes are spread by `editorToolbar`, and its
  // ground is listed by class in ENGINE_GROUNDS.
  '[data-scope="editor"][data-part="toolbar"]',
  // A session tab's `×` under the pointer (`ui/tabs/tabs.css`, #2914): its attributes are the
  // family's (`tabPart`), and it holds no button.
  '[data-scope="tabs"][data-part="tab"] [data-part="closer"][data-scope="tabs"]:hover',
  '[data-scope="tabs"][data-part="tab"] [data-part="closer"][data-scope="tabs"][data-keeps]:hover',
  // A card's ground (`ui/card`, #2912), set by the family's attributes on the element a screen
  // asks for: `surface.raised`, which the button family lists, and in Purlis `surface.base`
  // (its `card.root.fill`), which the engine holds to the button's grounds.
  '[data-scope="card"][data-part="root"]',
];

/**
 * The background rules that reach an element by what it holds (`:has(…)`). Each is read on the
 * element it paints, whatever it holds, so it counts as a ground there and never hides the one
 * under it; a class inside the `:has(…)` is another element's and is not read as a ground. Each
 * is listed so a new one is looked at.
 */
const WITHIN: readonly string[] = [
  ".chats-section li:has(> .ui-row[data-revealed])",
  // A pane's warning by its tone (`paneFrame.css`, #3142): a consent (it holds an Allow) on the
  // warning wash, trouble on the danger wash. Each is a translucent wash over the terminal's
  // fill, which the button family lists.
  '.pane-top\n  :is(.notice-pane-box, [data-scope="notice"][data-placement="pane"]):has([data-variant="grants"])',
  '.pane-top\n  :is(.notice-pane-box:has(> .notice-trouble), [data-scope="notice"][data-variant="trouble"])',
  // And each one's mark, a pseudo-element no button is drawn on.
  '.pane-top\n  :is(\n    .notice-pane-box:has(> .notice-trouble),\n    [data-scope="notice"][data-variant="trouble"]\n  )::before',
  '.pane-top\n  :is(.notice-pane-box, [data-scope="notice"][data-placement="pane"]):has(\n    [data-variant="grants"]\n  )::before',
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
             return <div className="raised"><div className="row"><button className="ui-button" /></div></div>;
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
          `export function Fix() { return <button className="ui-button">Fix</button>; }`,
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
             return <div className="raised"><div className={classes}><button className={\`ui-button \${alone ? "ui-button-sm" : ""}\`} /></div></div>;
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
             return <><table className="secrets"><tr><td><button className="ui-button" /></td></tr></table>
               <table className="other"><tr><td><button className="ui-button" /></td></tr></table>
               <div role="tab"><button className="ui-button" /></div></>;
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
          `export function H() { return <li className="row"><button className="ui-button" /></li>; }`,
        ),
      ],
      css(`li:has(> .row) { background: var(--surface-raised); }`),
    );
    expect(found).toEqual(["surface.base", "surface.raised"]);
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
          `export function B() { return <div className="bg-surface-sunken"><button className="ui-button fix" /></div>; }`,
        ),
      ],
      css(
        `.fix { background: var(--accent-surface); } .ui-button { background: var(--control-base); }`,
      ),
    );
    expect(found).toEqual(["surface.base", "surface.sunken"]);
  });

  /** The window, read once for the tests below. */
  const read = readGrounds(sources(SRC), sheets());

  it("are, in the window, exactly the grounds the button family's manifest lists", () => {
    const listed = manifestGrounds(readFileSync(PARTS, "utf8")).get("button");
    expect(listed, "the button family's grounds in parts.rs").toBeDefined();
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
