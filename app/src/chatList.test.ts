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
import type { ListedChat } from "./chatsTree";
import { menuOf, rowsOf } from "./menuModel";
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
          // Go to names the chat this one's work went to, and goes there (#1492).
          if (row.offer.id.startsWith("chat.handed:")) continue;
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
    // link it holds, and the Go to of the handoff it made.
    const own = (id: string) => id.startsWith("chat.workunlink:") || id.startsWith("chat.handed:");
    expect(shape(3)).toEqual(shape(1).filter((id) => !own(id)));
    expect(shape(2)).toEqual(shape(3));
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

  it("lists every chat's rows in the palette, not only the chat in front's", () => {
    const { palette } = world();
    const rename = palette.filter((offer) => offer.id.startsWith("chat.rename:"));
    // The panes' chats in the strip's order, then the chats no pane shows.
    expect(rename.map((offer) => offer.does)).toEqual(
      [1, 2, 6, 3, 4, 5].map((session) => ({ verb: "renameChat", session })),
    );
    expect(palette.filter((offer) => offer.menuOnly === true && isChats(offer.id, 3))).toEqual([]);
  });
});
