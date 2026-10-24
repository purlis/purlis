// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync } from "node:fs";
import { join } from "node:path";
import { $, browser } from "@wdio/globals";
import { ask } from "./switching.js";

/**
 * **Acting in the built window the way a person does, written for the engine CI drives it on.**
 * Shared by the specs for what shipped after 2026-10-08 (memory moves and the archive, the
 * persona mark, the Dispatches tab, Search into a chat, the file preview, a session record's
 * files).
 *
 * Four things measured on the scenario run decide how each helper here is written:
 *
 * - **The WebDriver plugin's keys are untrusted synthetic events.** A page's own key handler
 *   hears them; a browser's default action for them (a form submitted at Enter, a character
 *   typed into a box) may not happen. So a key that must do something is sent by the driver
 *   first and, where nothing came of it, dispatched from the page to the element that has the
 *   keyboard ({@link keyWithFallback}), and a spec asserts what the app did with the key, never
 *   what the browser would have done.
 * - **On Linux a WebDriver click is a `mousedown` and a `click`, and no pointer events.** Every
 *   press here is the element's own `click()` from the page, which is the event React's
 *   `onClick` and Radix's menu rows listen for on both engines.
 * - **A right-click raises no `contextmenu` on either engine**, so a row's menu is opened with
 *   the event the platform would have sent ({@link openMenuOn}), as `workspace-explorer.e2e.ts`
 *   does.
 * - **A read of an element that has left the document does not fail fast**: WebdriverIO finds it
 *   again by its selector and waits for it (`reading.ts`). So every read of a list here is one
 *   `browser.execute`, which sees the document as it is and returns plain values.
 *
 * No function is named inside an `execute`: the spec's bundler wraps a named helper in a
 * `__name` call the page does not have (`view-tabs.e2e.ts`).
 */

/** The strip of tabs a project's views and chats are on. */
export const TABS = '[data-strip="Tabs"]';

/** The project the run started the app in: the first one open. */
export async function projectRoot(): Promise<string> {
  return (await ask<string[]>("open_planes"))[0];
}

/** The project's local state folder, by the spelling it has on the disk (`names::STATE_DIR`). */
export function stateOf(project: string): string {
  return existsSync(join(project, ".purlis"))
    ? join(project, ".purlis")
    : join(project, ".charter");
}

/** What each tab on the strip says, whole and trimmed, left to right, read in one pass. Not
 *  `reach.ts`'s `tabNames`, which reads the name alone. */
export async function tabTexts(): Promise<string[]> {
  return browser.execute((strip: string) => {
    const tabs = document.querySelector(strip)?.querySelectorAll('[role="tab"]') ?? [];
    return [...tabs].map((tab) => (tab.textContent ?? "").trim());
  }, TABS);
}

/** The name of the tab in front on the strip, or `null` when none is. */
export async function inFront(): Promise<string | null> {
  return browser.execute(
    (strip: string) =>
      document.querySelector(`${strip} [role="tab"][aria-selected="true"]`)?.textContent?.trim() ??
      null,
    TABS,
  );
}

/** Waits until the tab in front is called `name`. */
export async function untilInFront(name: string, timeout = 20_000): Promise<void> {
  await browser.waitUntil(async () => (await inFront()) === name, {
    timeout,
    interval: 200,
    timeoutMsg: `${name} never came to the front; in front is ${String(await inFront())}`,
  });
}

/** Closes the view tab called `name` by its `×`, where it is open; a view ends nothing, so
 *  nothing is asked. */
export async function closeTab(name: string): Promise<void> {
  const pressed = await browser.execute(
    (strip: string, label: string) => {
      const closer = [
        ...document.querySelectorAll<HTMLElement>(`${strip} button[aria-label]`),
      ].find((button) => button.getAttribute("aria-label") === `Close ${label}`);
      closer?.click();
      return closer !== undefined;
    },
    TABS,
    name,
  );
  if (!pressed) return;
  await browser.waitUntil(async () => !(await tabTexts()).includes(name), {
    timeout: 20_000,
    interval: 200,
    timeoutMsg: `the ${name} tab did not close`,
  });
}

/** Focuses the workspace `name` from the workspace strip, by the name its tab draws. */
export async function focusWorkspace(name: string): Promise<void> {
  const selected = () =>
    browser.execute((wanted: string) => {
      const tab = [
        ...document.querySelectorAll<HTMLElement>(
          '[data-strip="Workspaces"] [role="tab"]:not(.plane-root)',
        ),
      ].find((one) => one.querySelector(".workspace-name")?.textContent?.trim() === wanted);
      if (tab === undefined) return "absent";
      if (tab.getAttribute("aria-selected") === "true") return "selected";
      tab.click();
      return "pressed";
    }, name);
  await browser.waitUntil(async () => (await selected()) === "selected", {
    timeout: 30_000,
    interval: 250,
    timeoutMsg: `the workspace strip never focused ${name}`,
  });
}

/**
 * Presses the first enabled button inside `within` whose words, or whose accessible name, are
 * `words`, start with them (a row whose note follows its words) or contain them (a row whose
 * mark draws letters before its words), waiting until there is one.
 */
export async function press(
  within: string,
  words: string,
  how: "exactly" | "starting" | "containing" = "exactly",
  timeout = 20_000,
): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute(
        (scope: string, said: string, way: string) => {
          const button = [
            ...document.querySelectorAll<HTMLButtonElement>(
              `${scope} button, ${scope} [role="menuitem"]`,
            ),
          ].find(
            (one) =>
              !one.hasAttribute("disabled") &&
              one.getAttribute("aria-disabled") !== "true" &&
              [one.textContent, one.getAttribute("aria-label")].some((text) =>
                way === "starting"
                  ? (text ?? "").trim().startsWith(said)
                  : way === "containing"
                    ? (text ?? "").includes(said)
                    : (text ?? "").trim() === said,
              ),
          );
          button?.click();
          return button !== undefined;
        },
        within,
        words,
        how,
      ),
    {
      timeout,
      interval: 200,
      timeoutMsg: `nothing in ${within} to press said ${JSON.stringify(words)}`,
    },
  );
}

/**
 * Opens a context menu with the `contextmenu` event a right-click would have sent, at the
 * element's centre, and waits for the menu: on the first element `selector` names, or, given
 * `words`, on the first button inside it whose words start with them.
 */
export async function openMenuOn(selector: string, words?: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute(
        (css: string, said: string | null) => {
          const element =
            said === null
              ? document.querySelector<HTMLElement>(css)
              : ([...document.querySelectorAll<HTMLElement>(`${css} button`)].find((one) =>
                  (one.textContent ?? "").trim().startsWith(said),
                ) ?? null);
          if (element === null) return false;
          const box = element.getBoundingClientRect();
          element.dispatchEvent(
            new MouseEvent("contextmenu", {
              bubbles: true,
              cancelable: true,
              clientX: Math.round(box.left + box.width / 2),
              clientY: Math.round(box.top + box.height / 2),
            }),
          );
          return true;
        },
        selector,
        words ?? null,
      ),
    {
      timeout: 20_000,
      interval: 250,
      timeoutMsg: `${selector} draws nothing${words === undefined ? "" : ` that says ${words}`}`,
    },
  );
  await $('[role="menu"]').waitForDisplayed({ timeout: 10_000 });
}

/** The rows of every open menu, by their words, the innermost menu last. */
export async function menuRows(): Promise<string[][]> {
  return browser.execute(() =>
    [...document.querySelectorAll('[role="menu"]')].map((menu) =>
      [...menu.querySelectorAll('[role="menuitem"]')].map((row) => (row.textContent ?? "").trim()),
    ),
  );
}

/**
 * Presses the row of the innermost open menu (a submenu is drawn after the menu it opened from)
 * whose words start with `words`: a row's note follows its words in its text.
 */
export async function pressMenuRow(words: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute((said: string) => {
        const menus = document.querySelectorAll('[role="menu"]');
        const row = [
          ...(menus[menus.length - 1]?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? []),
        ].find(
          (one) =>
            one.getAttribute("aria-disabled") !== "true" &&
            (one.textContent ?? "").trim().startsWith(said),
        );
        row?.click();
        return row !== undefined;
      }, words),
    { timeout: 10_000, interval: 200, timeoutMsg: `no open menu has a row ${words}` },
  );
}

/**
 * Closes every open menu with Escape: the driver's key, then the page's to the element with the
 * keyboard, where the driver's closed nothing.
 */
export async function closeMenus(): Promise<void> {
  const open = async () => (await menuRows()).length > 0;
  if (!(await open())) return;
  // A row's note that is up takes the first Escape for itself (WCAG 1.4.13), and the next is
  // the menu's: as many as it takes, and no more than a note, a submenu and a menu need.
  for (let tries = 0; tries < 3 && (await open()); tries++)
    await keyWithFallback(["Escape"], { key: "Escape" }, async () => !(await open()));
  if (await open())
    await browser.execute(() =>
      document.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
      ),
    );
  await browser.waitUntil(async () => !(await open()), {
    timeout: 10_000,
    timeoutMsg: "a menu stayed open after Escape",
  });
}

/**
 * Sends `keys` through the driver and, where `landed` has not come true within a few seconds,
 * dispatches `init` as a `keydown` from the page to the element that has the keyboard: what a
 * person's key does there, heard by the same handler.
 */
export async function keyWithFallback(
  keys: string[],
  init: KeyboardEventInit,
  landed: () => Promise<boolean>,
): Promise<void> {
  await browser.keys(keys);
  const came = await browser
    .waitUntil(landed, { timeout: 3_000, interval: 150 })
    .then(() => true)
    .catch(() => false);
  if (came) return;
  await browser.execute((given: KeyboardEventInit) => {
    (document.activeElement ?? document.body).dispatchEvent(
      new KeyboardEvent("keydown", { ...given, bubbles: true, cancelable: true }),
    );
  }, init);
}

/** Gives the keyboard to the first element matching `selector`, from the page. */
export async function focusOn(selector: string): Promise<void> {
  await $(selector).waitForExist({ timeout: 20_000 });
  const took = await browser.execute((css: string) => {
    const element = document.querySelector<HTMLElement>(css);
    element?.focus();
    return element !== null && document.activeElement === element;
  }, selector);
  if (!took) throw new Error(`${selector} did not take the keyboard`);
}

/**
 * Types `text` into the box `selector` as a person's typing reaches the app: the driver's own
 * typing first, and, where the box does not hold `text` after it, the value set the way an
 * input method sets it (the native setter, then an `input` event React's `onChange` hears).
 */
export async function typeInto(selector: string, text: string): Promise<void> {
  const box = await $(selector);
  await box.waitForExist({ timeout: 20_000 });
  await box.setValue(text);
  const held = await browser
    .waitUntil(async () => (await box.getValue()) === text, { timeout: 2_000, interval: 100 })
    .then(() => true)
    .catch(() => false);
  if (held) return;
  await browser.execute(
    (css: string, value: string) => {
      const input = document.querySelector<HTMLInputElement>(css);
      if (input === null) return;
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(input, value);
      input.dispatchEvent(new Event("input", { bubbles: true }));
    },
    selector,
    text,
  );
}

/**
 * Picks `value` in the select `selector`: the select family's (`ui/select/Select.tsx`, #2906),
 * a button (`[data-scope="select"][data-part="root"]`) that opens a listbox. Its list is opened
 * and the option that writes `value` pressed, each as a script's press, which React Aria answers
 * as a virtual press: no pointer this driver synthesises is needed, and none is left down.
 */
export async function choose(selector: string, value: string): Promise<void> {
  await $(selector).waitForExist({ timeout: 20_000 });
  await browser.execute((css: string) => {
    document.querySelector<HTMLElement>(css)?.click();
  }, selector);
  const option = `[role="listbox"] [role="option"][data-value=${JSON.stringify(value)}]`;
  await $('[role="listbox"]').waitForExist({ timeout: 10_000 });
  const picked = await browser.execute((css: string) => {
    const one = document.querySelector<HTMLElement>(css);
    if (one === null || one.getAttribute("aria-disabled") === "true") return false;
    one.click();
    return true;
  }, option);
  if (!picked) throw new Error(`${selector} offers no ${JSON.stringify(value)}`);
  await browser.waitUntil(async () => (await heldBy(selector)) === value, {
    timeout: 10_000,
    timeoutMsg: `${selector} did not keep ${JSON.stringify(value)}`,
  });
}

/** What the select `selector` holds: the value a pick of it wrote, as a field's value reads. */
export async function heldBy(selector: string): Promise<string | null> {
  return $(selector).getAttribute("data-value");
}

/** The text of the first element matching `selector`, read in one pass; `null` where none. */
export async function textOf(selector: string): Promise<string | null> {
  return browser.execute(
    (css: string) => (document.querySelector(css) as HTMLElement | null)?.innerText.trim() ?? null,
    selector,
  );
}

/** Waits until the text of `selector` includes `words`, or no longer does when `shown` is false. */
export async function untilSays(
  selector: string,
  words: string,
  shown = true,
  timeout = 30_000,
): Promise<void> {
  await browser.waitUntil(async () => ((await textOf(selector)) ?? "").includes(words) === shown, {
    timeout,
    interval: 250,
    timeoutMsg: `${selector} ${shown ? "never said" : "still says"} ${JSON.stringify(words)}`,
  });
}
