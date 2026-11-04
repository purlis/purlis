// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { answerId, briefId, smartCloseCancelId, stopId } from "./actions";
import { HEAD_DROPS, droppedAt, paneLinesOf, stateButtonOf, type HeadPart } from "./paneHead";

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

describe("the pane's two lines: what the head and the foot of a chat's pane draw (#5202)", () => {
  /** A chat with every fact the head's line still holds until #5204 moves them to the foot. */
  const full = { from: true, skills: true, workItem: true };

  it.each([
    // kind, width, the head's line, the foot
    [
      "chat",
      900,
      ["persona", "name", "sandbox", "mark", "markWord", "skills", "from", "workItem"],
      ["harness", "workspace", "ctx", "cache"],
    ],
    [
      "chat",
      undefined,
      ["persona", "name", "sandbox", "mark", "markWord", "skills", "from", "workItem"],
      ["harness", "workspace", "ctx", "cache"],
    ],
    [
      "chat",
      500,
      ["persona", "name", "mark", "markWord"],
      ["harness", "workspace", "ctx", "cache"],
    ],
    ["chat", 460, ["persona", "name", "mark"], ["harness", "workspace", "ctx", "cache"]],
    ["chat", 300, ["persona", "name"], ["harness", "workspace", "ctx", "cache"]],
    [
      "task",
      900,
      ["persona", "crumbs", "sandbox", "from", "workItem"],
      ["harness", "workspace", "ctx", "cache"],
    ],
    ["task", 300, ["persona", "crumbs"], ["harness", "workspace", "ctx", "cache"]],
    [
      "shell",
      900,
      ["persona", "name", "sandbox", "mark", "markWord", "skills", "from", "workItem"],
      null,
    ],
    ["oneJob", 300, ["persona", "name"], null],
  ] as const)("a %s pane %s px wide", (kind, width, head, foot) => {
    const lines = paneLinesOf(kind, width, full);
    expect(lines.head).toEqual(head);
    expect(lines.foot).toEqual(foot);
  });

  it("never draws harness · workspace or the gauge on the head, at any width", () => {
    for (const kind of ["chat", "task", "shell", "oneJob"] as const)
      for (const width of [undefined, 1200, 900, 520, 420, 300, 0]) {
        const head: string[] = paneLinesOf(kind, width, full).head;
        for (const gone of ["harness", "workspace", "where", "ctx", "cache", "gauge", "trend"])
          expect(head, `${kind} at ${width}`).not.toContain(gone);
      }
  });

  it("keeps the whole foot of a chat at every width, its drops being #5206's", () => {
    for (const width of [undefined, 1200, 520, 300, 0])
      expect(paneLinesOf("chat", width, full).foot).toEqual(["harness", "workspace", "ctx", "cache"]);
  });

  it("draws only the facts a chat has on the head's line", () => {
    expect(paneLinesOf("chat", 900, {}).head).toEqual([
      "persona",
      "name",
      "sandbox",
      "mark",
      "markWord",
    ]);
    expect(paneLinesOf("chat", 900, { from: true }).head).toContain("from");
  });
});
