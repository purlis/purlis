// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { expect } from "vitest";
import { waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";

/**
 * Test-only: **a Chats row's card, as a person brings it up** (#1675): a popover with the
 * `tooltip` role, which is what these find it by.
 */

/** The card's own `tooltip`, never a hint's (`ui/tooltip`, which draws `data-scope="tooltip"`). */
const CARD = '[role="tooltip"]:not([data-scope="tooltip"])';

/** The card that is up, or nothing. */
export const card = () => document.querySelector<HTMLElement>(CARD);

/** The card, once it has come up: after the rest it waits for. */
export const theCard = () =>
  waitFor(
    () => {
      const up = card();
      expect(up).not.toBeNull();
      return up as HTMLElement;
    },
    { timeout: 3000 },
  );

/** What a card says of each of its facts, by the fact's word. */
export const facts = (shown: HTMLElement | null): Record<string, string | undefined> =>
  Object.fromEntries(
    [...(shown?.querySelectorAll("dt") ?? [])].map((term) => [
      term.textContent ?? "",
      term.nextElementSibling?.textContent ?? undefined,
    ]),
  );

/**
 * What `row`'s card says, brought up by a pointer resting on the row and taken down again once
 * read: its whole text, and its facts.
 */
export async function cardOf(row: HTMLElement): Promise<{
  text: string;
  facts: Record<string, string | undefined>;
}> {
  // A card the pointer just left stays a moment (`CARD_LEAVE_MS`): read this row's, not that.
  await waitFor(() => expect(card()).toBeNull());
  await userEvent.hover(row);
  const shown = await theCard();
  const read = { text: shown.textContent ?? "", facts: facts(shown) };
  await userEvent.unhover(row);
  await waitFor(() => expect(card()).toBeNull());
  return read;
}
