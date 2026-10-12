// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Picking in the one select from a test** (`primitives/Select.tsx`, #1860), as a person does it
 * with the keyboard: the select focused, Enter to open its list, and the option pressed. An
 * option out of reach, or a select held off, is refused as a person would find it, and the list
 * is closed again.
 */
import { act, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";

/** Picks the option `name` in `select`, a Radix select's trigger: the option that writes it, or
 *  else the one its words name — as `userEvent.selectOptions` finds one in a native select. */
export async function pickIn(
  select: HTMLElement,
  name: string | RegExp,
  user = userEvent.setup(),
): Promise<void> {
  const said = select.getAttribute("aria-label") ?? select.id;
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

/** The words of each option `select` offers, in order, read from its open list and closed again. */
export async function offeredIn(select: HTMLElement, user = userEvent.setup()): Promise<string[]> {
  act(() => select.focus());
  await user.keyboard("{Enter}");
  const list = await screen.findByRole("listbox");
  const words = within(list)
    .getAllByRole("option")
    .map((one) => one.textContent ?? "");
  await user.keyboard("{Escape}");
  return words;
}

/** What each option `select` offers writes, in order, read as {@link offeredIn} reads its words. */
export async function offeredValuesIn(
  select: HTMLElement,
  user = userEvent.setup(),
): Promise<string[]> {
  act(() => select.focus());
  await user.keyboard("{Enter}");
  const list = await screen.findByRole("listbox");
  const values = within(list)
    .getAllByRole("option")
    .map((one) => one.getAttribute("data-value") ?? "");
  await user.keyboard("{Escape}");
  return values;
}
