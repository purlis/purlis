// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { answerId, briefId, smartCloseCancelId, stopId } from "./actions";
import {
  HEAD_DROPS,
  droppedAt,
  paneKindOf,
  paneLinesOf,
  stateButtonOf,
  type HeadPart,
} from "./paneHead";

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

  it("drops the details first, then the mark's word, the button's word, the mark and Split down", () => {
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

describe("the pane's two lines: what the head and the foot of a chat's pane draw (#5202, #5204)", () => {
  /** A chat with every fact either line can hold. */
  const full = {
    from: true,
    skills: true,
    studio: true,
    workItem: true,
    sandbox: true,
  };
  const WHOLE = [
    "harness",
    "workspace",
    "from",
    "studio",
    "workItem",
    "skills",
    "sandbox",
    "ctx",
    "cache",
  ];
  /** A task's foot says the chat that asked (`from`), its skills and its gauge, as a chat's does:
   *  its breadcrumb is on the head. */
  const FOOTLESS_HEAD = [
    "persona",
    "name",
    "sandbox",
    "mark",
    "markWord",
    "where",
    "from",
    "workItem",
  ];

  it.each([
    // kind, width, the head's line, the foot
    ["chat", 900, ["persona", "name", "mark", "markWord"], WHOLE],
    ["chat", undefined, ["persona", "name", "mark", "markWord"], WHOLE],
    ["task", 900, ["persona", "crumbs"], WHOLE],
    ["shell", 900, FOOTLESS_HEAD, null],
    ["oneJob", 300, ["persona", "name"], null],
  ] as const)("a %s pane %s px wide", (kind, width, head, foot) => {
    const lines = paneLinesOf(kind, width, full);
    expect(lines.head).toEqual(head);
    expect(lines.foot).toEqual(foot);
  });

  it("never draws the foot's facts on a chat's or a task's head, at any width", () => {
    for (const kind of ["chat", "task"] as const)
      for (const width of [undefined, 1200, 900, 520, 420, 300, 0]) {
        const head: string[] = paneLinesOf(kind, width, full).head;
        for (const gone of [
          "harness",
          "workspace",
          "where",
          "from",
          "studio",
          "workItem",
          "skills",
          "sandbox",
          "ctx",
          "cache",
          "gauge",
          "trend",
        ])
          expect(head, `${kind} at ${width}`).not.toContain(gone);
      }
  });

  it("keeps a footless pane's head as it was, its details going in a narrow pane", () => {
    // Spec #5191: shells' and one-job programs' panes "keep their current heads and get no foot".
    for (const kind of ["shell", "oneJob"] as const) {
      expect(paneLinesOf(kind, 900, {}).head).toContain("where");
      expect(paneLinesOf(kind, 900, full).head).toContain("from");
      expect(paneLinesOf(kind, 519, full).head).not.toContain("where");
      expect(paneLinesOf(kind, 519, full).head).not.toContain("from");
      expect(paneLinesOf(kind, 900, {}).foot).toBeNull();
    }
  });

  it("tells a pane's kind by what it shows, a task's path before anything else", () => {
    expect(paneKindOf({ task: true, oneJob: false, shell: false })).toBe("task");
    expect(paneKindOf({ task: false, oneJob: true, shell: false })).toBe("oneJob");
    expect(paneKindOf({ task: false, oneJob: false, shell: true })).toBe("shell");
    expect(paneKindOf({ task: false, oneJob: false, shell: false })).toBe("chat");
  });

  it("draws on the foot only the facts a chat has: the harness, the workspace and the gauge always", () => {
    expect(paneLinesOf("chat", 900, {}).foot).toEqual(["harness", "workspace", "ctx", "cache"]);
    expect(paneLinesOf("chat", 900, { from: true }).foot).toEqual([
      "harness",
      "workspace",
      "from",
      "ctx",
      "cache",
    ]);
    expect(paneLinesOf("chat", 900, { sandbox: true }).foot).toEqual([
      "harness",
      "workspace",
      "sandbox",
      "ctx",
      "cache",
    ]);
  });
});
