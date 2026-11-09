// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";

/**
 * **What covered an element a test could not press or see** (#2029's review). A spec that ends
 * with "still not clickable" or "still not displayed" was most often looking at something else
 * on top: a palette, a menu, a dialog an earlier spec left up, which `noDialogIsLeftUp` does not
 * see when it is not a dialog. The failure's record says what was at the element's centre, what
 * had the keyboard, and every overlay open, for its annotation (`failures.ts`).
 */

/** The selector WebdriverIO names in `message`, where it could not press or see that element. */
export function coveredIn(message: string): string | undefined {
  if (!/still not (clickable|displayed)/.test(message)) return undefined;
  const named = /element \("(.*)"\) still not/.exec(message);
  return named?.[1];
}

/**
 * **What a press that missed was pressing, as lines** (#5609), asked of the page: the target's
 * box, what is drawn at its centre (where WebdriverIO's click lands and what its
 * `isClickable` asks), whether it is disabled or kept from the pointer, every animation still
 * running on it or on what holds it, what has the keyboard, each overlay open, and an outline of
 * what is drawn around it with the target marked.
 *
 * `target` is the element itself, or WebdriverIO's selectors for it where it was never found:
 * one, or a chain (`$(a).$(b)`) each looked for inside what the one before found. It runs in the
 * page through `browser.execute`, so it is self-contained: nothing from outside its body, and
 * `budget` handed in. It reads no field's value, and no text but a control's or a heading's own
 * words, cut short; a field, anything holding one and anything in a terminal give only their
 * label. Its lines keep within `budget` characters, the outline cut first, so the lines about the
 * element itself always reach the annotation (`tools/e2e-annotations.mjs`'s EVIDENCE_CHARS).
 */
export function aMissedPress(
  target: Element | string | readonly string[],
  budget = MISSED_PRESS_CHARS,
): string[] {
  const LEAF_TAGS = /^(button|a|h[1-6]|label|legend|summary|option)$/;
  const LEAF_ROLES = /^(button|tab|menuitem\w*|option|heading|link|radio|checkbox|switch)$/;
  const FIELDS = "input, textarea, select, [contenteditable], .xterm";
  const describe = (el: Element | null): string => {
    if (el === null) return "nothing";
    const id = el.id ? `#${el.id.slice(0, 40)}` : "";
    const classes =
      typeof el.className === "string" && el.className.trim()
        ? `.${el.className.trim().split(/\s+/).slice(0, 3).join(".").slice(0, 60)}`
        : "";
    const role = el.getAttribute("role");
    const state = el.getAttribute("data-state");
    const label = el.getAttribute("aria-label");
    const tag = el.tagName.toLowerCase();
    const wordy =
      el.closest(FIELDS) === null &&
      el.querySelector(FIELDS) === null &&
      (LEAF_TAGS.test(tag) || LEAF_ROLES.test(role ?? ""));
    const words = wordy ? (el.textContent ?? "").trim().replace(/\s+/g, " ") : "";
    return [
      `${tag}${id}${classes}`,
      role ? `[role=${role}]` : "",
      state ? `[data-state=${state}]` : "",
      label ? `"${label.slice(0, 40)}"` : words ? `“${words.slice(0, 40)}”` : "",
    ]
      .filter(Boolean)
      .join(" ");
  };
  // WebdriverIO's own forms: `tag=text`, `tag*=text`, or plain CSS.
  const find = (wanted: string, inside: ParentNode): Element | null => {
    const text = /^([a-z0-9-]*)(\*?)=(.+)$/i.exec(wanted);
    if (!text) {
      try {
        return inside.querySelector(wanted);
      } catch {
        return null;
      }
    }
    const [, tag, partly, words] = text;
    for (const el of inside.querySelectorAll(tag || "*")) {
      const said = (el.textContent ?? "").trim();
      if (partly ? said.includes(words) : said === words) return el;
    }
    return null;
  };
  const OVERLAYS =
    '[role="dialog"], [role="alertdialog"], [role="menu"], [role="listbox"], ' +
    '[data-scope="popover"][data-part="root"]';
  const lines: string[] = [];
  const chain = typeof target === "string" ? [target] : target;
  let it: Element | null = null;
  if (Array.isArray(chain)) {
    let inside: ParentNode | null = document;
    for (const wanted of chain as readonly string[]) {
      inside = inside === null ? null : find(wanted, inside);
    }
    it = inside as Element | null;
  } else it = target as Element;
  if (it === null || !it.isConnected) lines.push("the element: not in the page");
  else {
    const box = it.getBoundingClientRect();
    lines.push(
      `the element: ${describe(it)}, ${Math.round(box.width)}×${Math.round(box.height)} ` +
        `at ${Math.round(box.left)},${Math.round(box.top)}`,
    );
    const inView =
      box.right > 0 && box.bottom > 0 && box.left < innerWidth && box.top < innerHeight;
    if (!inView) lines.push(`in view: no, the window is ${innerWidth}×${innerHeight}`);
    const over = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);
    lines.push(
      `at its centre: ${over === it || (over !== null && it.contains(over)) ? "itself" : describe(over)}`,
    );
    const why: string[] = [];
    if ((it as HTMLButtonElement).disabled === true) why.push("disabled");
    if (it.getAttribute("aria-disabled") === "true") why.push("aria-disabled");
    const inert = it.closest("[inert]");
    if (inert !== null) why.push(`inert from ${inert === it ? "itself" : describe(inert)}`);
    lines.push(why.length === 0 ? "disabled: no" : `disabled: yes, ${why.join(", ")}`);
    const kept: string[] = [];
    for (let up: Element | null = it; up !== null; up = up.parentElement) {
      // Visibility and pointer-events are inherited: named where they are set, not below.
      const style = getComputedStyle(up);
      const above = up.parentElement === null ? null : getComputedStyle(up.parentElement);
      if (style.opacity === "0") kept.push(`opacity 0 on ${describe(up)}`);
      if (style.visibility === "hidden" && above?.visibility !== "hidden")
        kept.push(`visibility hidden on ${describe(up)}`);
      if (style.pointerEvents === "none" && above?.pointerEvents !== "none")
        kept.push(`pointer-events none on ${describe(up)}`);
    }
    lines.push(kept.length === 0 ? "reachable: yes" : `reachable: no, ${kept.join("; ")}`);
    if (typeof document.getAnimations === "function") {
      let any = false;
      for (const one of document.getAnimations()) {
        const on = (one.effect as KeyframeEffect | null)?.target;
        if (!(on instanceof Element) || !on.contains(it)) continue;
        any = true;
        const named = one as Animation & { animationName?: string; transitionProperty?: string };
        const name = named.animationName ?? named.transitionProperty ?? (one.id || "an animation");
        const duration = one.effect?.getTiming().duration;
        const at = Math.round(Number(one.currentTime ?? 0));
        lines.push(
          `animating: ${name} on ${describe(on)}, ${one.playState}, ${at} of ` +
            `${typeof duration === "number" ? duration : String(duration)} ms`,
        );
      }
      if (!any) lines.push("animating: nothing");
    }
  }
  lines.push(`focused: ${describe(document.activeElement)}`);
  for (const one of document.querySelectorAll(OVERLAYS)) lines.push(`open: ${describe(one)}`);
  if (it !== null && it.isConnected) {
    // The overlay that holds it, or what holds it two levels up: depth-limited, but always
    // down the branch that leads to it.
    const root =
      it.parentElement?.closest(OVERLAYS) ?? it.parentElement?.parentElement ?? document.body;
    const MOST = 30;
    const outline: string[] = [];
    let left = 0;
    const walk = (el: Element, depth: number): void => {
      if (outline.length >= MOST) {
        left++;
        return;
      }
      outline.push(`${"  ".repeat(depth)}${describe(el)}${el === it ? " ← the element" : ""}`);
      if (el.matches("svg, " + FIELDS)) return;
      for (const child of el.children) {
        if (child.matches("script, style")) continue;
        if (depth < 4 || child.contains(it)) walk(child, depth + 1);
      }
    };
    walk(root, 1);
    lines.push("around it:", ...outline);
    if (left > 0) lines.push(`  … and ${left} more`);
  }
  // Within the budget: from the end, the outline first, with a line that counts what went.
  const size = (some: string[]): number => some.reduce((n, line) => n + line.length + 1, 0);
  if (size(lines) <= budget) return lines;
  let cut = 0;
  while (lines.length > 1 && size(lines) + 20 > budget) {
    lines.pop();
    cut++;
  }
  lines.push(`  … and ${cut} more`);
  return lines;
}

/**
 * What a press that missed was pressing (`aMissedPress`): `target` is the element, handed to the
 * page as itself where it was found, or its selector. A window that does not answer says so.
 */
export async function whatWasOver(target: WebdriverIO.Element | string): Promise<string[]> {
  const chain = typeof target === "string" ? [target] : selectorsOf(target);
  const asked = (handed: Element | readonly string[]) =>
    browser.execute(aMissedPress, handed, MISSED_PRESS_CHARS);
  const said = (err: unknown) => [`the window did not answer: ${String(err).slice(0, 120)}`];
  if (typeof target === "string" || !target.elementId) return asked(chain).catch(said);
  // The element itself, or, where it went stale as the page drew again, its selectors.
  return asked(target as unknown as Element).catch(() => asked(chain).catch(said));
}

/** The selectors `$(a).$(b)` was found by, outermost first; a selector that is not a string ends
 *  the chain (WebdriverIO's functions and element references). */
function selectorsOf(element: WebdriverIO.Element): string[] {
  const chain: string[] = [];
  for (
    let at: { selector?: unknown; parent?: unknown } | undefined = element;
    at !== undefined && typeof at.selector === "string";
    at = at.parent as typeof at
  )
    chain.unshift(at.selector);
  return chain;
}

/** How many characters `aMissedPress` says at most (#5609's review). */
export const MISSED_PRESS_CHARS = 1500;

/** The element commands a press or a sight goes through (`describeEveryMissedPress`). */
const PRESSES = [
  "waitForExist",
  "click",
  "doubleClick",
  "waitForClickable",
  "waitForDisplayed",
  "waitForEnabled",
] as const;

/** The message of `error`, an `Error` or one shaped like it. */
export function messageOf(error: unknown): string {
  return typeof error === "object" && error !== null && "message" in error
    ? String((error as { message: unknown }).message)
    : String(error ?? "");
}

/**
 * **A press that misses is described as it misses** (#5609): each of PRESSES, for every element
 * of the session, takes `aMissedPress`'s evidence before its failure leaves it, for the failure's
 * record to carry (`theMissBehind`). Asked afterwards, the page may already have been put away:
 * `closingByTheWindow` presses Escape on the close question before it throws, and the record of
 * the macOS miss said only "the element: not in the page". By `overwriteCommand`, WebdriverIO's
 * own way to wrap a command, which hands the wrapper the element as `this`.
 */
export function describeEveryMissedPress(): void {
  for (const name of PRESSES) {
    browser.overwriteCommand(
      name,
      async function (
        this: WebdriverIO.Element,
        original: (...args: unknown[]) => Promise<unknown>,
        ...args: unknown[]
      ) {
        try {
          return await original(...args);
        } catch (error) {
          rememberTheMiss(messageOf(error), await whatWasOver(this));
          throw error;
        }
      } as never,
      true,
    );
  }
}

/** The latest press that missed: its failure's message, and what the page said then. */
let latest: { message: string; lines: string[] } | undefined;

/** Keeps `lines` as what the press that failed with `message` was pressing; only the latest. */
export function rememberTheMiss(message: string, lines: string[]): void {
  latest = { message, lines };
}

/** Forgets the latest miss: as each test or hook begins, so a miss a spec caught on purpose
 *  is never handed to a later failure with the same message. */
export function forgetTheMiss(): void {
  latest = undefined;
}

/**
 * What was taken as the press behind a failure with `message` missed, once; `undefined` where
 * the latest miss is another's.
 */
export function theMissBehind(message: string): string[] | undefined {
  if (latest?.message !== message) return undefined;
  const { lines } = latest;
  latest = undefined;
  return lines;
}
