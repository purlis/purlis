// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import type { ChatCard } from "./bindings";
import { ClosedChatCard } from "./ClosedChatCard";
import { READ_IT, REOPEN, canRead, cannotReopen, numberRows } from "./closedChat";

/**
 * **The card and its palette row say the same** (#6506): the row names *Read it* and *Reopen*
 * exactly where the card draws them enabled, as both read one pair of rules (`canRead`,
 * `cannotReopen`).
 */

afterEach(cleanup);

const TASKED: ChatCard = {
  number: 641,
  chat: "01K6W0RKER000000000000000B",
  title: "Signed commits #5607",
  ended: "2026-11-11T11:08:00",
  place: "smart-ide",
  started_by: "steward 4",
  screen_kept: true,
  dispatched: true,
  task: {
    id: "01K9DISPATCH0000000000000A",
    asker: 0,
    name: "Signed commits #5607",
    chat: null,
    persona: null,
    how: "done",
    outcome: "done",
    folds: true,
    report: "Done.",
    changed: null,
    ended: "2026-11-11T11:08:00",
    place: "smart-ide",
    branch: null,
    reopens: true,
    not_reopened: null,
    did_not_start: false,
    attempts: 0,
    waits: null,
  },
};

const TASK = TASKED.task as NonNullable<ChatCard["task"]>;

const CARDS: [string, ChatCard][] = [
  ["a finished task with its screen", TASKED],
  ["a finished task with no screen", { ...TASKED, screen_kept: false }],
  [
    "a task that does not reopen",
    { ...TASKED, task: { ...TASK, reopens: false, not_reopened: "it was cleared" } },
  ],
  ["a chat no dispatch started", { ...TASKED, task: null, dispatched: false, screen_kept: false }],
  ["a handoff with its screen", { ...TASKED, task: null }],
];

function enabled(): string[] {
  return [READ_IT, REOPEN].filter((name) => {
    const button = screen.getByRole("button", { name });
    return !button.hasAttribute("disabled") && button.getAttribute("aria-disabled") !== "true";
  });
}

describe("a closed chat's card and its palette row (#6506)", () => {
  it.each(CARDS)("%s: the row names the buttons the card enables, and no other", (_, card) => {
    render(<ClosedChatCard card={card} titleId="t" onRead={() => {}} onReopen={() => {}} />);
    const shown = enabled();
    expect(shown).toEqual(
      [canRead(card) && READ_IT, cannotReopen(card) === undefined && REOPEN].filter(Boolean),
    );
    const note = numberRows({ number: card.number, lookup: { is: "ended", card } })[0].note;
    expect(note).toMatch(
      shown.length === 0
        ? /Shows its card\.$/
        : new RegExp(`Shows its card, with ${shown.join(" and ")}\\.$`),
    );
  });
});
