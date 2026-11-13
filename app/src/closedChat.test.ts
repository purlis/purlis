// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { ChatCard } from "./bindings";
import { narrow } from "./actions";
import { cardRowId, cardWhen, numberRows, opensACard, outcomeOf } from "./closedChat";

const TASK: NonNullable<ChatCard["task"]> = {
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
};

const CARD: ChatCard = {
  number: 641,
  chat: "01K6W0RKER000000000000000B",
  title: "Signed commits #5607",
  ended: "2026-11-11T11:08:00",
  place: "smart-ide",
  started_by: "steward 4",
  screen_kept: true,
  task: TASK,
};

describe("a chat found by its number in the palette (#6214)", () => {
  it("a closed chat is one row, titled by its number and its name, that opens its card", () => {
    const rows = numberRows({ number: 641, lookup: { is: "ended", card: CARD } });
    expect(rows.map((row) => row.title)).toEqual(["Show closed chat #641 · Signed commits #5607"]);
    expect(opensACard(rows[0])).toBe(true);
    expect(rows[0].id).toBe(cardRowId(641));
  });

  it("the finished row is found by 641 and by #641, and not by another number", () => {
    const rows = numberRows({ number: 641, lookup: { is: "ended", card: CARD } });
    expect(narrow("641", rows)).toHaveLength(1);
    expect(narrow(`#${641}`, rows)).toHaveLength(1);
    expect(narrow("642", rows)).toHaveLength(0);
  });

  it("a chat started again shows the chat under its new number", () => {
    const rows = numberRows({ number: 641, lookup: { is: "restarted", now: 668 } });
    expect(rows).toHaveLength(1);
    expect(rows[0].title).toBe("Show chat #668, started again from #641");
    // Not a needs-you item's Go: the chat, and nothing more (#6217).
    expect(rows[0].does).toEqual({ verb: "focusChat", session: 668 });
    expect(opensACard(rows[0])).toBe(false);
  });

  it("an open chat and no chat add no row: an open one's own rows find it", () => {
    expect(numberRows({ number: 641, lookup: { is: "open", number: 641 } })).toEqual([]);
    expect(numberRows({ number: 9999, lookup: { is: "no_chat" } })).toEqual([]);
    expect(numberRows(undefined)).toEqual([]);
  });

  it("the row names only what its card can do", () => {
    const note = (card: ChatCard) =>
      numberRows({ number: 641, lookup: { is: "ended", card } })[0].note;
    expect(note(CARD)).toBe(
      "Done at 11:08 · its chat has closed. Shows its card, with Read it and Reopen.",
    );
    expect(note({ ...CARD, screen_kept: false })).toBe(
      "Done at 11:08 · its chat has closed. Shows its card, with Reopen.",
    );
    expect(note({ ...CARD, task: { ...TASK, reopens: false } })).toBe(
      "Done at 11:08 · its chat has closed. Shows its card, with Read it.",
    );
    // A chat no dispatch started: no screen and no task, so neither.
    expect(note({ ...CARD, screen_kept: false, task: null })).toMatch(/\. Shows its card\.$/);
  });

  it("the card says how and when it ended, and that its chat has closed", () => {
    expect(cardWhen(CARD)).toBe("Done at 11:08 · its chat has closed");
  });

  it("says each end in the words a finished row says, coloured by how it went", () => {
    const of = (how: NonNullable<ChatCard["task"]>["how"], outcome: string) =>
      outcomeOf({ how, outcome });
    expect(of("done", "done")).toMatchObject({ badge: "Done", variant: "ok" });
    expect(of("closed_by_person", "closed by you")).toMatchObject({
      badge: "Closed by you",
      variant: "neutral",
    });
    expect(of("failed", "failed")).toMatchObject({ badge: "Failed", variant: "waiting" });
    // A blocked task is said as a failed one with the core's own word beside it.
    expect(of("blocked", "blocked").said).toBe("Failed (blocked)");
  });
});
