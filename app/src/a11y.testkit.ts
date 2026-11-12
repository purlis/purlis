// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * Test-only: **every control has a name** (DS-6, #629).
 *
 * A screen reader says a control by its role and its accessible name: "Reset, button". A
 * control without a name is said as "button", and nobody who cannot see the icon on it knows
 * what it does. So every control a surface draws is held to a non-empty accessible name, the
 * name `@testing-library`'s role queries compute (the accessible-name algorithm the platform
 * uses: `aria-labelledby`, `aria-label`, a `<label>`, the text inside, `title`).
 *
 * **Without axe** (D-629-1). axe-core would be a new dependency, which is the operator's call
 * (#629's acceptance line stays open for it). This is the rule axe's `button-name`, `link-name`,
 * `label` and `aria-input-field-name` hold, written with what the window's tests already have.
 *
 * Only what is in the accessibility tree is held: a control inside `aria-hidden` or `hidden`
 * is not said to anyone, so it has nothing to be named for.
 */
import { queryAllByRole, type ByRoleMatcher } from "@testing-library/react";
import { expect } from "vitest";

/** **An element's role as a screen reader hears it**: its own `role`, or a native input's by its
 *  type (a checkbox, a radio, a slider), or its tag. */
export function roleOf(element: Element): string {
  const own = element.getAttribute("role");
  if (own !== null) return own;
  if (element.tagName === "INPUT") {
    const type = (element as HTMLInputElement).type;
    if (type === "checkbox" || type === "radio") return type;
    if (type === "range") return "slider";
  }
  return element.tagName.toLowerCase();
}

/** **The words an element goes by**, as a reader of a list of stops reads them: its
 *  `aria-label`, the words of each element its `aria-labelledby` names, its `<label for>`, the
 *  label it is in, or its own words. */
export function wordsOf(element: Element): string {
  const label = element.getAttribute("aria-label");
  if (label !== null) return label;
  const by = element.getAttribute("aria-labelledby");
  if (by) {
    const words = by
      .split(/\s+/)
      .map((id) => {
        const one = document.getElementById(id);
        return one?.getAttribute("aria-label") ?? one?.textContent?.trim() ?? "";
      })
      .filter(Boolean)
      .join(" ");
    if (words !== "") return words;
  }
  const own = element.id ? document.querySelector(`label[for="${element.id}"]`) : null;
  if (own?.textContent) return own.textContent;
  if (element.tagName === "INPUT") return element.closest("label")?.textContent ?? "";
  return element.textContent ?? "";
}

/** Every role a person operates, as the window draws them. A `separator` counts only when a
 *  keyboard can reach it (see {@link namelessControls}). */
export const CONTROL_ROLES = [
  "button",
  "link",
  "textbox",
  "searchbox",
  "checkbox",
  "radio",
  "switch",
  "tab",
  "combobox",
  "listbox",
  "option",
  "slider",
  "spinbutton",
  "menuitem",
  "menuitemcheckbox",
  "menuitemradio",
  "separator",
] as const satisfies readonly ByRoleMatcher[];

/** How a control reads to whoever is looking for it: its role, and the words it goes by, as
 *  `button "Start"`. */
export function said(element: Element): string {
  return `${roleOf(element)} "${wordsOf(element).trim()}"`;
}

/** How a nameless control is written in a failure and in a debt list: its role, its element
 *  and the classes it is drawn with, which are what a person finds it by in the source. */
export function described(role: string, element: Element): string {
  const classes = (element.getAttribute("class") ?? "").trim().split(/\s+/).filter(Boolean);
  const tag = element.tagName.toLowerCase();
  return `${role} <${tag}${classes.length > 0 ? `.${classes.join(".")}` : ""}>`;
}

/**
 * Whether an element of `role` is one a person operates. Every role in {@link CONTROL_ROLES}
 * is, but a `separator`: only a separator the keyboard can reach is a control (a resize
 * handle, which its arrow keys move), and ARIA asks a name of that one alone. A plain divider
 * between a menu's rows is structure, said as nothing, and has nothing to be named for.
 */
function operated(role: (typeof CONTROL_ROLES)[number], element: Element): boolean {
  return role !== "separator" || element.hasAttribute("tabindex");
}

/** Each control under `root` whose accessible name is empty, described, in document order. */
export function namelessControls(root: HTMLElement): string[] {
  const nameless = new Map<Element, string>();
  for (const role of CONTROL_ROLES)
    for (const element of queryAllByRole(root, role, { name: (name) => name.trim() === "" }))
      if (operated(role, element) && !nameless.has(element))
        nameless.set(element, described(role, element));
  return [...nameless.entries()]
    .sort(([a], [b]) => (a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1))
    .map(([, said]) => said);
}

/**
 * Fails, naming each one, if a control under `root` has no accessible name. `debt` lists the
 * nameless controls a surface is known to draw in files this check cannot mend yet, each as
 * {@link described} writes it: they are let through, and a debt that is no longer drawn fails
 * too (see {@link paidDebts}), so the list only shrinks. Answers what it found, debts included.
 */
export function expectEveryControlNamed(
  root: HTMLElement,
  debt: readonly string[] = [],
  where = "",
): string[] {
  const nameless = namelessControls(root);
  const owed = new Set(debt);
  expect(
    nameless.filter((one) => !owed.has(one)),
    `nameless controls${where ? ` in ${where}` : ""}`,
  ).toEqual([]);
  return nameless;
}

/** The debts in `debt` that no surface drew: each is paid, and comes off the list. */
export function paidDebts(debt: readonly string[], drawn: Iterable<string>): string[] {
  const seen = new Set(drawn);
  return debt.filter((one) => !seen.has(one));
}

/** The classes that keep words off the screen but in a control's name: App.css's
 *  `.hidden-words` and Tailwind's `sr-only`. jsdom lays nothing out, so these are how a test
 *  tells the words shown from the words only heard. */
const UNSEEN = ".hidden-words, .sr-only";

/** A text's words, as a voice user says them: letters and digits, case and marks dropped. */
const wordsIn = (text: string) =>
  text
    .normalize("NFKD")
    .replace(/\p{M}/gu, "")
    .toLowerCase()
    .split(/[^\p{L}\p{N}]+/u)
    .filter((one) => one !== "");

/** **The words a control shows**: its text, less what is drawn for a screen reader alone. Words
 *  inside `aria-hidden` still count, as they are on screen (axe counts them too). */
export function shownWordsOf(element: Element): string {
  const copy = element.cloneNode(true) as Element;
  copy.querySelectorAll(UNSEEN).forEach((one) => one.remove());
  return copy.textContent ?? "";
}

/**
 * **Each control whose name does not hold the words it shows** (WCAG 2.5.3, Label in Name,
 * #6371): the words shown must come, in order and unbroken, among the words of its accessible
 * name (the one the role queries compute), so a voice user who says what they see reaches it.
 * axe's `label-content-name-mismatch` holds this only in a browser: in jsdom it sees no words on
 * screen and reports the rule incomplete, never a violation, and it looks only at a control named
 * by `aria-label` or `aria-labelledby`. This asks it of every control of `role` in `root`, however
 * it is named, and says each that fails as `button "its name" shows "its words"`.
 */
export function namedWithoutItsWords(root: HTMLElement, role: ByRoleMatcher = "button"): string[] {
  const named: [Element, string][] = [];
  queryAllByRole(root, role, {
    name: (name, element) => {
      named.push([element, name]);
      return true;
    },
  });
  return named
    .filter(([element, name]) => {
      const shown = wordsIn(shownWordsOf(element));
      const heard = wordsIn(name);
      return (
        shown.length > 0 && !heard.some((_, at) => shown.every((word, i) => heard[at + i] === word))
      );
    })
    .map(
      ([element, name]) => `${roleOf(element)} "${name}" shows "${shownWordsOf(element).trim()}"`,
    );
}
