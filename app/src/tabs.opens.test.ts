// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  contentsOf,
  type Fronts,
  focusedChat,
  insertPane,
  noTabs,
  noteUsed,
  openEmptyTab,
  openNew,
  openOneJob,
  type Opening,
  opensAt,
  openTab,
  openView,
  press,
  wouldLeaveStrip,
  type Pressing,
  selectTab,
  setPinned,
  type Tabs,
  type ViewRef,
  workspaceOf,
} from "./tabs";

/**
 * **One rule says where a chat opens** (#5289; spec #5288 stories 66 to 72): what is opening ×
 * the shape of the front tab. New chats never replace anything; a pressed chat shows in a
 * one-pane tab, and a split keeps its layout. Every path that opens a chat asks `opensAt`.
 */

const PERSONA: ViewRef = { from: null, view: "persona", key: "steward" };

/** Chats 1 to 8 work in `ops`, 9 and up in `web`. */
const filedIn = (session: number) => (session < 9 ? "ops" : "web");
const pressing = (fronts: Fronts = {}): Pressing => ({
  filedIn,
  frontOf: (workspace) => fronts[workspace],
});

const pressed = (session: number) => ({ session, chat: String(session), who: "steward" });

const onePane = () => openTab(noTabs(), 1, "1", "steward");

function beside(tabs: Tabs, content: Parameters<typeof insertPane>[1]["content"]): Tabs {
  const id = tabs.inFront as number;
  return insertPane(tabs, {
    tab: id,
    pane: tabs.byId[id].focused,
    direction: "row",
    side: "after",
    content,
  });
}

const session = (n: number) => ({ kind: "session", session: n, chat: String(n) }) as const;
const nothing = { kind: "empty", workspace: "ops" } as const;

/** The front tab's shapes the rule tells apart. */
const SHAPES: Record<string, () => Tabs> = {
  "one pane, a chat": onePane,
  "one pane, empty": () => openEmptyTab(noTabs(), "ops"),
  "one pane, a view": () => openView(noTabs(), PERSONA, "steward", "ops"),
  "one pane, a one-job program": () => openOneJob(noTabs(), 5, "vim"),
  "split, two chats": () => beside(onePane(), session(2)),
  "split, a chat and an empty pane": () => beside(onePane(), nothing),
  "split, a chat and a one-job program": () => openOneJob(onePane(), 5, "vim"),
  "split, a chat and a view": () =>
    beside(onePane(), { kind: "view", view: PERSONA, workspace: "ops" }),
};

/** How it opens, and which pane of the tab in front, in reading order, where it names one. */
type Row = [shape: string, how: "fills" | "shows" | "goes-to" | "new-tab", pane?: number];

const TABLE: { opening: string; of: Opening; rows: Row[] }[] = [
  {
    opening: "a new chat",
    of: { kind: "new" },
    rows: [
      ["one pane, a chat", "new-tab"],
      ["one pane, empty", "fills", 0],
      ["one pane, a view", "new-tab"],
      ["one pane, a one-job program", "new-tab"],
      ["split, two chats", "new-tab"],
      ["split, a chat and an empty pane", "fills", 1],
      ["split, a chat and a one-job program", "new-tab"],
      ["split, a chat and a view", "new-tab"],
    ],
  },
  {
    opening: "a pressed chat that's off screen",
    of: { kind: "pressed", session: 3 },
    rows: [
      ["one pane, a chat", "shows", 0],
      ["one pane, empty", "fills", 0],
      ["one pane, a view", "new-tab"],
      ["one pane, a one-job program", "new-tab"],
      ["split, two chats", "new-tab"],
      ["split, a chat and an empty pane", "fills", 1],
      ["split, a chat and a one-job program", "new-tab"],
      ["split, a chat and a view", "new-tab"],
    ],
  },
  {
    opening: "a pressed chat that's on screen",
    of: { kind: "pressed", session: 1 },
    rows: [
      ["one pane, a chat", "goes-to", 0],
      ["split, two chats", "goes-to", 0],
      ["split, a chat and an empty pane", "goes-to", 0],
      ["split, a chat and a one-job program", "goes-to", 0],
      ["split, a chat and a view", "goes-to", 0],
    ],
  },
];

describe("where a chat opens (#5289)", () => {
  for (const { opening, of, rows } of TABLE) {
    describe(opening, () => {
      it.each(rows)("in a front tab of %s: %s", (shape, how, pane) => {
        const tabs = SHAPES[shape]();

        const opens = opensAt(tabs, of, pressing());

        expect(opens.how).toBe(how);
        if (pane !== undefined && opens.how !== "new-tab") {
          expect(opens.at.tab).toBe(tabs.inFront);
          expect(opens.at.pane).toBe(contentsOf(tabs, opens.at.tab)[pane].pane);
        }
      });
    });
  }

  it("fills the focused pane first, where the tab has two empty ones", () => {
    const tabs = beside(beside(onePane(), nothing), nothing);
    const id = tabs.inFront as number;
    const [, first, second] = contentsOf(tabs, id);
    expect(tabs.byId[id].focused).toBe(second.pane);

    const onFirst: Tabs = {
      ...tabs,
      byId: { ...tabs.byId, [id]: { ...tabs.byId[id], focused: first.pane } },
    };

    expect(opensAt(tabs, { kind: "new" }, pressing())).toEqual({
      how: "fills",
      at: { tab: id, pane: second.pane },
    });
    expect(opensAt(onFirst, { kind: "new" }, pressing())).toEqual({
      how: "fills",
      at: { tab: id, pane: first.pane },
    });
  });

  it("goes to a chat on screen in another tab", () => {
    const tabs = openTab(onePane(), 2, "2", "steward");
    const [first] = tabs.order;

    expect(opensAt(tabs, { kind: "pressed", session: 1 }, pressing())).toEqual({
      how: "goes-to",
      at: { tab: first, pane: contentsOf(tabs, first)[0].pane },
    });
  });

  it("fills only the front tab's empty pane: another tab's keeps waiting", () => {
    const empty = openEmptyTab(noTabs(), "ops");
    const tabs = openTab(empty, 1, "1", "steward");

    expect(opensAt(tabs, { kind: "new" }, pressing()).how).toBe("new-tab");
  });

  it("reads a start that named its workspace against that strip's front tab", () => {
    const web = openEmptyTab(onePane(), "web");
    const webTab = web.inFront as number;
    const tabs = selectTab(web, web.order[0]);

    expect(opensAt(tabs, { kind: "new" }, pressing()).how).toBe("new-tab");
    expect(opensAt(tabs, { kind: "new", strip: "web" }, pressing())).toEqual({
      how: "fills",
      at: { tab: webTab, pane: contentsOf(tabs, webTab)[0].pane },
    });
  });

  it("fills the pane New chat was pressed for, while it is still empty", () => {
    const tabs = beside(onePane(), nothing);
    const id = tabs.inFront as number;
    const pane = contentsOf(tabs, id)[1].pane;
    const elsewhere = openTab(tabs, 2, "2", "steward");

    expect(opensAt(elsewhere, { kind: "new", asked: { tab: id, pane } }, pressing())).toEqual({
      how: "fills",
      at: { tab: id, pane },
    });
  });

  it("forgets the pane New chat was pressed for once something else fills it", () => {
    const tabs = beside(onePane(), session(2));
    const id = tabs.inFront as number;
    const pane = contentsOf(tabs, id)[1].pane;

    expect(opensAt(tabs, { kind: "new", asked: { tab: id, pane } }, pressing()).how).toBe(
      "new-tab",
    );
  });

  it("shows a press in the pane it names, even in a split: a breadcrumb or ⌘⇧J's pick", () => {
    const tabs = beside(onePane(), session(2));
    const id = tabs.inFront as number;
    const pane = contentsOf(tabs, id)[0].pane;

    expect(
      opensAt(tabs, { kind: "pressed", session: 3, into: { tab: id, pane } }, pressing()),
    ).toEqual({ how: "shows", at: { tab: id, pane } });
  });

  describe("a pressed chat with a lone view or program in front", () => {
    /** Tab 1 with chat 1, used; then `then` in front. */
    function behind(then: (tabs: Tabs) => Tabs): Tabs {
      return noteUsed(then(noteUsed(onePane())));
    }

    it("goes to the one-pane chat tab used last, and leaves the view alone", () => {
      const tabs = behind((was) => openView(was, PERSONA, "steward", "ops"));

      expect(opensAt(tabs, { kind: "pressed", session: 3 }, pressing())).toEqual({
        how: "shows",
        at: { tab: tabs.order[0], pane: contentsOf(tabs, tabs.order[0])[0].pane },
      });
    });

    it("gets a new tab with a lone one-job program in front, and touches no other tab", () => {
      const job = openOneJob(noTabs(), 5, "vim");
      const tabs = selectTab(noteUsed(openTab(job, 1, "1", "steward")), job.inFront as number);

      expect(opensAt(tabs, { kind: "pressed", session: 3 }, pressing()).how).toBe("new-tab");
    });

    it("gets a new tab where the tab used last is a split, so that layout survives", () => {
      const tabs = noteUsed(
        openView(noteUsed(beside(onePane(), session(2))), PERSONA, "steward", "ops"),
      );

      expect(opensAt(tabs, { kind: "pressed", session: 3 }, pressing()).how).toBe("new-tab");
    });
  });

  it("reads a chat of another workspace against its own strip", () => {
    const opsSplit = beside(onePane(), session(2));
    const tabs = openTab(opsSplit, 9, "9", "steward");
    const web = tabs.inFront as number;
    const back = selectTab(tabs, tabs.order[0]);

    expect(opensAt(back, { kind: "pressed", session: 10 }, pressing({ web }))).toEqual({
      how: "shows",
      at: { tab: web, pane: contentsOf(back, web)[0].pane },
    });
  });
});

describe("what opening leaves (#5289)", () => {
  it("a new chat in a split leaves both panes alone and opens a tab of its own", () => {
    const tabs = beside(onePane(), session(2));
    const split = tabs.inFront as number;

    const { tabs: next } = openNew(tabs, pressed(3), pressing());

    expect(next.order).toHaveLength(2);
    expect(contentsOf(next, split)).toEqual(contentsOf(tabs, split));
    expect(next.inFront).not.toBe(split);
    expect(focusedChat(next)).toBe(3);
    expect(next.byId[next.inFront as number].name).toBe("steward 3");
  });

  it("a new chat in one pane opens a tab too, and never replaces the chat there", () => {
    const tabs = onePane();

    const { tabs: next } = openNew(tabs, pressed(3), pressing());

    expect(next.order).toHaveLength(2);
    expect(contentsOf(next, 1)).toEqual(contentsOf(tabs, 1));
    expect(focusedChat(next)).toBe(3);
  });

  it("a new chat fills an empty pane, and adds no tab", () => {
    const tabs = beside(onePane(), nothing);

    const { tabs: next } = openNew(tabs, pressed(3), pressing());

    expect(next.order).toEqual(tabs.order);
    expect(contentsOf(next, 1).map((one) => one.content)).toEqual([
      session(1),
      { kind: "session", session: 3, chat: "3" },
    ]);
    expect(focusedChat(next)).toBe(3);
  });

  it("a new chat of another workspace gets its tab on its own strip, and says which", () => {
    const { tabs: next, workspace } = openNew(onePane(), pressed(9), pressing());

    expect(workspace).toBe("web");
    expect(workspaceOf(next, next.inFront as number, filedIn)).toBe("web");
  });

  it("a press off screen in a split opens a tab, and the split survives", () => {
    const tabs = beside(onePane(), session(2));

    const { tabs: next } = press(tabs, pressed(3), pressing());

    expect(next.order).toHaveLength(2);
    expect(contentsOf(next, 1)).toEqual(contentsOf(tabs, 1));
    expect(focusedChat(next)).toBe(3);
  });

  it("a press off screen in a pinned split opens a tab, and the pin stays on the split", () => {
    const tabs = setPinned(beside(onePane(), session(2)), 1, true);

    const { tabs: next } = press(tabs, pressed(3), pressing());

    expect(next.order).toHaveLength(2);
    expect(contentsOf(next, 1)).toEqual(contentsOf(tabs, 1));
    expect(next.byId[1].pinned).toBe(true);
  });

  it("a press off screen in one pane shows there, and remembers the chat it replaced", () => {
    const { tabs: next } = press(onePane(), pressed(3), pressing());

    expect(next.order).toHaveLength(1);
    expect(focusedChat(next)).toBe(3);
    expect(next.byId[1].previous).toEqual({ [contentsOf(onePane(), 1)[0].pane]: 1 });
  });

  it("a press never replaces a one-job program, alone or in a split", () => {
    for (const shape of ["one pane, a one-job program", "split, a chat and a one-job program"]) {
      const tabs = SHAPES[shape]();
      const id = tabs.inFront as number;

      const { tabs: next } = press(tabs, pressed(3), pressing());

      expect(contentsOf(next, id)).toEqual(contentsOf(tabs, id));
      expect(focusedChat(next)).toBe(3);
    }
  });
});

/**
 * **A press that would move a window to another strip** (#5330): what a tab window hands to the
 * project window, since it holds only the tabs torn off into it.
 */
describe("a press that would leave the strip in front (#5330)", () => {
  it("for a chat in no pane, filed on another strip", () => {
    expect(wouldLeaveStrip(onePane(), 9, filedIn)).toBe(true);
  });

  it("not for a chat in no pane, filed on the strip in front", () => {
    expect(wouldLeaveStrip(onePane(), 2, filedIn)).toBe(false);
  });

  it("not for a chat on screen, even on another strip's tab", () => {
    const both = selectTab(openTab(onePane(), 9, "9", "steward"), 1);
    expect(wouldLeaveStrip(both, 9, filedIn)).toBe(false);
  });

  it("not with no tab in front", () => {
    expect(wouldLeaveStrip(noTabs(), 9, filedIn)).toBe(false);
  });
});
