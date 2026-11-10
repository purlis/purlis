// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Does, Offer } from "./actions";
import { todoRefOf } from "./todos";

/**
 * **What an artifact's page may offer, and how purlis draws it** (ADR 0122 §8, #5972).
 *
 * A page never performs an action. It offers a `Does` id of the action catalogue
 * (`actions.ts`, the part of a row's id before its colon) and a subject (the part after it);
 * purlis looks that row up in the catalogue the window itself built, so the button reads the
 * catalogue's words and does what the palette's row does, never what the page says.
 *
 * - **Only the ids in {@link OFFERABLE}**: the rows about things a page can read through its
 *   queries (`purlis.todo_list`). Any other is refused with a sentence, never drawn.
 * - **Only about the artifact's own workspace**: a row the catalogue built for another
 *   workspace, or none at all, is refused.
 * - **A delete always asks** ({@link Offerable.asks}): the bar's press opens a question, and
 *   only its answer does the row.
 */

/** One offer a page made, as the core passed it on. */
export type PageOffer = { does: string; subject: string };

/** What a page may offer a row of, and what the workspace it is about is. */
type Offerable = {
  /** The answer of the question a delete asks first; none for a row that does not ask. */
  asks?: string;
  /** The workspace the row's `Does` is about, if it is about one. */
  about: (does: Does) => string | undefined;
};

/** The workspace a todo row's `Does` names. */
function todoWorkspace(does: Does): string | undefined {
  switch (does.verb) {
    case "closeTodo":
    case "reopenTodo":
    case "forgetTodo":
    case "forgetDoneTodo":
      return does.workspace;
    case "openView":
      return does.view.from === null && does.view.view === "todo"
        ? todoRefOf(does.view.key)?.workspace
        : undefined;
    default:
      return undefined;
  }
}

/** **The ids a page may offer**, each with whether it asks first. */
export const OFFERABLE: Readonly<Record<string, Offerable>> = {
  "todo.open": { about: todoWorkspace },
  "todo.done": { about: todoWorkspace },
  "todo.reopen": { about: todoWorkspace },
  "todo.forget": { asks: "Forget", about: todoWorkspace },
  "todo.delete": { asks: "Delete", about: todoWorkspace },
};

/** An offer purlis draws, or the sentence that refused it. */
export type Resolved =
  { key: string; offer: Offer; asks?: string } | { key: string; refused: string };

/**
 * **What purlis draws for `offered`**, made by a page pinned to `workspace`: the catalogue's
 * row it names, through `offerFor`, or why none.
 */
export function resolve(
  offered: PageOffer,
  workspace: string,
  offerFor: ((id: string) => Offer | undefined) | undefined,
): Resolved {
  const key = `${offered.does}:${offered.subject}`;
  if (!Object.prototype.hasOwnProperty.call(OFFERABLE, offered.does)) {
    return {
      key,
      refused: `A page cannot offer ${offered.does}, so purlis did not draw it. ${OFFER_INSTEAD}`,
    };
  }
  const offerable = OFFERABLE[offered.does];
  const offer = offerFor?.(key);
  if (offer === undefined || offerable.about(offer.does) !== workspace) {
    return {
      key,
      refused: `This page offered ${offered.does} for ${offered.subject}, which ${workspace} does not have now`,
    };
  }
  return offerable.asks === undefined ? { key, offer } : { key, offer, asks: offerable.asks };
}

/** What a page's author can offer instead of an id a page may not, said after its refusal. */
const OFFER_INSTEAD = (() => {
  const ids = Object.keys(OFFERABLE);
  return `Change the page to offer one of ${ids.slice(0, -1).join(", ")} or ${ids[ids.length - 1]}`;
})();
