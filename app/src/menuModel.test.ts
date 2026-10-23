// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  askRows,
  catalogue,
  catalogued,
  ENDS_IT,
  memoryOffers,
  menuRows,
  sessionOffers,
  OUTSIDE,
  type Catalogued,
  type MenuOn,
  type Now,
} from "./actions";
import type { ListedChat } from "./chatsTree";
import {
  DRESSED,
  menuIsEmpty,
  menuOf,
  NAMES_ANOTHER,
  rowsOf,
  stemOf,
  unnamed,
  type EntryDrawn,
  type MenuDrawn,
} from "./menuModel";
import { memoryKey } from "./memories";
import { SAID, SUBMENU, WARNS } from "./menuWords";
import { terminalGroup, terminalRows } from "./terminalRows";
import { GRANT_WORDS } from "../eslint.ui-rules.js";
import { noTabs, openTab, openView, selectTab } from "./tabs";

/**
 * **How every context menu is drawn** (`menuModel.ts`; `docs/ui-primitives.md`, "Menus").
 *
 * The rules are held over EVERY menu at once, from one window that has one of everything, so
 * the next menu, or the next row of one, is held to them without a test of its own.
 */

/** Twelve personas, as the project in the report had: one top-level row each ran the chat tab's
 *  menu off the window. */
const PERSONAS = [
  "authz-master",
  "backend-developer",
  "console-developer",
  "devops",
  "keycloak-master",
  "marketing",
  "product-owner",
  "qa",
  "steward",
  "trading-master",
  "ux-writer",
  "zookeeper",
];

const chat = (session: number, more: Partial<ListedChat> = {}): ListedChat => ({
  session,
  name: `chat ${session}`,
  persona: null,
  workspace: "beta",
  shell: false,
  parent: null,
  mode: null,
  from: null,
  tab: true,
  branch: null,
  report: null,
  outcome: null,
  asking: null,
  harness: "claude",
  ...more,
});

const CUT = { workspace: "beta", repo: "svc", piece: "fix-it", branch: "fix-it" };
const RECORD = "workspaces/beta/sessions/ship-the-widget.md";
const MEMORY = { scope: { kind: "shared" as const }, slug: "a-lesson" };
/** A finished task that worked on a branch purlis cut for it: all its menu reads of it. */
const FINISHED = {
  id: "d-1",
  name: "lint",
  branch: "task/lint",
  waits: null,
} as unknown as NonNullable<Now["finished"]> extends ReadonlyMap<number, readonly (infer T)[]>
  ? T
  : never;

/** A window with one of everything a menu can be opened on. Tab 1 is a chat behind, tab 2 the
 *  chat in front, tab 3 a view's tab. */
function world(over: Partial<Now> = {}): { now: Now; offers: Catalogued; tabs: Now["tabs"] } {
  let tabs = openTab(noTabs(), 7, "16", "console-developer");
  tabs = openTab(tabs, 9, "1", "smart-ide");
  const [behind, front] = tabs.order;
  tabs = selectTab(tabs, front);
  const names: Record<number, string> = {
    7: tabs.byId[behind].name,
    9: tabs.byId[front].name,
    11: "devops 11",
    12: "qa 12",
  };
  const now: Now = {
    tabs,
    workspaces: [OUTSIDE, "alpha", "beta"],
    focused: "beta",
    plane: "/plane",
    needsYou: [7],
    nameOf: (session) => names[session] ?? String(session),
    ask: { personas: PERSONAS, locked: null },
    askable: () => true,
    restartable: () => true,
    linkable: () => true,
    workItems: { 7: "PRJ-1" },
    listed: [
      chat(7, { name: names[7] }),
      chat(9, { name: names[9] }),
      chat(11, { name: names[11], mode: "task", parent: 7, from: names[7], tab: false }),
      chat(12, { name: names[12], mode: "handoff", parent: 7, from: names[7], tab: false }),
    ],
    pinned: { chats: [], workspaces: ["alpha"], projects: [] },
    projects: [
      { plane: "/plane", name: "charter" },
      { plane: "/other", name: "umbrella" },
    ],
    pieces: [CUT],
    clones: [{ repo: "svc", path: "/plane/workspaces/beta/svc" }],
    absent: ["lib"],
    personas: ["devops"],
    vaults: ["forge"],
    todos: [{ slug: "ship-it", title: "Ship it" }],
    memoryStores: [{ kind: "shared" }, { kind: "workspace", name: "beta" }],
    // A read of beta stands refused, and a task of chat 7 finished on a branch of its own.
    readRefused: true,
    finished: new Map([[7, [FINISHED]]]),
    ...over,
  };
  // And the rows a panel adds for what it lists: a session record's, and a memory's.
  const offers = catalogued([
    ...catalogue(now),
    ...sessionOffers([{ path: RECORD, title: "Ship the widget", resumable: true }]),
    ...memoryOffers(MEMORY, "A lesson", now.memoryStores),
  ]);
  return { now, offers, tabs };
}

/** Every thing the world's menus are opened on, by what the test calls it. */
function everyMenu(): { said: string; on: MenuOn; offers: Catalogued }[] {
  const { offers, tabs } = world();
  const [behind, front] = tabs.order;
  return [
    { said: "a chat's tab behind", on: { on: "chat", tab: behind, session: 7 }, offers },
    { said: "the chat's tab in front", on: { on: "chat", tab: front, session: 9 }, offers },
    { said: "a chat's row", on: { on: "listed", session: 7, handed: [12] }, offers },
    { said: "a task's row", on: { on: "listed", session: 11 }, offers },
    { said: "a workspace's tab", on: { on: "workspace", workspace: "alpha" }, offers },
    { said: "the focused workspace's tab", on: { on: "workspace", workspace: "beta" }, offers },
    { said: "the plane root's tab", on: { on: "root" }, offers },
    { said: "a project's tab", on: { on: "project", plane: "/other" }, offers },
    { said: "the project in front", on: { on: "project", plane: "/plane" }, offers },
    { said: "a branch's folder", on: { on: "worktree", repo: "svc", piece: "fix-it" }, offers },
    { said: "a clone", on: { on: "clone", repo: "svc" }, offers },
    { said: "a repo not cloned", on: { on: "absent", repo: "lib" }, offers },
    { said: "a persona", on: { on: "persona", persona: "devops" }, offers },
    { said: "a vault", on: { on: "vault", vault: "forge" }, offers },
    { said: "a todo", on: { on: "todo", slug: "ship-it" }, offers },
    { said: "the panes", on: { on: "pane" }, offers },
    {
      said: "a file",
      on: {
        on: "file",
        kind: "file",
        at: { workspace: "beta", repo: "svc", piece: "fix-it", path: "src/main.rs" },
      },
      offers,
    },
    { said: "a view's icon", on: { on: "view", view: "explorer", to: "panel" }, offers },
    { said: "a session record", on: { on: "session", path: RECORD }, offers },
    { said: "a memory", on: { on: "memory", key: memoryKey(MEMORY) }, offers },
    { said: "a finished task", on: { on: "finished", id: "d-1" }, offers },
    { said: "a refused read", on: { on: "refusal" }, offers },
  ];
}

/** A menu as its rows' words: a group per line, a submenu as `Words ▸ [its rows]`. */
function said(menu: MenuDrawn): string[][] {
  const one = (entry: EntryDrawn): string =>
    entry.kind === "row"
      ? entry.said
      : `${entry.said} ▸ [${entry.groups.flatMap((group) => group.entries.map(one)).join(", ")}]`;
  return menu.groups.map((group) => group.entries.map(one));
}

describe("a row's words", () => {
  it("are its title with the name it carries taken out", () => {
    const row = (title: string, name: string) =>
      unnamed({ id: "x", title, name, available: true, reason: "", does: { verb: "nothing" } });

    expect(row("End chat 3 steward", "3 steward")).toBe("End");
    expect(row("Close notes", "notes")).toBe("Close");
    expect(row("Close notes and end the chat beside it", "notes")).toBe(
      "Close and end the chat beside it",
    );
    expect(row("Stop chat steward 4 and everything below it", "steward 4")).toBe(
      "Stop and everything below it",
    );
    expect(row("Send devops 2 back into steward 4's tab", "devops 2")).toBe(
      "Send back into steward 4's tab",
    );
    expect(row("Make alpha live…", "alpha")).toBe("Make live…");
    // A name with a pattern's own characters in it is read as a name.
    expect(row("End chat a.b (1)", "a.b (1)")).toBe("End");
    // A chat a person named for a row's own verb, or for its kind, is still taken out whole.
    expect(row("End chat End", "End")).toBe("End");
    expect(row("Close Close", "Close")).toBe("Close");
    expect(row("Stop chat chat and everything below it", "chat")).toBe(
      "Stop and everything below it",
    );
    expect(row("Make live live…", "live")).toBe("Make live…");
  });
});

describe("every menu's words, as a row says them at run time", () => {
  it("never grant: a grant is a button with its lock, never a menu row (ADR 0097 §6)", () => {
    const words = new Set<string>();
    const walk = (entries: readonly EntryDrawn[]) => {
      for (const entry of entries) {
        words.add(entry.said);
        if (entry.kind === "row") {
          words.add(entry.offer.title);
          if (entry.midTurn !== undefined) words.add(entry.midTurn);
        } else for (const group of entry.groups) walk(group.entries);
      }
    };
    for (const { on, offers } of everyMenu())
      for (const group of menuOf(on, offers).groups) walk(group.entries);
    for (const mac of [true, false])
      for (const selected of [true, false])
        walk(terminalGroup(terminalRows({ selected, mac })).entries);
    const fixed = [SAID, SUBMENU].flatMap((table) =>
      Object.values(table).filter((one): one is string => typeof one === "string"),
    );
    expect(words.size).toBeGreaterThan(50);
    expect([...words, ...fixed].filter((one) => GRANT_WORDS.test(one))).toEqual([]);
    // And the check is the lint's, which finds a grant anywhere in a label.
    expect(GRANT_WORDS.test("Always allow")).toBe(true);
  });
});

describe("every menu", () => {
  it("draws every row the catalogue lists for it, once, and no other", () => {
    for (const { said: which, on, offers } of everyMenu()) {
      const listed = menuRows(on, offers);
      const drawn = rowsOf(menuOf(on, offers)).map((row) => row.offer.id);
      // The one kind of row left out: a row that cannot run and would say only what a press
      // does (rule 5). Everything else the catalogue listed is drawn.
      const expected = [...listed.above, ...listed.below]
        .filter((offer) => offer.available || !QUIET.has(stemOf(offer.id)))
        .map((offer) => offer.id);
      const own = drawn.filter((id) => !GROWN.test(id));
      expect([...own].sort(), which).toEqual([...expected].sort());
      expect(new Set(drawn).size, which).toBe(drawn.length);
    }
  });

  it("has words of its own for every row it lists, but the ones that name nothing", () => {
    const bare = new Set<string>();
    for (const { on, offers } of everyMenu())
      for (const row of rowsOf(menuOf(on, offers)))
        if (!DRESSED.has(stemOf(row.offer.id)) && !GROWN.test(row.offer.id))
          bare.add(stemOf(row.offer.id));

    // A view's icon has the one row, built for the icon; it is drawn by its catalogue title.
    expect([...bare].sort()).toEqual(["view.move"]);
  });

  it("says the name of the thing it is about once, as its own name, and in no row", () => {
    const named: string[] = [];
    for (const { said: which, on, offers } of everyMenu()) {
      const menu = menuOf(on, offers);
      if (menu.name === undefined) continue;
      named.push(which);
      expect(menu.about, which).toContain(menu.name);
      for (const row of rowsOf(menu)) {
        // The exceptions, listed: a row that names ANOTHER thing than the one clicked.
        if (NAMES_ANOTHER.has(stemOf(row.offer.id))) continue;
        expect(row.said, `${which}: ${row.offer.title}`).not.toContain(menu.name);
        expect(row.midTurn ?? "", `${which}: ${row.offer.title}`).not.toContain(menu.name);
      }
    }
    // Every menu opened on a thing with a name is held to it.
    expect(named).toEqual([
      "a chat's tab behind",
      "the chat's tab in front",
      "a chat's row",
      "a task's row",
      "a workspace's tab",
      "the focused workspace's tab",
      "a project's tab",
      "the project in front",
      "a branch's folder",
      "a clone",
      "a repo not cloned",
      "a persona",
      "a vault",
      "a todo",
      "a file",
      "a session record",
      "a memory",
    ]);
  });

  it("lets a row name another thing only where it acts on that thing", () => {
    expect([...NAMES_ANOTHER].sort()).toEqual(
      ["chat.back", "chat.beside", "chat.handed", "tab.close", "worktree.merge"].sort(),
    );
    const { offers } = world();
    const rows = rowsOf(menuOf({ on: "listed", session: 7, handed: [12] }, offers));
    // The chat its work was handed to is another chat, and the row goes to it.
    expect(rows.map((row) => row.said)).toContain("Go to qa 12");
    const branch = rowsOf(menuOf({ on: "worktree", repo: "svc", piece: "fix-it" }, offers));
    expect(branch.map((row) => row.said)).toContain("Merge into svc");
  });

  it("gives a row an ellipsis exactly when the catalogue's title has one", () => {
    for (const { said: which, on, offers } of everyMenu())
      for (const row of rowsOf(menuOf(on, offers)))
        expect(row.said.endsWith("…"), `${which}: ${row.said} / ${row.offer.title}`).toBe(
          row.offer.title.endsWith("…"),
        );
  });

  it("has at most four groups, and what cannot be undone in the last, a submenu's rows too", () => {
    let folded = 0;
    for (const { said: which, on, offers } of everyMenu()) {
      const menu = menuOf(on, offers);
      expect(menu.groups.length, which).toBeLessThanOrEqual(4);
      // Every row of every group but the last, through the submenus a group's rows open.
      for (const group of menu.groups.slice(0, -1))
        for (const row of rowsOf({ ...menu, groups: [group] })) {
          expect(row.danger, `${which}: ${row.said}`).toBe(false);
        }
      // And in the last group a submenu of such rows stands above the rows themselves, so the
      // menu ends with the row that ends the thing.
      const last = menu.groups.at(-1)?.entries ?? [];
      const sub = last.findIndex((entry) => entry.kind === "sub");
      if (sub === -1) continue;
      folded += 1;
      expect(sub, which).toBe(0);
      expect(
        last.slice(1).every((entry) => entry.kind === "row"),
        which,
      ).toBe(true);
    }
    // The ways to stop a chat are such a submenu, on its tab and on its row.
    expect(folded).toBeGreaterThanOrEqual(3);
  });

  it("colours only what cannot be undone: a program ended, a thing deleted", () => {
    const danger = new Set<string>();
    const plain = new Set<string>();
    for (const { on, offers } of everyMenu())
      for (const row of rowsOf(menuOf(on, offers)))
        (row.danger ? danger : plain).add(stemOf(row.offer.id));

    expect([...danger].sort()).toEqual(
      [
        // Ends the program a chat runs, or every chat of a project.
        "tab.close",
        "pane.close",
        "project.close",
        "chat.stop",
        "chat.stop.below",
        "chat.stop.tasks",
        "task.stop",
        "task.close",
        // Deletes what purlis cannot give back.
        "workspace.remove",
        "persona.remove",
        "vault.remove",
        "todo.forget",
        // Removes a finished task's branch folder. It asks first.
        "task.discard",
      ].sort(),
    );
    // Restart keeps the conversation; a branch's folder goes and its branch stays; a repo
    // taken out of a workspace is deleted nowhere.
    for (const stem of ["tab.restart", "chat.restart", "worktree.remove", "absent.drop"])
      expect(plain, stem).toContain(stem);
  });

  it("says what a row that cannot be undone costs under it, on such a row only, and no more than its note says", () => {
    for (const { said: which, on, offers } of everyMenu())
      for (const row of rowsOf(menuOf(on, offers))) {
        if (row.warns === undefined) continue;
        expect(row.danger, `${which}: ${row.said}`).toBe(true);
        expect(row.offer.available, `${which}: ${row.said}`).toBe(true);
        // The whole of it is the catalogue's note, which is the row's tooltip.
        expect(row.offer.note, `${which}: ${row.said}`).toBeTruthy();
      }
    // That it is never cut is the stylesheet's, held in `primitives/controls.test.tsx`.
    // The catalogue's own sentence, where it is short enough.
    expect(WARNS.ends).toBe(ENDS_IT);
    // Only a keychain vault's secrets are destroyed (the catalogue's note): the warning says
    // which, and claims nothing of a vault kept elsewhere.
    expect(WARNS.deletesVault).toMatch(/^A keychain vault's secrets/);
    // Stop all tasks keeps the session, and says so.
    expect(WARNS.stopsTasks).toContain("The chat keeps running.");
  });

  it("marks a group named by data, which wears no capitals: a persona's group under Curate", () => {
    const subject = {
      subject: "workspace:alpha",
      name: "alpha",
      actions: [
        {
          id: "devops/tidy",
          label: "Tidy",
          declared_by: "devops",
          runner: "devops",
          cwd: "/plane/workspaces/alpha",
          prompt: "Tidy alpha.",
        },
      ],
      left_out: [],
      trouble: null,
    };
    const { offers } = world({ curations: { subjects: [subject], cannot: null } });
    const curate = menuOf({ on: "workspace", workspace: "alpha" }, offers)
      .groups.flatMap((group) => group.entries)
      .find((entry) => entry.kind === "sub" && entry.key === "curate");
    if (curate?.kind !== "sub") throw new Error("no Curate submenu");

    expect(curate.groups.map((group) => [group.label, group.named])).toEqual([["devops", true]]);
  });
});

describe("a chat tab's menu", () => {
  const { offers, tabs } = world();
  const [behind, front] = tabs.order;

  it("is nine rows in four groups with twelve personas, and Ask is one of them", () => {
    const menu = menuOf({ on: "chat", tab: behind, session: 7 }, offers);

    expect(said(menu)).toEqual([
      ["Switch to", "Rename…", "Pinned", "Link to work item…", "Unlink work item"],
      [`Open ▸ [Activity, Network]`, `Ask ▸ [${PERSONAS.map((one) => `${one}…`).join(", ")}]`],
      ["Restart"],
      // The ways to stop it end something too: in the last group, above the row that ends it.
      ["Stop ▸ [Stop, Stop and everything below it, Stop all tasks]", "End"],
    ]);
    const top = menu.groups.flatMap((group) => group.entries);
    expect(top.length).toBeLessThanOrEqual(10);
    // With no persona at all there is no Ask row, and with fifty there is still one.
    const many = world({ ask: { personas: [...Array(50)].map((_, i) => `p${i}`), locked: null } });
    expect(
      menuOf({ on: "chat", tab: behind, session: 7 }, many.offers).groups.flatMap(
        (group) => group.entries,
      ).length,
    ).toBe(top.length);
    const none = world({ ask: { personas: [], locked: null } });
    expect(said(menuOf({ on: "chat", tab: behind, session: 7 }, none.offers))[1]).toEqual([
      "Open ▸ [Activity, Network]",
    ]);
  });

  it("names a persona's row by the persona alone, and says what asking does once", () => {
    const menu = menuOf({ on: "chat", tab: behind, session: 7 }, offers);
    const ask = menu.groups
      .flatMap((group) => group.entries)
      .find((entry) => entry.kind === "sub" && entry.key === "ask");
    if (ask?.kind !== "sub") throw new Error("no Ask submenu");

    expect(ask.tip).toMatch(/^A task starts as the persona you pick/);
    const rows = rowsOf(ask);
    expect(rows.map((row) => row.offer.id)).toEqual(
      askRows(behind, offers).map((offer) => offer.id),
    );
    for (const row of rows) {
      expect(row.untipped).toBe(true);
      expect(row.said).not.toMatch(/^Ask /);
    }
  });

  it("has no Switch to row on the tab in front, and it is first on any other", () => {
    expect(said(menuOf({ on: "chat", tab: behind, session: 7 }, offers))[0][0]).toBe(SAID.switchTo);
    const inFront = rowsOf(menuOf({ on: "chat", tab: front, session: 9 }, offers));
    expect(inFront.map((row) => row.said)).not.toContain(SAID.switchTo);
    expect(inFront[0].said).toBe(SAID.rename);
  });

  it("draws Pinned as one row with a tick while it is on", () => {
    const pin = (from: Catalogued) =>
      rowsOf(menuOf({ on: "chat", tab: behind, session: 7 }, from)).find(
        (row) => row.said === SAID.pinned,
      );

    expect(pin(offers)?.checked).toBe(false);
    const held = world({ pinned: { chats: [7], workspaces: [], projects: [] } });
    expect(pin(held.offers)?.checked).toBe(true);
    // The catalogue's row is the one that would unpin it.
    expect(pin(held.offers)?.offer.title).toMatch(/^Unpin /);
  });

  it("says the key that does the same from the tab, at the row's end", () => {
    const rows = rowsOf(menuOf({ on: "chat", tab: behind, session: 7 }, offers));
    const keys = Object.fromEntries(
      rows.filter((row) => row.keys !== undefined).map((row) => [row.said, row.keys]),
    );

    expect(keys).toEqual({ "Rename…": "F2", End: expect.stringMatching(/^(⌫|Delete)$/) });
  });

  it("says Restart waits for the turn while its chat is mid-turn, and names no chat doing it", () => {
    const restart = rowsOf(menuOf({ on: "chat", tab: behind, session: 7 }, offers)).find(
      (row) => row.said === SAID.restart,
    );

    expect(restart?.midTurn).toBe("Restart when this turn ends");
    expect(restart?.danger).toBe(false);
  });

  it("colours Start fresh, which starts again with no conversation, and puts it last with End", () => {
    const changed = world({ planeUpdated: { 7: ["CLAUDE.md"] } });
    const menu = menuOf({ on: "chat", tab: behind, session: 7 }, changed.offers);

    expect(said(menu).at(-1)).toEqual([
      "Stop ▸ [Stop, Stop and everything below it, Stop all tasks]",
      "Start fresh",
      "End",
    ]);
    const fresh = rowsOf(menu).find((row) => row.said === SAID.startFresh);
    expect(fresh?.danger).toBe(true);
    expect(fresh?.warns).toBe(WARNS.fresh);
  });

  it("on a view's tab is Pinned and Close, and Close is not coloured", () => {
    const withView = openView(tabs, { from: null, view: "vault", key: "forge" }, "forge", "beta");
    const view = withView.order[withView.order.length - 1];
    const menu = menuOf(
      { on: "chat", tab: view },
      catalogued(catalogue({ ...world().now, tabs: withView })),
    );
    const rows = rowsOf(menu);

    expect(rows.map((row) => [row.said, row.danger])).toEqual([
      ["Pinned", false],
      ["Close", false],
    ]);
  });
});

describe("the other tabs' menus", () => {
  const { offers } = world();

  it("a workspace's tab", () => {
    expect(said(menuOf({ on: "workspace", workspace: "alpha" }, offers))).toEqual([
      ["Focus", "Pinned", "Settings…", "Make live…", "Rename…"],
      ["New shell", "New workspace…"],
      ["Delete"],
    ]);
    // The focused one has no Focus row.
    expect(said(menuOf({ on: "workspace", workspace: "beta" }, offers))[0][0]).toBe("Pinned");
  });

  it("the plane root's tab", () => {
    expect(said(menuOf({ on: "root" }, offers))).toEqual([
      ["Focus"],
      ["New chat", "New shell", "New workspace…"],
    ]);
  });

  it("a project's tab", () => {
    expect(said(menuOf({ on: "project", plane: "/other" }, offers))).toEqual([
      ["Switch to", "Pinned", "Settings…", "Saving…", "Move to a new window"],
      ["New project…", "Open a project…"],
      ["Close"],
    ]);
    expect(said(menuOf({ on: "project", plane: "/plane" }, offers))[0][0]).toBe("Pinned");
  });

  it("a task's row in the Chats list", () => {
    expect(said(menuOf({ on: "listed", session: 11 }, offers))).toEqual([
      ["Brief", "Move to its own tab", "Open beside console-developer 16"],
      ["Restart"],
      ["Stop and get its report", "Close now"],
    ]);
  });

  it("the panes keep the shell's key and the catalogue's words", () => {
    const rows = rowsOf(menuOf({ on: "pane" }, offers));

    expect(said(menuOf({ on: "pane" }, offers))).toEqual([
      ["New tab", "New shell"],
      ["Split right", "Split down", "Send F2 to the chat in front"],
      ["End this pane's chat"],
    ]);
    expect(rows.find((row) => row.said === "New shell")?.keys).toMatch(/^(⌘⇧T|Ctrl\+Shift\+T)$/);
    // The same words on a workspace's tab open a shell in that workspace, which has no key.
    expect(
      rowsOf(menuOf({ on: "workspace", workspace: "alpha" }, offers)).find(
        (row) => row.said === "New shell",
      )?.keys,
    ).toBeUndefined();
  });
});

describe("a memory's menu", () => {
  it("opens and edits it, moves it from one row, and deletes it under the line uncoloured", () => {
    const { offers } = world();
    const menu = menuOf({ on: "memory", key: memoryKey(MEMORY) }, offers);

    expect(said(menu)).toEqual([["Open", "Edit"], ["Move to ▸ [beta — workspace]"], ["Delete"]]);
    expect(menu.about).toBe("Memory A lesson");
    // An archive with an Undo (ADR 0065): last and under the line, and not the danger colour.
    expect(rowsOf(menu).map((row) => row.danger)).toEqual([false, false, false, false]);
  });
});

describe("a menu with nothing to draw", () => {
  it("is empty when its only row could not run and would only say what a press does", () => {
    const { offers } = world({ workspaces: ["beta"], plane: undefined, focused: "beta" });
    const only = new Map([...offers].filter(([id]) => id === "workspace.focus:beta"));

    expect(menuIsEmpty({ on: "vault", vault: "forge" }, only)).toBe(true);
    expect(menuIsEmpty({ on: "todo", slug: "none" }, offers)).toBe(true);
    expect(menuIsEmpty({ on: "pane" }, offers)).toBe(false);
  });
});

/** The rows of the submenus the catalogue decides the length of (Ask, Move to, Curate), which
 *  `menuRows` does not list and their submenu names. */
const GROWN = /^(tab\.ask|memory\.move|curate)[:.]/;

/** The rows left out while they cannot run. */
const QUIET = new Set(["tab.select", "workspace.focus", "project.select"]);
