// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Finding and picking in a select from a test** (`ui/select/Select.tsx`, #2906), as a person
 * does it with the keyboard: the select focused, Enter to open its list, and the option pressed.
 * An option out of reach, or a select held off, is refused as a person would find it, and the
 * list is closed again.
 *
 * A select is React Aria's: a button that opens a listbox (`aria-haspopup="listbox"`), named by
 * what it holds and then by its own name. {@link selectNamed} finds it by its own name, as a
 * person names it.
 */
import { act, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";

type Named = string | RegExp;

const SELECT = '[data-scope="select"][data-part="root"]';

/** Whether a button's accessible name, which React Aria writes as what it holds and then its
 *  own name, ends in `name`. */
function namedBy(name: Named) {
  return (said: string, element: Element): boolean => {
    if (!element.matches(SELECT)) return false;
    if (typeof name !== "string") return name.test(said) || name.test(ownName(element));
    return said === name || said.endsWith(` ${name}`) || ownName(element) === name;
  };
}

/** A select's own name: its `aria-label`, or the words of what labels it. */
function ownName(element: Element): string {
  const label = element.getAttribute("aria-label");
  if (label !== null) return label;
  const own = (element.getAttribute("aria-labelledby") ?? "")
    .split(/\s+/)
    .filter((id) => id !== "" && element.querySelector(`[id="${id}"]`) === null)
    .map((id) => element.ownerDocument.getElementById(id)?.textContent ?? "");
  return own.join(" ").trim();
}

const scoped = (scope?: HTMLElement) => (scope === undefined ? screen : within(scope));

/** **The select named `name`**, in `scope` or the whole page. */
export function selectNamed(name: Named, scope?: HTMLElement): HTMLElement {
  return scoped(scope).getByRole("button", { name: namedBy(name) });
}

/** The select named `name` once it is drawn. */
export function findSelectNamed(name: Named, scope?: HTMLElement): Promise<HTMLElement> {
  return scoped(scope).findByRole("button", { name: namedBy(name) });
}

/** The select named `name`, or `null` where none is drawn. */
export function querySelectNamed(name: Named, scope?: HTMLElement): HTMLElement | null {
  return scoped(scope).queryByRole("button", { name: namedBy(name) });
}

/** Every select in `scope`, or the whole page, in order. */
export function allSelects(scope: HTMLElement = document.body): HTMLElement[] {
  return [...scope.querySelectorAll<HTMLElement>(SELECT)];
}

/** Picks the option `name` in `select`: the option that writes it, or else the one its words
 *  name, as `userEvent.selectOptions` finds one in a native select. */
export async function pickIn(
  select: HTMLElement,
  name: Named,
  user = userEvent.setup(),
): Promise<void> {
  const said = ownName(select) || select.id;
  if (select.hasAttribute("disabled") || select.hasAttribute("data-disabled"))
    throw new Error(`${said} is out of reach`);
  act(() => select.focus());
  await user.keyboard("{Enter}");
  const list = await screen.findByRole("listbox");
  const option =
    within(list)
      .getAllByRole("option")
      .find((one) => typeof name === "string" && one.getAttribute("data-value") === name) ??
    within(list).getByRole("option", { name });
  if (option.getAttribute("aria-disabled") === "true") {
    await user.keyboard("{Escape}");
    throw new Error(`${String(name)} is out of reach in ${said}`);
  }
  await user.click(option);
}

/** The value `select` holds: what a pick of it would write. */
export function chosenIn(select: HTMLElement): string | null {
  return select.getAttribute("data-value");
}

/** The options `select` offers, read from its open list and closed again. */
async function offered(select: HTMLElement, user: ReturnType<typeof userEvent.setup>) {
  act(() => select.focus());
  await user.keyboard("{Enter}");
  const list = await screen.findByRole("listbox");
  const options = within(list).getAllByRole("option");
  const read = options.map((one) => ({
    // Its words, without the line under them that says why it is out of reach.
    words: (one.querySelector('[data-part="label"]') ?? one).textContent ?? "",
    value: one.getAttribute("data-value") ?? "",
  }));
  await user.keyboard("{Escape}");
  return read;
}

/** The words of each option `select` offers, in order. */
export async function offeredIn(select: HTMLElement, user = userEvent.setup()): Promise<string[]> {
  return (await offered(select, user)).map((one) => one.words);
}

/** What each option `select` offers writes, in order. */
export async function offeredValuesIn(
  select: HTMLElement,
  user = userEvent.setup(),
): Promise<string[]> {
  return (await offered(select, user)).map((one) => one.value);
}
