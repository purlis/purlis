// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { ChatCard } from "./bindings";
import { narrow } from "./actions";
import { cardRowId, cardWhen, numberRows, opensACard, outcomeOf } from "./closedChat";

const CARD: ChatCard = {
  number: 641,
  chat: "01K6W0RKER000000000000000B",
  started_by: "steward 4",
  screen_kept: true,
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

describe("a chat found by its number in the palette (#6214)", () => {
  it("a finished chat is one row, titled by its outcome, its number and its name, that opens its card", () => {
    const rows = numberRows({ number: 641, found: { found: "ended", card: CARD } });
    expect(rows.map((row) => row.title)).toEqual(["Finished chat #641 · Signed commits #5607"]);
    expect(opensACard(rows[0])).toBe(true);
    expect(rows[0].id).toBe(cardRowId(641));
  });

  it("the finished row is found by 641 and by #641, and not by another number", () => {
    const rows = numberRows({ number: 641, found: { found: "ended", card: CARD } });
    expect(narrow("641", rows)).toHaveLength(1);
    expect(narrow("#641", rows)).toHaveLength(1);
    expect(narrow("642", rows)).toHaveLength(0);
  });

  it("a chat started again shows the chat under its new number", () => {
    const rows = numberRows({ number: 641, found: { found: "restarted", now: 668 } });
    expect(rows).toHaveLength(1);
    expect(rows[0].title).toBe("Show chat #668, started again from #641");
    expect(rows[0].does).toEqual({ verb: "showChat", session: 668 });
    expect(opensACard(rows[0])).toBe(false);
  });

  it("an open chat and no chat add no row: an open one's own rows find it", () => {
    expect(numberRows({ number: 641, found: { found: "open", number: 641 } })).toEqual([]);
    expect(numberRows({ number: 9999, found: { found: "no_chat" } })).toEqual([]);
    expect(numberRows(undefined)).toEqual([]);
  });

  it("the card says how and when it ended, and that it is no open chat now", () => {
    expect(cardWhen(CARD)).toBe("Finished at 11:08 · it is in no list of open chats now");
  });

  it("a task the person closed reads Closed, and one blocked reads Blocked", () => {
    expect(outcomeOf("closed_by_person").badge).toBe("Closed");
    expect(outcomeOf("blocked").badge).toBe("Blocked");
    expect(outcomeOf("failed").badge).toBe("Failed");
  });
});
