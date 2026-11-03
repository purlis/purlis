// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { answerId, briefId, smartCloseCancelId, stopId } from "./actions";
import { HEAD_DROPS, droppedAt, stateButtonOf, type HeadPart } from "./paneHead";

/** A chat whose list holds exactly `ids`. */
function listing(...ids: string[]) {
  return (id: string) => ids.includes(id);
}

describe("the pane head's one state button (#4760)", () => {
  it("is Cancel smart close while the chat wraps up, before anything its state says", () => {
    const has = listing(smartCloseCancelId(4), stopId(4));
    expect(stateButtonOf({ session: 4, task: false, working: true, has })).toEqual({
      id: smartCloseCancelId(4),
      word: "Cancel smart close",
    });
  });

  it("is Answer while a task asks", () => {
    const has = listing(answerId(9), briefId(9));
    expect(stateButtonOf({ session: 9, task: true, working: false, has })).toEqual({
      id: answerId(9),
      word: "Answer",
    });
  });

  it("is Stop while a chat works", () => {
    const has = listing(stopId(4));
    expect(stateButtonOf({ session: 4, task: false, working: true, has })).toEqual({
      id: stopId(4),
      word: "Stop",
    });
  });

  it("is Brief on a task that does not ask, its stop being in the chat's list", () => {
    const has = listing(briefId(9));
    expect(stateButtonOf({ session: 9, task: true, working: false, has })?.word).toBe("Brief");
    expect(stateButtonOf({ session: 9, task: true, working: true, has })?.word).toBe("Brief");
  });

  it("is nothing on a quiet chat", () => {
    const has = listing(stopId(4));
    expect(stateButtonOf({ session: 4, task: false, working: false, has })).toBeUndefined();
  });

  it("is never a row the chat's list does not have", () => {
    // A shell that works has a Stop; one whose list has no Stop (it is restoring) shows none.
    expect(
      stateButtonOf({ session: 4, task: false, working: true, has: listing() }),
    ).toBeUndefined();
    expect(
      stateButtonOf({ session: 9, task: true, working: false, has: listing() }),
    ).toBeUndefined();
  });
});

describe("a narrow pane head drops its parts in the ruled order (#4760, #4585 rule 4)", () => {
  const order = (width: number) => [...droppedAt(width)].sort();

  it("drops nothing in a wide pane, or one not measured", () => {
    expect(order(900)).toEqual([]);
    expect(order(520)).toEqual([]);
    expect([...droppedAt(undefined)]).toEqual([]);
  });

  it("drops harness · workspace first, then the mark's word, the button's word, the mark and Split down", () => {
    expect([...droppedAt(519)]).toEqual(["where"]);
    expect([...droppedAt(469)]).toEqual(["where", "markWord"]);
    expect([...droppedAt(419)]).toEqual(["where", "markWord", "buttonWord"]);
    expect([...droppedAt(379)]).toEqual(["where", "markWord", "buttonWord", "mark"]);
    expect([...droppedAt(299)]).toEqual(["where", "markWord", "buttonWord", "mark", "splitDown"]);
  });

  it("never drops the bell, ⋯, Split right or Close", () => {
    const kept: string[] = ["bell", "menu", "splitRight", "close"];
    const droppable = HEAD_DROPS.map((drop) => drop.part as string);
    for (const part of kept) expect(droppable).not.toContain(part);
    expect([...droppedAt(0)].length).toBe(HEAD_DROPS.length);
  });

  it("drops each part at a narrower width than the one before it", () => {
    const widths = HEAD_DROPS.map((drop) => drop.below);
    expect(widths).toEqual([...widths].sort((a, b) => b - a));
    const parts: HeadPart[] = HEAD_DROPS.map((drop) => drop.part);
    expect(new Set(parts).size).toBe(parts.length);
  });
});
