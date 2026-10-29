// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  classNameOf,
  classWords,
  isView,
  openingTags,
  ratchet,
  SRC,
  sourceFiles,
  withoutComments,
} from "./guards.testkit";

/**
 * **No colour in `App.css` lands on a word that wears a text role** (#3917).
 *
 * `App.css` is imported into the `charter` layer and the text roles (`primitives/text.css`) into
 * `components`, a later layer, which wins at any specificity (`styles.css`). So a `color` an
 * `App.css` rule sets on an element that also wears `ui-text-title`, `-heading`, `-body`,
 * `-secondary`, `-meta` or `-label` is never drawn: #3859 found the Explorer's repo that is not
 * cloned drawn in a cloned repo's primary, and five more rules on main (#3917), among them the
 * Explorer's trouble lines, meant red and drawn secondary. A word that needs another colour
 * wears another role, or wears none and takes its size from the role it sits in, beside the tint
 * a state gives it.
 *
 * Read from the source, as the other guards are: each `App.css` rule that sets `color`, by the
 * last compound of each of its selectors (the element it lands on), against each element a
 * component draws with those classes and attributes and a role. What sits above the element in
 * the selector is not read, so a rule whose context never holds the element is allowed below,
 * each with why.
 */

/** The roles that set a colour; `ui-text-code` and `ui-text-strong` set none. */
const COLOURED = /^ui-text-(?:title|heading|body|secondary|meta|label)$/;

/** Each selector of each rule of `css` that sets `color`. */
function colourSelectors(css: string): string[] {
  const text = withoutComments(css, "css");
  return [...text.matchAll(/([^{};]+)\{([^{}]*)\}/g)].flatMap((hit) =>
    /(?<![\w-])color\s*:/.test(hit[2] ?? "") ? splitTop(hit[1] ?? "", ",") : [],
  );
}

/** `text` split at `at` where no bracket or parenthesis is open. */
function splitTop(text: string, at: RegExp | string): string[] {
  const out: string[] = [];
  let depth = 0;
  let from = 0;
  const is = (char: string) => (typeof at === "string" ? char === at : at.test(char));
  for (let i = 0; i < text.length; i++) {
    const char = text[i] ?? "";
    if (char === "(" || char === "[") depth++;
    else if (char === ")" || char === "]") depth--;
    else if (depth === 0 && is(char)) {
      out.push(text.slice(from, i));
      from = i + 1;
    }
  }
  out.push(text.slice(from));
  return out.map((one) => one.trim()).filter(Boolean);
}

/** What the last compound of `selector` asks of the element it lands on: its classes and the
 *  attributes it names. `undefined` for a pseudo-element, which draws something else. */
function landsOn(
  selector: string,
): { tag: string | undefined; classes: string[]; attributes: string[] } | undefined {
  if (selector.includes("::")) return undefined;
  const last = splitTop(selector, /[\s>+~]/).at(-1) ?? "";
  // A functional pseudo-class says what the element is or is not; it adds no class it must wear.
  const plain = last.replace(/:[\w-]+\((?:[^()]|\([^()]*\))*\)/g, "");
  return {
    tag: /^[a-z][\w-]*/.exec(plain)?.[0],
    classes: [...plain.matchAll(/\.([\w-]+)/g)].map((one) => one[1] ?? ""),
    attributes: [...plain.matchAll(/\[([\w-]+)/g)].map((one) => one[1] ?? ""),
  };
}

/** Each element a component draws with a coloured role that a colour rule of `css` lands on, by
 *  file: `<selector> on "<the element's className as written>"`. No line numbers, so an edit
 *  above either one moves no entry. */
function layeredOut(css: string, read: (file: string) => string): Map<string, string[]> {
  const rules = colourSelectors(css).flatMap((selector) => {
    const on = landsOn(selector);
    return on && on.classes.length > 0 ? [{ selector, ...on }] : [];
  });
  const found = new Map<string, string[]>();
  for (const file of sourceFiles(isView)) {
    const source = read(file);
    for (const tag of openingTags(source, /^[a-z]/)) {
      const words = new Set(classWords(classNameOf(tag.text)));
      if (![...words].some((word) => COLOURED.test(word))) continue;
      for (const rule of rules) {
        if (rule.tag !== undefined && rule.tag !== tag.name) continue;
        if (!rule.classes.every((one) => words.has(one))) continue;
        if (!rule.attributes.every((one) => new RegExp(`\\s${one}[=\\s/>]`).test(tag.text)))
          continue;
        const entry = `${rule.selector} on "${classNameOf(tag.text) ?? ""}"`;
        found.set(file, [...(found.get(file) ?? []), entry]);
      }
    }
  }
  return found;
}

const read = (file: string) => readFileSync(join(SRC, file), "utf8");

/**
 * Rules whose context never holds the element named, each with why. An entry names the rule
 * and the element as written, so a change to either is a new entry to look at again.
 */
const ALLOWED: Readonly<Record<string, readonly string[]>> = {
  // A sentence wears `ui-text-secondary` and a count `ui-text-meta`, on purpose (#1920): the views' and dialogs' muted `.none` and `.pending` are for sentences that wear no role.
  "Explorer.tsx": [
    '.view-body .none on "none ui-text-secondary"',
    '.view-body .none on "none ui-text-secondary"',
    '[data-scope="dialog"] .pending on "pending ui-text-secondary"',
    '.view-body .pending on "pending ui-text-secondary"',
    '.view-body .none on "piece-count none ui-text-meta"',
    '[data-scope="dialog"] .pending on "piece-count pending ui-text-meta"',
    '.view-body .pending on "piece-count pending ui-text-meta"',
  ],
  // An ask's row is never an `.inbox-update`, and a read update wears no role: only an unread one does (`UpdateItem`).
  "Inbox.tsx": [
    '.inbox-update[data-read] .ask-says on "ask-says ask-lines inbox-primary ui-text-body ui-text-strong"',
    '.inbox-update[data-read] .ask-says on "`ask-says inbox-primary ui-text-body ui-text-strong${whole ? "" : " inbox-clamped"}`"',
    '.inbox-update[data-read] .ask-says on "`ask-says inbox-primary${update.read ? "" : " ui-text-body ui-text-strong"}${answers.some((one) => one.allows) ? "" : " inbox-clamped"}`"',
  ],
  // These are asks, never an `.inbox-update`.
  "InboxElsewhere.tsx": [
    '.inbox-update[data-read] .ask-says on "ask-says inbox-primary ui-text-body ui-text-strong"',
    '.inbox-update[data-read] .ask-says on "ask-says inbox-primary ui-text-body ui-text-strong inbox-clamped"',
    '.inbox-update[data-read] .ask-says on "ask-says inbox-primary ui-text-body ui-text-strong inbox-clamped"',
  ],
  // The same: a clone under way says so in `ui-text-secondary` (#1920).
  "NotCloned.tsx": [
    '[data-scope="dialog"] .pending on "pending ui-text-secondary"',
    '.view-body .pending on "pending ui-text-secondary"',
  ],
  // The same: an empty filter's sentence wears `ui-text-secondary` (#1920).
  "PanelList.tsx": ['.view-body .none on "none ui-text-secondary"'],
  // A pending sentence wears `ui-text-secondary` (#1920); a note block is `trouble` or `note ui-text-secondary`, never both, so no trouble colour meets the role.
  "Panels.tsx": [
    '[data-scope="dialog"] .pending on "pending ui-text-secondary"',
    '.view-body .pending on "pending ui-text-secondary"',
    '.trouble on "block.tone === "trouble" ? "trouble" : "note ui-text-secondary""',
    '.explorer .trouble on "block.tone === "trouble" ? "trouble" : "note ui-text-secondary""',
    '.state-bar .trouble on "block.tone === "trouble" ? "trouble" : "note ui-text-secondary""',
    '.panels .trouble on "block.tone === "trouble" ? "trouble" : "note ui-text-secondary""',
  ],
};

/** How long each file's list may be: it only gets shorter. */
const CEILING: Readonly<Record<string, number>> = {
  "Explorer.tsx": 7,
  "Inbox.tsx": 3,
  "InboxElsewhere.tsx": 3,
  "NotCloned.tsx": 2,
  "PanelList.tsx": 1,
  "Panels.tsx": 6,
};

describe("no App.css colour lands on a word that wears a text role (#3917)", () => {
  it("finds none in the window's components", () => {
    expect(
      ratchet(layeredOut(read("App.css"), read), ALLOWED, CEILING, {
        added:
          "an App.css colour on a word that wears a role is never drawn: give the word another role, or none and the state's tint",
      }),
    ).toEqual([]);
  });

  it("finds a colour rule that lands on a word with a role, in another file than the role", () => {
    const files = sourceFiles(isView);
    const css = ".lone-word { color: var(--state-failed); }\n.lone-word .inner { color: red; }";
    const found = layeredOut(css, (file) =>
      file === files[0] ? '<p className="lone-word ui-text-secondary">x</p>' : "",
    );
    expect(found).toEqual(new Map([[files[0], ['.lone-word on "lone-word ui-text-secondary"']]]));
  });

  it("leaves a word with no role, a role that sets no colour, a pseudo-element and another tag alone", () => {
    const files = sourceFiles(isView);
    const found = layeredOut(
      ".a { color: var(--text-muted); }\n.b::before { color: var(--text-muted); }\nli.c { color: var(--text-muted); }",
      (file) =>
        file === files[0]
          ? '<p className="a">x</p><p className="a ui-text-code">y</p><p className="b ui-text-body">z</p><p className="c ui-text-body">w</p>'
          : "",
    );
    expect(found).toEqual(new Map());
  });

  it("asks for each attribute the rule's element names", () => {
    const files = sourceFiles(isView);
    const css = '.mark[data-driver="person"] { color: var(--text-primary); }';
    const drawn = (tag: string) => layeredOut(css, (file) => (file === files[0] ? tag : ""));
    expect(drawn('<span className="mark ui-text-meta">x</span>')).toEqual(new Map());
    expect(drawn('<span className="mark ui-text-meta" data-driver={driver}>x</span>').size).toBe(1);
  });
});
