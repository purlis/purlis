// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  catalogue,
  catalogued,
  FRESH_UNCHANGED,
  inPalette,
  OUTSIDE,
  type Catalogued,
  type Now,
  type Offer,
} from "./actions";
import type { ChatWorktree } from "./bindings";
import type { ListedChat } from "./chatsTree";
import { menuOf, rowsOf, type SubDrawn } from "./menuModel";
import { FOOT_FACTS, FOOT_ROWS, type FootPart } from "./paneHead";
import { noTabs, openTab, selectTab, splitFocusedPane } from "./tabs";

/**
 * **One action list per chat** (#4753, spec #4750, `command-table.md` on
 * `prototype/pane-chat-actions`). A chat's pane head's ⋯, its sidebar row's ⋯ and right-click,
 * and the palette all read one ordered list keyed by the chat. The pane's ⋯ and the row's menus
 * draw `menuOf({ on: "chat", session })` itself (`Menus.ChatMenuButton`, `ChatsSection`'s row),
 * so this holds that list against the palette for every kind of chat: what is on one is on the
 * other, and nothing a chat can do lives on one surface only.
 */

const chat = (session: number, more: Partial<ListedChat> = {}): ListedChat => ({
  session,
  name: `chat ${session}`,
  persona: "steward",
  workspace: "beta",
  shell: false,
  parent: null,
  mode: null,
  from: null,
  tab: false,
  branch: null,
  report: null,
  outcome: null,
  asking: null,
  harness: "claude",
  ...more,
});

/** Every kind of chat, by what the test calls it, and its number. */
const KINDS = {
  "a chat in its own tab": 1,
  "a chat in a split beside it": 2,
  "a chat with no tab": 3,
  "a task with no tab": 4,
  "a handoff with no tab": 5,
  "a shell": 6,
} as const;

/** The branch folder chat `session` works in, named by its number. */
const folderOf = (session: number): ChatWorktree => ({
  workspace: "beta",
  repo: "svc",
  piece: `fix-${session}`,
  branch: `fix/${session}`,
  wired: false,
  stale: false,
});

/** A window holding one chat of every kind, every row of a chat's list on, with or without a
 *  tab: twelve-persona Ask, the Skill studio, a pool to move to, a work link. */
function world(over: Partial<Now> = {}): { now: Now; offers: Catalogued; palette: Offer[] } {
  let tabs = openTab(noTabs(), 1, "1", "steward");
  tabs = splitFocusedPane(tabs, "row", 2, "chat 2");
  tabs = openTab(tabs, 6, "6", "sh");
  tabs = selectTab(tabs, tabs.order[0]);
  const listed = [
    chat(1, { tab: true }),
    chat(2, { tab: true }),
    chat(3),
    // A task asking the chat that dispatched it a question: its Answer row.
    chat(4, { mode: "task", parent: 1, from: "chat 1", asking: "Which branch?" }),
    chat(5, { mode: "handoff", parent: 1, from: "chat 1" }),
    chat(6, { tab: true, shell: true, persona: null, harness: null }),
  ];
  const names = new Map(listed.map((one) => [one.session, one.name]));
  const now: Now = {
    tabs,
    workspaces: [OUTSIDE, "beta"],
    focused: "beta",
    plane: "/plane",
    // Its state now, on every kind: chat 3 needs you, chat 1 wraps up (ADR 0064), and chat 5's
    // smart close stopped without its record (SI-8f).
    needsYou: [3],
    wrappingUp: [1],
    stopped: { 5: "it ended without writing its record" },
    nameOf: (session) => names.get(session) ?? String(session),
    listed,
    // qa is locked from chat 2 by policy: a row that cannot run, said on its list too.
    ask: {
      personas: ["devops", "qa"],
      locked: null,
      locked_for: [{ session: 2, persona: "qa", why: "Policy forbids it." }],
    },
    skillStudio: true,
    askable: (session) => session !== 6,
    restartable: (session) => session !== 6,
    linkable: () => true,
    movesTo: (session) => (session === 6 ? [] : ["builds"]),
    workItems: { 1: "PRJ-1" },
    // Every chat works in a branch folder purlis cut but the handoff, which works in its
    // clone: a shell's folder is its own as much as a harness's (#4851).
    worktreeOf: (session) => (session === 5 ? undefined : folderOf(session)),
    ...over,
  };
  const all = catalogue(now);
  return { now, offers: catalogued(all), palette: inPalette(all) };
}

/** The ids of a chat's list, its submenus' rows too, as its pane's ⋯ and its row draw them. */
function listOf(session: number, offers: Catalogued): string[] {
  return rowsOf(menuOf({ on: "chat", session }, offers)).map((row) => row.offer.id);
}

/** Whether a catalogue row is one of chat `session`'s: its id names that chat. */
function isChats(id: string, session: number): boolean {
  return new RegExp(`^(chat|needs|task)\\.[a-z.]+:${session}(:|$)`).test(id);
}

/**
 * **The rows of a chat the palette has and its list does not, and why each is not a row of the
 * list.** Each is what a press on the chat's own row already does, so a row of its list would
 * say only what that press does (the menus' rule 5). Any other row of a chat's that only the
 * palette has fails the test below.
 */
const THE_ROWS_PRESS: readonly RegExp[] = [
  // Show the task: the row's own press, kept in the palette as its keyboard way (#1499).
  /^chat\.show:/,
  // Show the chat that needs you: the same press, from the needs-you queue.
  /^needs\.show:/,
];

describe("a chat's one list (#4753)", () => {
  for (const [kind, session] of Object.entries(KINDS)) {
    describe(kind, () => {
      it("has every row of its list in the palette", () => {
        const { offers, palette } = world();
        const inThePalette = new Set(palette.map((offer) => offer.id));

        const list = listOf(session, offers);
        expect(list.length).toBeGreaterThan(0);
        expect(list.filter((id) => !inThePalette.has(id))).toEqual([]);
      });

      it("has every palette row of this chat on its list: none lives on one surface only", () => {
        const { offers, palette } = world();
        const list = new Set(listOf(session, offers));

        const paletteOnly = palette
          .filter((offer) => isChats(offer.id, session))
          .filter((offer) => !THE_ROWS_PRESS.some((press) => press.test(offer.id)))
          .map((offer) => offer.id)
          .filter((id) => !list.has(id));
        expect(paletteOnly).toEqual([]);
      });

      it("acts on this chat by its number, whatever surface presses it", () => {
        const { offers } = world();
        for (const row of rowsOf(menuOf({ on: "chat", session }, offers))) {
          // Go to names the chat this one's work went to, and goes there (#1492), and the chat
          // it came from (#5204).
          if (/^chat\.(handed|asker):/.test(row.offer.id)) continue;
          const does = row.offer.does as { session?: number; tab?: number };
          expect(does.tab, row.offer.id).toBeUndefined();
          if (does.session !== undefined) expect(does.session, row.offer.id).toBe(session);
        }
      });
    });
  }

  it("is the same list for a chat with a tab and one without", () => {
    const { offers } = world();
    // Less the rows for each chat's own state now, which this world gives each a different one.
    const state = /^(chat\.(smartclose\.cancel|answer)|needs\.(ignore|dismiss)):/;
    const shape = (session: number) =>
      listOf(session, offers)
        .filter((id) => !state.test(id))
        .map((id) => id.replace(`:${session}`, ":N"));

    // A chat with no tab has every row a chat in a tab has, but what is chat 1's own: the work
    // link it holds, the Go to of the handoff it made, and the links on its pane's screen
    // (#6332): a chat no pane draws has none.
    const own = (id: string) =>
      id.startsWith("chat.workunlink:") ||
      id.startsWith("chat.handed:") ||
      id.startsWith("chat.links:");
    expect(shape(3)).toEqual(shape(1).filter((id) => !own(id)));
    expect(shape(2).filter((id) => !id.startsWith("chat.links:"))).toEqual(shape(3));
  });

  describe("the pane foot's presses (#5204, spec #5191 stories 33 to 37)", () => {
    const card = {
      name: "claude",
      title: "Claude Code",
      label: "What Claude Code can do here",
      lines: [],
      cannot_type: null,
    };
    /** The window, with chat 5 (a handoff from chat 1) on a harness with a card, and started by
     *  the Skill studio. */
    const footed = (over: Partial<Now> = {}) =>
      world({
        cardOf: (session) => (session === 6 ? undefined : card),
        studioOf: (session) =>
          session === 5
            ? {
                skill: "test-design",
                level: { at: "project", local: false },
                job: "draft",
                done: false,
                started: null,
                finished: null,
              }
            : undefined,
        ...over,
      });

    it("is every press a row of the chat's list and of the palette: nothing lives on the foot only", () => {
      const { offers, palette } = footed();
      const inThePalette = new Set(palette.map((offer) => offer.id));
      const list = new Set(listOf(5, offers));
      for (const [part, rowOf] of Object.entries(FOOT_ROWS)) {
        expect(list, part).toContain(rowOf(5));
        expect(inThePalette, part).toContain(rowOf(5));
      }
    });

    it("gives every part of the foot a row, or says it is a fact with nothing to press", () => {
      const parts: FootPart[] = [
        "harness",
        "workspace",
        "from",
        "studio",
        "workItem",
        "skills",
        "skillsMark",
        "sandbox",
        "ended",
        "ctx",
        "cache",
      ];
      const pressed = new Set(Object.keys(FOOT_ROWS));
      const facts = new Set<string>(FOOT_FACTS);
      for (const part of parts) expect(pressed.has(part) !== facts.has(part), part).toBe(true);
    });

    it("goes to the chat that asked an ended task its pane still shows, while that chat is listed (#5366)", () => {
      // Task 7 of chat 1 has ended: the core lists it no more, and its pane keeps its last
      // screen with the path it last had, whose last but one is the chat that asked.
      const shown = splitFocusedPane(world().now.tabs, "column", 7, "fix-7");
      const { offers, palette } = footed({ tabs: shown, endedAskedBy: new Map([[7, 1]]) });
      expect(offers.get(FOOT_ROWS.from(7))?.does).toEqual({ verb: "showChat", session: 1 });
      expect(listOf(7, offers)).toContain(FOOT_ROWS.from(7));
      expect(palette.map((offer) => offer.id)).toContain(FOOT_ROWS.from(7));
      // Asked nothing of now, as a chat the core lists no more: the row is had all the same.
      const unasked = footed({
        tabs: shown,
        endedAskedBy: new Map([[7, 1]]),
        askable: (session) => session !== 6 && session !== 7,
      });
      expect(unasked.offers.get(FOOT_ROWS.from(7))?.does).toEqual({ verb: "showChat", session: 1 });
      // A chat the core lists again (a Resume) says its own lineage: listed with no asker, it
      // came from none, whatever the pane's last path said.
      const again = footed({ tabs: shown, endedAskedBy: new Map([[3, 1]]) });
      expect(again.offers.get(FOOT_ROWS.from(3))).toBeUndefined();
      // An asker that has closed has no row to go to: the foot says it in words.
      const closed = footed({ tabs: shown, endedAskedBy: new Map([[7, 9]]) });
      expect(closed.offers.get(FOOT_ROWS.from(7))).toBeUndefined();
    });

    it("goes where each part says", () => {
      const { offers } = footed();
      expect(offers.get(FOOT_ROWS.harness(5))?.does).toMatchObject({
        verb: "openView",
        view: { view: "harness" },
      });
      expect(offers.get(FOOT_ROWS.from(5))?.does).toEqual({ verb: "showChat", session: 1 });
      expect(offers.get(FOOT_ROWS.ctx(5))?.does).toMatchObject({ verb: "openView" });
      expect(offers.get(FOOT_ROWS.skills(5))?.does).toEqual({ verb: "showChatSkills", session: 5 });
      expect(offers.get(FOOT_ROWS.studio(5))?.does).toEqual({ verb: "openStudioChat", session: 5 });
    });

    it("focuses the chat's workspace from another, and says why not from its own", () => {
      const elsewhere = footed({ focused: OUTSIDE }).offers.get(FOOT_ROWS.workspace(5));
      expect(elsewhere).toMatchObject({
        available: true,
        does: { verb: "focusWorkspace", workspace: "beta" },
      });
      const here = footed().offers.get(FOOT_ROWS.workspace(5));
      expect(here).toMatchObject({ available: false, reason: "It is already focused." });
    });

    it("has a row only for what the chat has: no asker on a chat nobody started, no card or workspace on a shell", () => {
      const { offers } = footed();
      expect(offers.has(FOOT_ROWS.from(3))).toBe(false);
      expect(offers.has(FOOT_ROWS.studio(3))).toBe(false);
      for (const rowOf of Object.values(FOOT_ROWS).filter((row) => row !== FOOT_ROWS.ctx))
        expect(offers.has(rowOf(6)), rowOf(6)).toBe(false);
    });

    it("has no asker's row once that chat has closed, since there is nowhere to go", () => {
      const listed = world().now.listed?.filter((one) => one.session !== 1);
      expect(footed({ listed }).offers.has(FOOT_ROWS.from(5))).toBe(false);
    });

    it("has no Skills row with the Skill studio off, as it has no Use skill", () => {
      expect(footed({ skillStudio: false }).offers.has(FOOT_ROWS.skills(5))).toBe(false);
    });
  });

  it("says End chat in the palette only on End…: a tab's close says Close (#4753 D6, #4756)", () => {
    // Until #4756 the palette had `End chat X` (the tab's close, which ended the chat) beside
    // `End chat X…` (the chat's End…). A close ends nothing now, so only End… says End.
    const { palette } = world();
    const ends = palette.filter((offer) => offer.title.startsWith("End chat "));

    expect(ends.length).toBeGreaterThan(0);
    expect(ends.filter((offer) => !offer.id.startsWith("chat.end:"))).toEqual([]);
    expect(
      palette.filter((offer) => offer.id.startsWith("tab.close:")).map((o) => o.title),
    ).toEqual(expect.arrayContaining([expect.stringMatching(/^Close /)]));
  });

  it("names the chat in the palette's words, and leaves it out of the menu's", () => {
    const { offers, palette } = world();
    const rows = rowsOf(menuOf({ on: "chat", session: 3 }, offers));
    const titles = new Map(palette.map((offer) => [offer.id, offer.title]));

    for (const row of rows) {
      expect(row.said, row.offer.id).not.toContain("chat 3");
      expect(titles.get(row.offer.id), row.offer.id).toContain("chat 3");
    }
  });

  it("leaves out a row its kind cannot mean: a shell has no harness rows, a task no End…", () => {
    const { offers } = world();
    const kinds = (session: number) =>
      new Set(listOf(session, offers).map((id) => id.slice(0, id.indexOf(":"))));

    for (const harness of ["chat.useskill", "chat.ask", "chat.restart", "chat.fresh", "chat.move"])
      expect(kinds(6), harness).not.toContain(harness);
    // A shell keeps what is not the harness's: its name, its reads, its end.
    for (const kept of ["chat.rename", "chat.activity", "chat.network", "chat.end"])
      expect(kinds(6), kept).toContain(kept);
    // A task ends by Stop and get its report or Close now, never End…; any other chat by End….
    expect(kinds(4)).not.toContain("chat.end");
    expect(kinds(4)).toContain("task.stop");
    for (const session of [1, 2, 3, 5]) expect(kinds(session)).toContain("chat.end");
  });

  it("greys an Ask policy locks, with policy's sentence, inside Ask ▸ on the list", () => {
    const { offers } = world();
    const qa = rowsOf(menuOf({ on: "chat", session: 2 }, offers)).find(
      (row) => row.offer.id === "chat.ask:2:qa",
    );
    expect(qa?.offer).toMatchObject({ available: false, reason: "Policy forbids it." });
  });

  it("greys a row that cannot run now, with its reason, and keeps it on the list", () => {
    const quiet = world();
    const fresh = rowsOf(menuOf({ on: "chat", session: 3 }, quiet.offers)).find(
      (row) => row.offer.id === "chat.fresh:3",
    );
    expect(fresh?.offer).toMatchObject({ available: false, reason: FRESH_UNCHANGED });

    const changed = world({ planeUpdated: { 3: ["CLAUDE.md"] } });
    expect(changed.offers.get("chat.fresh:3")?.available).toBe(true);
    // And End… while the chat still restores: no program yet to end, said why.
    const restoring = world({ restoring: [3] });
    expect(restoring.offers.get("chat.end:3")?.available).toBe(false);
    expect(listOf(3, restoring.offers)).toContain("chat.end:3");
  });

  it("has only its reading rows for a chat ended on purpose that its pane still shows (#5019)", () => {
    // Its pane keeps it under the ended band, whose Resume and Clear pane are its ways out: no
    // End… to press a second time, nothing to rename, restart or stop. What it said and what it
    // reached can still be read.
    const { palette } = world({ endedHere: [1] });
    expect(
      palette
        .filter((offer) => offer.id.startsWith("chat.") && offer.id.endsWith(":1"))
        .map((offer) => offer.id),
    ).toEqual(["chat.activity:1", "chat.network:1"]);
    expect(palette.some((offer) => offer.id === "chat.end:2")).toBe(true);
  });

  it("lists every chat's rows in the palette, not only the chat in front's", () => {
    const { palette } = world();
    const rename = palette.filter((offer) => offer.id.startsWith("chat.rename:"));
    // The panes' chats in the strip's order, then the chats no pane shows.
    expect(rename.map((offer) => offer.does)).toEqual(
      [1, 2, 6, 3, 4, 5].map((session) => ({ verb: "renameChat", session })),
    );
    expect(palette.filter((offer) => offer.menuOnly === true && isChats(offer.id, 3))).toEqual([]);
  });

  describe("its Branch rows (#4851, command-table.md rows 24 and 25)", () => {
    it("are on the list of every kind of chat in a branch folder, a shell's and a task's too", () => {
      const { offers } = world();
      for (const session of [1, 2, 3, 4, 6]) {
        expect(listOf(session, offers), `chat ${session}`).toEqual(
          expect.arrayContaining([`chat.branch.merge:${session}`, `chat.branch.remove:${session}`]),
        );
      }
    });

    it("are not on the list of a chat that works in no branch folder", () => {
      const { offers } = world();
      expect(listOf(5, offers).filter((id) => id.startsWith("chat.branch."))).toEqual([]);
    });

    it("say the table's words: the chat's name in the palette, the verb alone under Branch", () => {
      const { offers, palette } = world();
      const titles = new Map(palette.map((offer) => [offer.id, offer.title]));
      expect(titles.get("chat.branch.merge:3")).toBe("Merge chat 3's branch into its clone");
      expect(titles.get("chat.branch.remove:3")).toBe("Remove chat 3's branch folder");

      const branch = menuOf({ on: "chat", session: 3 }, offers)
        .groups.flatMap((group) => group.entries)
        .find((entry): entry is SubDrawn => entry.kind === "sub" && entry.key === "branch");
      expect(branch?.said).toBe("Branch");
      expect(branch === undefined ? [] : rowsOf(branch).map((row) => row.said)).toEqual([
        "Merge its branch",
        "Remove its branch folder",
      ]);
    });

    it("act on that chat's own folder, never the chat in front's", () => {
      const { offers } = world();
      expect(offers.get("chat.branch.merge:3")?.does).toEqual({
        verb: "mergeWorktree",
        cut: { workspace: "beta", repo: "svc", piece: "fix-3", branch: "fix/3" },
      });
      expect(offers.get("chat.branch.remove:3")?.does).toEqual({
        verb: "removeWorktree",
        cut: { workspace: "beta", repo: "svc", piece: "fix-3", branch: "fix/3" },
        force: false,
      });
    });

    it("stand after Stop and before End… (the table's rule 3)", () => {
      const { offers } = world();
      const list = listOf(1, offers);
      expect(list.indexOf("chat.stop:1")).toBeLessThan(list.indexOf("chat.branch.merge:1"));
      expect(list.indexOf("chat.branch.remove:1")).toBeLessThan(list.indexOf("chat.end:1"));
    });

    it("are greyed with the reason where purlis has no project to reach the branch in", () => {
      const { offers } = world({ plane: undefined });
      for (const id of ["chat.branch.merge:3", "chat.branch.remove:3"]) {
        expect(offers.get(id), id).toMatchObject({ available: false });
        expect(offers.get(id)?.reason, id).toMatch(/no project/);
        expect(listOf(3, offers)).toContain(id);
      }
    });

    it("offer the discard only on the chat whose removal was refused", () => {
      expect(world().offers.get("chat.branch.discard:3")).toBeUndefined();

      const { offers } = world({ refused: "chat.branch.remove:3" });
      expect(offers.get("chat.branch.discard:3")?.does).toEqual({
        verb: "removeWorktree",
        cut: { workspace: "beta", repo: "svc", piece: "fix-3", branch: "fix/3" },
        force: true,
      });
      expect(listOf(3, offers)).toContain("chat.branch.discard:3");
      expect(offers.get("chat.branch.discard:1")).toBeUndefined();
    });
  });
});
