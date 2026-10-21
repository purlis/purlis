// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { expect } from "vitest";
import { drawnWith } from "./cascade.testkit";
import { declarations } from "./guards.testkit";

/**
 * **What a word of the window is drawn with, as a text role** (#1920; `docs/design-system.md`
 * "Text roles"): its size in px at the default text size, its weight and its colour's token,
 * resolved through the window's own cascade (`cascade.testkit.ts`) and, where the word's own
 * element sets none, through what it sits in. So a rule of `App.css` that re-ranked a word, or
 * a root that stopped handing its role down, fails the test that reads the word.
 */

const core = new Map(
  declarations(readFileSync(join(process.cwd(), "src", "theme/geometry.css"), "utf8")).map(
    ({ property: name, value }) => [name, value],
  ),
);

/** What a `var(--x)` names, or the value itself when it is not one. */
const named = (value: string | undefined) => /^var\((--[\w-]+)\)$/.exec(value ?? "")?.[1] ?? value;

/** A size token in px at the window's default text size (14px). */
function px(token: string | undefined): number {
  const value = core.get(token ?? "") ?? "";
  const share = /^calc\(([\d.]+)rem \/ 14\)$/.exec(value);
  if (share) return Number(share[1]);
  const rem = /^([\d.]+)rem$/.exec(value);
  if (rem) return Number(rem[1]) * 14;
  throw new Error(`${token} is ${value}, not a core size`);
}

/** The property as the element draws it: its own rule's, or the nearest ancestor's that sets
 *  one, which is how the three type properties reach a word. */
export function inherited(element: Element, property: string): string | undefined {
  for (let at: Element | null = element; at !== null; at = at.parentElement) {
    const own = drawnWith(at, property);
    if (own !== undefined && own !== "inherit") return own;
  }
  return undefined;
}

/** The roles' table (`primitives/text.test.ts` holds the classes to it). */
export const ROLES = {
  body: { class: "ui-text-body", size: 13, weight: "400", colour: "--text-primary" },
  secondary: { class: "ui-text-secondary", size: 12, weight: "400", colour: "--text-secondary" },
  meta: { class: "ui-text-meta", size: 11, weight: "400", colour: "--text-quiet" },
  label: { class: "ui-text-label", size: 11, weight: "600", colour: "--text-secondary" },
} as const;

export type Role = keyof typeof ROLES;

/** The size in px, the weight and the colour's token a word is drawn with. */
export function typeOf(word: Element) {
  return {
    size: px(named(inherited(word, "font-size"))),
    weight: core.get(named(inherited(word, "font-weight")) ?? ""),
    colour: named(inherited(word, "color")),
  };
}

/** `word` is drawn as `role`: that role's size, weight and colour, whether it wears the class
 *  itself or takes it from what it sits in. `strong` for the role at the strong weight. */
export function drawnAs(
  word: Element | null | undefined,
  role: Role,
  what: string,
  strong = false,
) {
  expect(word, `${what} was not drawn`).toBeTruthy();
  const { size, weight, colour } = ROLES[role];
  expect(typeOf(word as Element), what).toEqual({
    size,
    weight: strong ? "600" : weight,
    colour,
  });
}

/** `word` wears `role`'s class itself, and is drawn as it. */
export function wears(word: Element | null | undefined, role: Role, what: string, strong = false) {
  expect(word, `${what} was not drawn`).toBeTruthy();
  const one = word as Element;
  expect(one.classList.contains(ROLES[role].class), `${what} does not wear the ${role}`).toBe(true);
  expect(one.classList.contains("ui-text-strong"), `${what}: strong`).toBe(strong);
  drawnAs(one, role, what, strong);
}
