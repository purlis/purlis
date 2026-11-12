// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { answerId, briefId, smartCloseCancelId, stopId } from "./actions";
import {
  FOOT_DROPS,
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

describe("a narrow pane head drops its parts in the ruled order (#4760, #4585 rule 4, #5206)", () => {
  it("drops nothing in a wide pane, or one not measured", () => {
    expect([...droppedAt(900)]).toEqual([]);
    expect([...droppedAt(500)]).toEqual([]);
    expect([...droppedAt(undefined)]).toEqual([]);
  });

  it("drops the chat's number, the mark's word, what the chat uses, the button's word, the mark and Split down, and no workspace", () => {
    // The number first (#6212): its tab's hover says it too.
    expect([...droppedAt(499)]).toEqual(["number"]);
    expect([...droppedAt(469)]).toEqual(["number", "markWord"]);
    expect([...droppedAt(449)]).toEqual(["number", "markWord", "using"]);
    expect([...droppedAt(419)]).toEqual(["number", "markWord", "using", "buttonWord"]);
    expect([...droppedAt(379)]).toEqual(["number", "markWord", "using", "buttonWord", "mark"]);
    expect([...droppedAt(299)]).toEqual([
      "number",
      "markWord",
      "using",
      "buttonWord",
      "mark",
      "splitDown",
    ]);
    expect(HEAD_DROPS.map((drop) => drop.part as string)).not.toContain("where");
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

describe("a narrow pane's two lines (#5206, spec #5191 stories 28 to 32)", () => {
  /** A chat or a task with every fact a line can hold. */
  const full = { from: true, skills: true, studio: true, workItem: true, sandbox: true };
  const ORIGIN = ["from", "studio", "workItem"];
  const WARM = ["sandbox", "ctx", "cache"];

  const chatHead = ["persona", "name", "number", "mark", "markWord"];
  /** Under 500 px the number has gone (#6212). */
  const narrowHead = ["persona", "name", "mark", "markWord"];
  const ALL = ["state", "stateWord", "bell", "menu", "splitRight", "splitDown", "close"];

  it.each([
    // width, the chat's head, its actions, its foot
    [900, chatHead, ALL, ["harness", "workspace", ...ORIGIN, "skills", ...WARM]],
    [520, chatHead, ALL, ["harness", "workspace", ...ORIGIN, "skills", ...WARM]],
    [519, chatHead, ALL, ["harness", "workspace", ...ORIGIN, "skills", "sandbox", "ctx"]],
    [
      450,
      ["persona", "name", "mark"],
      ALL,
      ["harness", "workspace", ...ORIGIN, "skills", "sandbox", "ctx"],
    ],
    [470, narrowHead, ALL, ["harness", "workspace", ...ORIGIN, "skills", "sandbox", "ctx"]],
    [
      449,
      ["persona", "name", "mark"],
      ALL,
      ["harness", "workspace", ...ORIGIN, "skillsMark", "sandbox", "ctx"],
    ],
    [
      420,
      ["persona", "name", "mark"],
      ALL,
      ["harness", "workspace", ...ORIGIN, "skillsMark", "sandbox", "ctx"],
    ],
    [
      419,
      ["persona", "name", "mark"],
      ALL.filter((part) => part !== "stateWord"),
      ["harness", "workspace", ...ORIGIN, "skillsMark", "sandbox", "ctx"],
    ],
    [
      399,
      ["persona", "name", "mark"],
      ALL.filter((part) => part !== "stateWord"),
      ["harness", "workspace", "skillsMark", "sandbox", "ctx"],
    ],
    [
      379,
      ["persona", "name"],
      ALL.filter((part) => part !== "stateWord"),
      ["harness", "workspace", "skillsMark", "sandbox", "ctx"],
    ],
    [
      299,
      ["persona", "name"],
      ALL.filter((part) => part !== "stateWord" && part !== "splitDown"),
      ["harness", "workspace", "skillsMark", "sandbox", "ctx"],
    ],
    [
      269,
      ["persona", "name"],
      ALL.filter((part) => part !== "stateWord" && part !== "splitDown"),
      ["harness", "skillsMark", "sandbox", "ctx"],
    ],
  ] as const)("a chat's pane %s px wide", (width, head, actions, foot) => {
    const lines = paneLinesOf("chat", width, full);
    expect(lines.head).toEqual(head);
    expect(lines.actions).toEqual(actions);
    expect(lines.foot).toEqual(foot);
  });

  it.each([
    // width, the task's foot; its head is its path at every width, its state said there
    [900, ["harness", "workspace", ...ORIGIN, "skills", ...WARM]],
    [519, ["harness", "workspace", ...ORIGIN, "skills", "sandbox", "ctx"]],
    [449, ["harness", "workspace", ...ORIGIN, "skillsMark", "sandbox", "ctx"]],
    [399, ["harness", "workspace", "skillsMark", "sandbox", "ctx"]],
    [269, ["harness", "skillsMark", "sandbox", "ctx"]],
  ] as const)("a task's pane %s px wide", (width, foot) => {
    const lines = paneLinesOf("task", width, { ...full, closes: false });
    expect(lines.head).toEqual(
      width >= 500 ? ["persona", "crumbs", "number"] : ["persona", "crumbs"],
    );
    expect(lines.foot).toEqual(foot);
    expect(lines.actions).not.toContain("close");
  });

  it("always keeps the harness, the ⚠ of a warning and ctx", () => {
    for (const width of [900, 520, 450, 400, 270, 200, 0]) {
      const foot: string[] = paneLinesOf("chat", width, full).foot ?? [];
      expect(foot, `at ${width}`).toContain("harness");
      expect(foot, `at ${width}`).toContain("ctx");
      expect(foot.some((part) => part === "skills" || part === "skillsMark")).toBe(true);
    }
  });

  it("drops each part of the foot at a narrower width than the one before it", () => {
    const widths = FOOT_DROPS.map((drop) => drop.below);
    expect(widths).toEqual([...widths].sort((a, b) => b - a));
    expect(FOOT_DROPS.map((drop) => drop.part)).toEqual(["cache", "skills", "from", "workspace"]);
  });

  it("has only a view's and an empty pane's controls on theirs, which never drop", () => {
    for (const kind of ["view", "empty"] as const)
      for (const width of [undefined, 900, 200])
        expect(paneLinesOf(kind, width, {}).actions).toEqual(["splitRight", "splitDown", "close"]);
  });

  it("draws no close on a one-job program's pane, nor where the pane says it has none", () => {
    expect(paneLinesOf("oneJob", 900, {}).actions).not.toContain("close");
    expect(paneLinesOf("chat", 900, { closes: false }).actions).not.toContain("close");
    expect(paneLinesOf("chat", 900, {}).actions).toContain("close");
  });

  describe("a pane alone in its tab has no Close, the tab's own × doing the same (#5318)", () => {
    const SPLITS = ["splitRight", "splitDown"];

    it.each(["chat", "task", "ended", "endedTask", "shell"] as const)(
      "draws a %s's Close in a split and none alone",
      (kind) => {
        expect(paneLinesOf(kind, 900, {}).actions).toContain("close");
        expect(paneLinesOf(kind, 900, { alone: false }).actions).toContain("close");
        const alone = paneLinesOf(kind, 900, { alone: true }).actions;
        expect(alone).not.toContain("close");
        expect(alone.slice(-2)).toEqual(SPLITS);
      },
    );

    it("draws an empty pane's two splits alone, and its Close only in a split", () => {
      expect(paneLinesOf("empty", undefined, { alone: true }).actions).toEqual(SPLITS);
      expect(paneLinesOf("empty", undefined, {}).actions).toEqual([...SPLITS, "close"]);
    });

    it("draws nothing on a view alone, and all three in a split", () => {
      expect(paneLinesOf("view", undefined, { alone: true }).actions).toEqual([]);
      expect(paneLinesOf("view", undefined, {}).actions).toEqual([...SPLITS, "close"]);
    });

    it("keeps a one-job program's pane with no Close whether it is alone or not", () => {
      for (const alone of [true, false])
        expect(paneLinesOf("oneJob", 900, { alone }).actions).not.toContain("close");
    });

    it("leaves a folded pane its rail, which has no controls to take away", () => {
      expect(paneLinesOf("folded", 40, { alone: true }).actions).toEqual(["bell"]);
    });

    it("gives a narrow pane's drops the same answer: Split down goes under 300, alone or not", () => {
      expect(paneLinesOf("chat", 299, { alone: true }).actions).toEqual([
        "state",
        "bell",
        "menu",
        "splitRight",
      ]);
      expect(paneLinesOf("chat", 299, {}).actions).toEqual([
        "state",
        "bell",
        "menu",
        "splitRight",
        "close",
      ]);
    });
  });

  describe("what the chat uses now, its skill run and its browsers (#5614)", () => {
    const uses = { using: true };

    it("draws it after the state, on every kind of pane that has a head to say it", () => {
      expect(paneLinesOf("chat", 900, uses).head).toEqual([
        "persona",
        "name",
        "number",
        "mark",
        "markWord",
        "using",
      ]);
      expect(paneLinesOf("task", 900, uses).head).toEqual(["persona", "crumbs", "number", "using"]);
      for (const kind of ["ended", "endedTask", "shell", "oneJob"] as const)
        expect(paneLinesOf(kind, 900, uses).head, kind).toContain("using");
      expect(paneLinesOf("chat", undefined, uses).head).toContain("using");
    });

    it("draws nothing where the pane has no way to say it, nor on a rail, a view or an empty pane", () => {
      expect(paneLinesOf("chat", 900, {}).head).not.toContain("using");
      for (const kind of ["folded", "view", "empty"] as const)
        expect(paneLinesOf(kind, 900, uses).head, kind).not.toContain("using");
    });

    it("goes after the state's word and before the mark, so a narrow pane keeps its name and mark", () => {
      expect(paneLinesOf("chat", 450, uses).head).toEqual(["persona", "name", "mark", "using"]);
      expect(paneLinesOf("chat", 449, uses).head).toEqual(["persona", "name", "mark"]);
      expect(paneLinesOf("chat", 379, uses).head).toEqual(["persona", "name"]);
      expect(paneLinesOf("task", 449, uses).head).toEqual(["persona", "crumbs"]);
      const order = HEAD_DROPS.map((drop) => drop.part);
      expect(order.indexOf("using")).toBe(order.indexOf("markWord") + 1);
      expect(order.indexOf("using")).toBeLessThan(order.indexOf("mark"));
    });
  });

  it("keeps a folded pane's bell", () => {
    expect(paneLinesOf("folded", 40, {}).actions).toEqual(["bell"]);
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
    "number",
    "sandbox",
    "mark",
    "markWord",
    "where",
    "from",
    "workItem",
  ];

  it.each([
    // kind, width, the head's line, the foot
    ["chat", 900, ["persona", "name", "number", "mark", "markWord"], WHOLE],
    ["chat", undefined, ["persona", "name", "number", "mark", "markWord"], WHOLE],
    ["task", 900, ["persona", "crumbs", "number"], WHOLE],
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
    const shows = { task: false, oneJob: false, shell: false, ended: false };
    expect(paneKindOf({ ...shows, task: true })).toBe("task");
    expect(paneKindOf({ ...shows, task: true, ended: true })).toBe("endedTask");
    expect(paneKindOf({ ...shows, oneJob: true })).toBe("oneJob");
    expect(paneKindOf({ ...shows, shell: true })).toBe("shell");
    expect(paneKindOf({ ...shows, ended: true })).toBe("ended");
    expect(paneKindOf(shows)).toBe("chat");
    // A shell and a one-job program that ended still have no foot to keep.
    expect(paneKindOf({ ...shows, shell: true, ended: true })).toBe("shell");
    expect(paneKindOf({ ...shows, oneJob: true, ended: true })).toBe("oneJob");
  });

  describe("for each kind of pane (#5205)", () => {
    const ENDED_FOOT = [
      "harness",
      "workspace",
      "from",
      "studio",
      "workItem",
      "sandbox",
      "ended",
      "ctx",
      "cache",
    ];

    it.each([
      // kind, the head's line, the foot
      ["task", ["persona", "crumbs", "number"], WHOLE],
      ["ended", ["persona", "name", "number", "mark", "markWord"], ENDED_FOOT],
      ["endedTask", ["persona", "crumbs", "number"], ENDED_FOOT],
      ["view", [], null],
      ["empty", [], null],
      ["folded", ["persona", "name"], null],
    ] as const)("a %s pane", (kind, head, foot) => {
      const lines = paneLinesOf(kind, 900, full);
      expect(lines.head).toEqual(head);
      expect(lines.foot).toEqual(foot);
    });

    it("says no skills warning on what has ended: nothing there can act on it", () => {
      expect(paneLinesOf("ended", 900, { skills: true }).foot).not.toContain("skills");
      expect(paneLinesOf("endedTask", 900, { skills: true }).foot).not.toContain("skills");
      expect(paneLinesOf("task", 900, { skills: true }).foot).toContain("skills");
    });
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
