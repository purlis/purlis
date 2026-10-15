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
 * What lay over the element `selector` names, as lines: at its centre, the keyboard's place, and
 * each overlay open. Asked of the window; a window that does not answer says so.
 */
export async function whatCovers(selector: string): Promise<string[]> {
  return browser
    .execute((selector: string) => {
      const describe = (el: Element | null): string => {
        if (el === null) return "nothing";
        const id = el.id ? `#${el.id}` : "";
        const classes =
          typeof el.className === "string" && el.className.trim()
            ? `.${el.className.trim().split(/\s+/).slice(0, 3).join(".")}`
            : "";
        const role = el.getAttribute("role");
        const label = el.getAttribute("aria-label");
        const text = (el.textContent ?? "").trim().replace(/\s+/g, " ").slice(0, 60);
        return [
          `${el.tagName.toLowerCase()}${id}${classes}`,
          role ? `[role=${role}]` : "",
          label ? `"${label}"` : text ? `“${text}”` : "",
        ]
          .filter(Boolean)
          .join(" ");
      };
      // WebdriverIO's own forms: `tag=text`, `tag*=text`, or plain CSS.
      const find = (wanted: string): Element | null => {
        const text = /^([a-z0-9-]*)(\*?)=(.+)$/i.exec(wanted);
        if (!text) {
          try {
            return document.querySelector(wanted);
          } catch {
            return null;
          }
        }
        const [, tag, partly, words] = text;
        for (const el of document.querySelectorAll(tag || "*")) {
          const said = (el.textContent ?? "").trim();
          if (partly ? said.includes(words) : said === words) return el;
        }
        return null;
      };
      const lines: string[] = [];
      const target = find(selector);
      if (target === null) lines.push(`the element: not in the page`);
      else {
        const box = target.getBoundingClientRect();
        lines.push(
          `the element: ${describe(target)}, ${Math.round(box.width)}×${Math.round(box.height)}`,
        );
        const over = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);
        lines.push(
          `at its centre: ${over === target || (over && target.contains(over)) ? "itself" : describe(over)}`,
        );
      }
      lines.push(`focused: ${describe(document.activeElement)}`);
      const open = document.querySelectorAll(
        '[role="dialog"], [role="alertdialog"], [role="menu"], [role="listbox"], ' +
          "[data-radix-popper-content-wrapper]",
      );
      for (const one of open) lines.push(`open: ${describe(one)}`);
      return lines;
    }, selector)
    .catch((err: unknown) => [`the window did not answer: ${String(err).slice(0, 120)}`]);
}
