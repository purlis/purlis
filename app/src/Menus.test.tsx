// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { askId, catalogue, catalogued, menuRows, OUTSIDE, type Now } from "./actions";
import { Menued, useNoBrowserMenu } from "./Menus";
import { TIP_AFTER_MS } from "./ui/menu/MenuTip";
import { noTabs, openTab, selectTab } from "./tabs";
import { TOOLTIP_CLOSE_DELAY } from "./ui/tooltip/Tooltip";

/**
 * The context menus, and the browser menu they replace.
 *
 * Two claims are worth a test and both are about the SAME list: that a menu draws the
 * catalogue's rows rather than a list of its own, and that a row the catalogue does not have
 * is simply not in the menu. Everything else — what a row says, whether it can run, what it
 * costs — is `actions.test.ts`'s, and asserting it again here would be the second answer this
 * whole design exists to prevent.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  clearMocks();
});

function now(over: Partial<Now> = {}): Now {
  return {
    tabs: noTabs(),
    workspaces: [],
    needsYou: [],
    nameOf: (session) => String(session),
    ...over,
  };
}

/** The rows a menu would draw, as their titles — above the line, then below it. */
function titles(what: Parameters<typeof menuRows>[0], over: Partial<Now> = {}) {
  const rows = menuRows(what, catalogued(catalogue(now(over))));
  return {
    above: rows.above.map((row) => row.title),
    below: rows.below.map((row) => row.title),
  };
}

describe("what a menu lists", () => {
  it("is the catalogue, filtered to the thing it was opened on", () => {
    const shown = titles(
      { on: "workspace", workspace: "alpha" },
      {
        workspaces: ["alpha", "beta"],
        focused: "beta",
        plane: "/plane",
      },
    );

    expect(shown.above).toEqual([
      "Focus workspace alpha",
      "New shell in alpha",
      "Pin workspace alpha",
      "Workspace settings…",
      "Make alpha live…",
      "Rename workspace alpha…",
      "New workspace…",
    ]);
    expect(shown.below).toEqual(["Delete workspace alpha"]);
  });

  it("drops a row the catalogue does not have, without knowing which rows those are", () => {
    // A workspace with no settings row offered — here, one the catalogue was not told is on
    // the plane — has no such row in its menu. Nothing in `Menus.tsx` or in `menuOn` was told
    // which: the rows are looked up by id and the ones that do not exist are not found.
    const shown = titles({ on: "workspace", workspace: "ghost" }, { workspaces: ["alpha"] });

    expect(shown.above).toEqual(["New workspace…"]);
    expect(shown.below).toEqual([]);
  });

  it("gives the plane root a menu of its own: focus, a chat and a shell there, and no delete (SI-1)", () => {
    const shown = titles({ on: "root" }, { workspaces: [OUTSIDE, "alpha"], plane: "/plane" });

    expect(shown.above).toEqual([
      "Focus the plane root",
      "New chat at the project root",
      "New shell at the project root",
      "New workspace…",
    ]);
    expect(shown.below).toEqual([]);
  });

  it("puts everything that destroys something below the line and nothing else", () => {
    const chat = titles({ on: "chat", tab: 1 }, { tabs: openTab(noTabs(), 7, "3 steward") });

    expect(chat.above).toEqual([
      "Switch to tab 3 steward",
      "Rename chat 3 steward…",
      "Pin chat 3 steward",
      // Its Activity only reads (#1495).
      "Activity",
      "Network",
    ]);
    expect(chat.below).toEqual(["End chat 3 steward"]);
  });

  it("offers a worktree's own verbs on the explorer's rows (charter-app#174)", () => {
    // The surface #172 could not reach, and the reason it could not: the catalogue had two
    // worktree rows and both were about the chat in front. These are about this piece.
    const cut = { workspace: "alpha", repo: "svc", piece: "fix-it", branch: "fix-it" };
    const shown = titles(
      { on: "worktree", repo: "svc", piece: "fix-it" },
      {
        plane: "/plane",
        pieces: [cut],
      },
    );

    expect(shown.above).toEqual([
      "Focus on branch fix-it",
      "Browse the files of fix-it",
      // The branch's own folder (#1143).
      "Copy the absolute path of branch fix-it",
      expect.stringMatching(/^Reveal branch fix-it in /),
      "Open a shell tab in branch fix-it",
      "Merge branch fix-it into svc",
    ]);
    expect(shown.below).toEqual(["Remove folder fix-it in svc"]);
  });

  it("draws the discard row on the piece whose removal was refused, and on no other", () => {
    // Listed in every worktree menu by `menuOn` and found in one, because the catalogue only
    // builds it beside the removal the core has just refused. Nothing in `Menus.tsx` or in
    // `menuOn` knows that — it is the same drop that leaves `Outside every workspace` with
    // no pin row.
    const pieces = [
      { workspace: "alpha", repo: "svc", piece: "fix-it" },
      { workspace: "alpha", repo: "svc", piece: "other" },
    ];
    const over = { plane: "/plane", pieces, refused: "worktree.remove:svc/fix-it" };

    expect(titles({ on: "worktree", repo: "svc", piece: "fix-it" }, over).below).toEqual([
      "Remove folder fix-it in svc",
      "Discard that work and remove fix-it anyway",
    ]);
    expect(titles({ on: "worktree", repo: "svc", piece: "other" }, over).below).toEqual([
      "Remove folder other in svc",
    ]);
  });

  it("offers a persona's reading and editing above the line and its deletion below (SI-3)", () => {
    const shown = titles({ on: "persona", persona: "steward" }, { personas: ["steward"] });

    expect(shown.above).toEqual([
      "Show what steward is",
      "Edit steward's persona.md",
      "Set steward's profile…",
      "New persona…",
    ]);
    expect(shown.below).toEqual(["Delete persona steward…"]);
  });

  it("offers a vault's opening above the line and its deletion below (SI-3)", () => {
    const shown = titles({ on: "vault", vault: "ops" }, { vaults: ["ops"] });

    expect(shown.above).toEqual(["Open vault ops", "New vault…"]);
    expect(shown.below).toEqual(["Delete vault ops…"]);
  });

  it("offers to open and close a todo above the line and to forget it below (SI-3, #1214)", () => {
    const shown = titles(
      { on: "todo", slug: "20260302-091400-review" },
      {
        workspaces: ["alpha"],
        focused: "alpha",
        todos: [{ slug: "20260302-091400-review", title: "Review the plan" }],
      },
    );

    expect(shown.above).toEqual(["Open todo: Review the plan", "Mark done: Review the plan"]);
    expect(shown.below).toEqual(["Forget todo Review the plan"]);
  });

  it("offers the pane's own verbs in the centre of the window", () => {
    const pane = titles({ on: "pane" }, { tabs: openTab(noTabs(), 7, "one") });

    expect(pane.above).toEqual([
      "New tab",
      "New shell",
      "Split right",
      "Split down",
      "Send F2 to the chat in front",
    ]);
    expect(pane.below).toEqual(["End this pane's chat"]);
  });

  it("keeps a row that cannot run, with the catalogue's reason on it", () => {
    // The same rule the palette follows: an operator cannot ask about an option they cannot
    // see, and a greyed row saying why is an answer where a missing row is a mystery.
    const rows = menuRows(
      { on: "workspace", workspace: "alpha" },
      catalogued(catalogue(now({ workspaces: ["alpha"], focused: "alpha", plane: "/plane" }))),
    );

    const focus = rows.above[0];
    expect(focus.available).toBe(false);
    expect(focus.reason).toBe("It is already focused.");
  });
});

/** Twelve personas, as the project in the report had. */
const TWELVE = [
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

describe("a menu on screen", () => {
  const NAME = "console-developer 16";

  /** A trigger with a chat's rows on it, and what was pressed: the chat's tab is behind another,
   *  the project has twelve personas, and the chat is listed, so it has its Stop rows. */
  function aMenu() {
    const pressed: string[] = [];
    const tabs = openTab(openTab(noTabs(), 7, "16", "console-developer"), 9, "1", "smart-ide");
    const listed = (session: number, name: string) => ({
      session,
      name,
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
    });
    const offers = catalogued(
      catalogue(
        now({
          tabs: selectTab(tabs, tabs.order[1]),
          ask: { personas: TWELVE, locked: null },
          askable: () => true,
          restartable: () => true,
          listed: [listed(7, NAME), listed(9, "smart-ide 1")],
        }),
      ),
    );
    render(
      <Menued
        on={{ on: "chat", tab: tabs.order[0], session: 7 }}
        offers={offers}
        onPress={(offer) => pressed.push(offer.id)}
      >
        <span data-testid="tab" tabIndex={0}>
          {NAME}
        </span>
      </Menued>,
    );
    return pressed;
  }

  /** Every row of a menu, whatever kind of row it is. */
  const rowsIn = (menu: HTMLElement) =>
    Array.from(menu.querySelectorAll<HTMLElement>('[role^="menuitem"]'));

  async function open(): Promise<HTMLElement> {
    fireEvent.contextMenu(screen.getByTestId("tab"));
    return screen.findByRole("menu");
  }

  /** The submenu a row of the open menu opens, opened. */
  async function openSub(name: string): Promise<HTMLElement> {
    await userEvent.click(screen.getByRole("menuitem", { name }));
    const menus = await screen.findAllByRole("menu");
    return menus[menus.length - 1];
  }

  it("opens on a right-click, named for the tab, and draws short rows", async () => {
    aMenu();

    const menu = await open();

    // **The tab is named once, by the menu**, and by no row of it.
    expect(menu).toHaveAccessibleName(`Tab ${NAME}`);
    expect(rowsIn(menu).map((row) => row.getAttribute("aria-label"))).toEqual([
      "Switch to",
      "Rename…",
      "Pinned",
      "Open",
      "Ask",
      "Restart",
      "Stop",
      "End",
    ]);
    // What a row draws: its words, and the one line that may stand under them.
    for (const row of rowsIn(menu))
      expect(row.querySelector('[data-part="words"]')?.textContent).not.toContain(NAME);
  });

  it("has at most ten rows with twelve personas, and Ask is one row that opens them all", async () => {
    aMenu();

    const menu = await open();
    expect(rowsIn(menu).length).toBeLessThanOrEqual(10);
    expect(within(menu).queryByRole("menuitem", { name: /authz-master/ })).toBeNull();
    const ask = within(menu).getByRole("menuitem", { name: "Ask" });
    expect(ask).toHaveAttribute("aria-haspopup", "menu");

    const sub = await openSub("Ask");
    expect(rowsIn(sub).map((row) => row.getAttribute("aria-label"))).toEqual(
      TWELVE.map((persona) => `${persona}…`),
    );
    // What asking does is said once, by the row that opened them, and under no persona.
    expect(sub.querySelector('[data-part="description"]')).toBeNull();
    expect(ask).toHaveAccessibleDescription(/^A task starts as the persona you pick/);
  });

  it("draws a note under no row but one that cannot be undone or cannot run", async () => {
    aMenu();
    const menu = await open();

    const noted = rowsIn(menu).filter((row) => row.querySelector('[data-part="description"]'));
    expect(noted.map((row) => row.getAttribute("aria-label"))).toEqual(["End"]);
    expect(noted[0].querySelector('[data-part="description"]')).toHaveTextContent(
      "Ends the program it runs. There is no undo.",
    );

    // In the Stop submenu: each way to stop it ends something, and the two that have nothing
    // to end cannot run and say why.
    const sub = await openSub("Stop");
    expect(
      rowsIn(sub).map((row) => [
        row.getAttribute("aria-label"),
        row.querySelector('[data-part="description"]')?.textContent,
      ]),
    ).toEqual([
      ["Stop", "Ends the chat."],
      ["Stop and everything below it", `${NAME} started no chat that is still running.`],
      ["Stop all tasks", `${NAME} has no task open.`],
    ]);
    for (const row of rowsIn(sub))
      expect(
        row.getAttribute("data-variant") === "danger" ||
          row.getAttribute("aria-disabled") === "true",
      ).toBe(true);
  });

  it("keeps every row's description: its note, or why it cannot run", async () => {
    aMenu();
    const menu = await open();

    const row = (name: string) => {
      const found = rowsIn(menu).find((one) => one.getAttribute("aria-label") === name);
      if (found === undefined) throw new Error(`no row ${name}`);
      return found;
    };
    expect(row("Pinned")).toHaveAccessibleDescription(
      "Draws it first on its strip. Yours, on this machine only.",
    );
    // The catalogue's note as it reads now: this chat reports no state, and the note says so.
    expect(row("Restart")).toHaveAccessibleDescription(/It keeps its conversation/);
    expect(row("End")).toHaveAccessibleDescription("Ends the program it runs. There is no undo.");
    // A row with nothing more to say than its words has none.
    expect(row("Rename…")).not.toHaveAttribute("aria-describedby");

    const sub = await openSub("Stop");
    const below = within(sub).getByRole("menuitem", { name: "Stop and everything below it" });
    // The whole reason, where the line under the row had no room for it.
    expect(below).toHaveAccessibleDescription(`${NAME} started no chat that is still running.`);
  });

  it("raises no row's note for the focus a menu puts on a row as it opens", async () => {
    aMenu();
    const menu = await open();
    const pinned = within(menu).getByRole("menuitemcheckbox", { name: "Pinned" });
    vi.useFakeTimers();
    try {
      // No key since the menu came, and the pointer is not on the row: the menu's own focus.
      act(() => pinned.focus());
      act(() => {
        vi.advanceTimersByTime(TIP_AFTER_MS * 2);
      });
      expect(pinned).toBeInTheDocument();
      expect(document.querySelector('[data-part="tip"]')).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("shows a row's note as its tooltip once the keyboard has rested on it, and not before", async () => {
    aMenu();
    const menu = await open();
    const pinned = within(menu).getByRole("menuitemcheckbox", { name: "Pinned" });
    vi.useFakeTimers();
    try {
      // A key moved the keyboard here, as a person's does.
      fireEvent.keyDown(document.body, { key: "ArrowDown" });
      act(() => pinned.focus());
      act(() => {
        vi.advanceTimersByTime(TIP_AFTER_MS - 1);
      });
      expect(document.querySelector('[data-part="tip"]')).toBeNull();
      // No `title` either: the platform's tooltip would say it a second time.
      expect(pinned).not.toHaveAttribute("title");

      act(() => {
        vi.advanceTimersByTime(1);
      });
      const tip = document.querySelector('[data-part="tip"]');
      expect(tip).toHaveTextContent("Draws it first on its strip. Yours, on this machine only.");
      expect(tip).toHaveClass("ui-text-secondary");
      // The window's one tooltip (#2908).
      expect(tip?.closest('[role="tooltip"]')).toHaveAttribute("data-scope", "tooltip");

      // It goes a moment after the keyboard leaves the row: time for the pointer to cross onto
      // it, which keeps it (WCAG 1.4.13, hoverable).
      // A key moved the keyboard here, as a person's does.
      fireEvent.keyDown(document.body, { key: "ArrowDown" });
      act(() => within(menu).getByRole("menuitem", { name: "Rename…" }).focus());
      act(() => {
        vi.advanceTimersByTime(TOOLTIP_CLOSE_DELAY);
      });
      expect(document.querySelector('[data-part="tip"]')).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("keeps a row's note while the pointer is on it, and lets it go once the pointer leaves", async () => {
    aMenu();
    const menu = await open();
    vi.useFakeTimers();
    try {
      // A key moved the keyboard here, as a person's does.
      fireEvent.keyDown(document.body, { key: "ArrowDown" });
      act(() => within(menu).getByRole("menuitemcheckbox", { name: "Pinned" }).focus());
      act(() => {
        vi.advanceTimersByTime(TIP_AFTER_MS);
      });
      const tip = document.querySelector('[data-part="tip"]')?.closest('[role="tooltip"]');
      expect(tip).not.toBeNull();
      // The pointer leaves the row for the note: the row loses the keyboard, the note stays.
      fireEvent.pointerEnter(tip as Element);
      // A key moved the keyboard here, as a person's does.
      fireEvent.keyDown(document.body, { key: "ArrowDown" });
      act(() => within(menu).getByRole("menuitem", { name: "Rename…" }).focus());
      act(() => {
        vi.advanceTimersByTime(TOOLTIP_CLOSE_DELAY * 10);
      });
      expect(document.querySelector('[data-part="tip"]')).not.toBeNull();
      fireEvent.pointerLeave(tip as Element);
      act(() => {
        vi.advanceTimersByTime(TOOLTIP_CLOSE_DELAY);
      });
      expect(document.querySelector('[data-part="tip"]')).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("takes the first Escape for the row's note, and the next one closes the menu", async () => {
    aMenu();
    const menu = await open();
    vi.useFakeTimers();
    try {
      // A key moved the keyboard here, as a person's does.
      fireEvent.keyDown(document.body, { key: "ArrowDown" });
      act(() => within(menu).getByRole("menuitemcheckbox", { name: "Pinned" }).focus());
      act(() => {
        vi.advanceTimersByTime(TIP_AFTER_MS);
      });
      expect(document.querySelector('[data-part="tip"]')).not.toBeNull();

      fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });
      act(() => {
        vi.runOnlyPendingTimers();
      });
      // The note went, and the menu stays with the keyboard on its row (WCAG 1.4.13).
      expect(document.querySelector('[data-part="tip"]')).toBeNull();
      expect(screen.queryByRole("menu")).not.toBeNull();
      fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });
      act(() => {
        vi.runOnlyPendingTimers();
      });
    } finally {
      vi.useRealTimers();
    }
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("colours only what cannot be undone, by meaning and by look", async () => {
    // `ends-it` is the house meaning (D-1719-3) and the `danger` variant the look (DS-2): a
    // row that cannot be undone carries both, and no other row carries either.
    aMenu();
    const menu = await open();

    expect(
      rowsIn(menu)
        .filter((row) => row.getAttribute("data-variant") === "danger")
        .map((row) => row.getAttribute("aria-label")),
    ).toEqual(["End"]);
    const ends = within(menu).getByRole("menuitem", { name: "End" });
    expect(ends).toHaveClass("ends-it");
    expect(ends).toHaveAttribute("data-scope", "menu");
    expect(ends).toHaveAttribute("data-variant", "danger");
    // Restart keeps the conversation, and Stop only opens the ways to stop.
    for (const name of ["Restart", "Stop", "Switch to"]) {
      const row = within(menu).getByRole("menuitem", { name });
      expect(row).not.toHaveClass("ends-it");
      expect(row).toHaveAttribute("data-variant", "default");
    }
  });

  it("says the key that does the same at the row's end, and ticks a toggle that is on", async () => {
    aMenu();
    const menu = await open();

    const keys = (name: string) =>
      within(menu).getByRole("menuitem", { name }).querySelector('[data-part="shortcut"]');
    expect(keys("Rename…")).toHaveTextContent("F2");
    expect(keys("End")).toHaveTextContent(/^(⌫|Delete)$/);
    expect(keys("Restart")).toBeNull();
    expect(within(menu).getByRole("menuitemcheckbox", { name: "Pinned" })).toHaveAttribute(
      "aria-checked",
      "false",
    );
  });

  it("moves by the arrows, opens a submenu on Right, finds a persona by its letters, and closes on Left and Escape", async () => {
    const user = userEvent.setup();
    const pressed = aMenu();
    const tab = screen.getByTestId("tab");
    tab.focus();
    fireEvent.keyDown(tab, { key: "F10", shiftKey: true });
    const menu = await screen.findByRole("menu");

    // Opened from the keyboard, so the first row has it; the arrows move on from there.
    await waitFor(() =>
      expect(within(menu).getByRole("menuitem", { name: "Switch to" })).toHaveFocus(),
    );
    await user.keyboard("{ArrowDown}");
    expect(within(menu).getByRole("menuitem", { name: "Rename…" })).toHaveFocus();
    // Type-ahead reads a row's words, not its note.
    await user.keyboard("a");
    const ask = within(menu).getByRole("menuitem", { name: "Ask" });
    expect(ask).toHaveFocus();

    await user.keyboard("{ArrowRight}");
    const sub = (await screen.findAllByRole("menu"))[1];
    expect(within(sub).getByRole("menuitem", { name: "authz-master…" })).toHaveFocus();
    await user.keyboard("q");
    expect(within(sub).getByRole("menuitem", { name: "qa…" })).toHaveFocus();

    await user.keyboard("{ArrowLeft}");
    await waitFor(() => expect(screen.getAllByRole("menu")).toHaveLength(1));
    // The keyboard is handed back to the row that opened it once the submenu has gone.
    await waitFor(() => expect(ask).toHaveFocus());

    await user.keyboard("{ArrowRight}q{Enter}");
    expect(pressed).toEqual([askId(1, "qa")]);
    expect(screen.queryByRole("menu")).toBeNull();

    fireEvent.keyDown(tab, { key: "F10", shiftKey: true });
    await screen.findByRole("menu");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("hands back the catalogue's own offer when a row is pressed", async () => {
    const pressed = aMenu();
    const menu = await open();

    await userEvent.click(within(menu).getByRole("menuitem", { name: "End" }));

    expect(pressed).toEqual(["tab.close:1"]);
  });

  it("adds no element of its own to the strip it is on", () => {
    // `asChild`: the trigger IS the element, because the chat strip's tabs are measured to
    // decide what is off screen (`offscreen.ts`) and a wrapper would be measured instead.
    aMenu();

    const tab = screen.getByTestId("tab");
    expect(tab.parentElement?.tagName).toBe("DIV");
    expect(tab.parentElement?.getAttribute("data-testid")).toBeNull();
  });
});

/** What the core answers `curation_offers` for workspace `alpha` (ADR 0061): charter's own two,
 *  `ops`'s and `qa`'s, and one file it left out. */
function curations(cannot: string | null = null): NonNullable<Now["curations"]> {
  const action = (id: string, label: string, by: string | null) => ({
    id,
    label,
    declared_by: by,
    runner: by ?? "steward",
    cwd: "/plane/workspaces/alpha",
    prompt: `${label} alpha.`,
  });
  return {
    subjects: [
      {
        subject: "workspace:alpha",
        name: "alpha",
        actions: [
          action("charter/safe-remove", "Safe remove", null),
          action("charter/compact", "Compact & improve", null),
          action("ops/tidy", "Tidy", "ops"),
          action("qa/audit", "Audit", "qa"),
        ],
        left_out: [
          {
            what: "personas/ops/curation/bad.md",
            why: "personas/ops/curation/bad.md is not offered: no label.",
          },
        ],
        trouble: null,
      },
    ],
    cannot,
  };
}

describe("the Curate ▸ submenu (ADR 0061)", () => {
  function aWorkspaceMenu(cannot: string | null = null, harnesses?: Now["harnesses"]) {
    const pressed: string[] = [];
    const offers = catalogued(
      catalogue(
        now({ workspaces: ["alpha"], plane: "/plane", curations: curations(cannot), harnesses }),
      ),
    );
    render(
      <Menued
        on={{ on: "workspace", workspace: "alpha" }}
        offers={offers}
        onPress={(offer) => pressed.push(offer.id)}
      >
        <span data-testid="strip-alpha">alpha</span>
      </Menued>,
    );
    return pressed;
  }

  async function openCurate() {
    fireEvent.contextMenu(screen.getByTestId("strip-alpha"));
    const menu = await screen.findByRole("menu");
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Curate" }));
    const menus = await screen.findAllByRole("menu");
    return menus[menus.length - 1];
  }

  it("lists charter's own first, then a named group per persona, then what was left out", async () => {
    aWorkspaceMenu();

    const sub = await openCurate();

    expect(
      Array.from(
        sub.querySelectorAll('[role=menuitem], [data-part="header"], [data-part="rule"]'),
      ).map((el) =>
        el.getAttribute("role") === "menuitem"
          ? el.getAttribute("aria-label")
          : el.getAttribute("data-part") === "header"
            ? `[${el.textContent}]`
            : "---",
      ),
    ).toEqual([
      "Safe remove",
      "Compact & improve",
      "---",
      "[ops]",
      "Tidy",
      "---",
      "[qa]",
      "Audit",
      "---",
      "personas/ops/curation/bad.md is left out",
    ]);
    // **A persona's name is data, and is written as it was given** (ruling T4): its group's
    // label does not wear the label role, whose capitals would head it "OPS".
    const ops = within(sub).getByText("ops");
    expect(ops).toHaveAttribute("data-part", "header");
    expect(ops).toHaveAttribute("data-named");
  });

  it("draws an action the core left out as a row that cannot run, saying why", async () => {
    aWorkspaceMenu();

    const sub = await openCurate();

    const left = within(sub).getByRole("menuitem", {
      name: "personas/ops/curation/bad.md is left out",
    });
    expect(left.getAttribute("aria-disabled")).toBe("true");
    expect(left).toHaveAccessibleDescription(
      "personas/ops/curation/bad.md is not offered: no label.",
    );
  });

  it("hands back the action's own row when one is chosen", async () => {
    const pressed = aWorkspaceMenu();

    const sub = await openCurate();
    await userEvent.click(within(sub).getByRole("menuitem", { name: "Tidy" }));

    expect(pressed).toEqual(["curate:workspace:alpha/ops/tidy"]);
  });

  it("draws every action ready to run when the core says a chat can be typed into (a Codex default, SI-2e)", async () => {
    // The core answers `cannot: null` for a Codex default as it does for Claude Code: the
    // prompt is typed once Codex's terminal is raw and quiet (ADR 0061, amended 2026-09-27).
    const pressed = aWorkspaceMenu(null);

    const sub = await openCurate();

    for (const name of ["Safe remove", "Compact & improve", "Tidy", "Audit"]) {
      const row = within(sub).getByRole("menuitem", { name });
      expect(row.getAttribute("aria-disabled")).not.toBe("true");
    }
    await userEvent.click(within(sub).getByRole("menuitem", { name: "Safe remove" }));
    expect(pressed).toEqual(["curate:workspace:alpha/charter/safe-remove"]);
  });

  it("draws every action disabled, with the reason, when no chat can be typed into", async () => {
    aWorkspaceMenu(
      "The default profile 'work' runs opencode, which says nothing until your first prompt.",
    );

    const sub = await openCurate();

    const safe = within(sub).getByRole("menuitem", { name: "Safe remove" });
    expect(safe.getAttribute("aria-disabled")).toBe("true");
    expect(safe).toHaveAccessibleDescription(expect.stringContaining("first prompt"));
  });

  /** opencode's card, as the core reads it: what it lacks, and its label. */
  const OPENCODE = {
    name: "opencode",
    title: "opencode",
    label: "What opencode can do here",
    lines: [
      "opencode cannot have a prompt typed in for you, because purlis cannot tell when it has finished starting.",
    ],
    cannot_type:
      "opencode cannot have a prompt typed in for you, because purlis cannot tell when it has finished starting. See What opencode can do here.",
  };
  const OFF_ON_OPENCODE =
    "The default profile 'work' cannot be typed into: opencode cannot have a prompt typed in for you, because purlis cannot tell when it has finished starting. See What opencode can do here.";

  it("ends the off rows with the card's label as a row of its own that opens the card (#1134)", async () => {
    const pressed = aWorkspaceMenu(OFF_ON_OPENCODE, [OPENCODE]);

    const sub = await openCurate();

    // The off rows keep their reason; the card is one row under them, once, and it can run.
    expect(within(sub).getByRole("menuitem", { name: "Safe remove" })).toHaveAccessibleDescription(
      OFF_ON_OPENCODE,
    );
    const card = within(sub).getAllByRole("menuitem", { name: "What opencode can do here" });
    expect(card).toHaveLength(1);
    expect(card[0].getAttribute("aria-disabled")).not.toBe("true");
    await userEvent.click(card[0]);
    expect(pressed).toEqual(["harness.card:opencode"]);
  });

  it("draws no card row where no off row names a card", async () => {
    aWorkspaceMenu(null, [OPENCODE]);

    const sub = await openCurate();

    expect(within(sub).queryByRole("menuitem", { name: "What opencode can do here" })).toBeNull();
  });

  it("curates the plane from the plane root's tab (SI-1)", async () => {
    const pressed: string[] = [];
    const plane = {
      subject: "plane",
      name: "plane",
      actions: [
        {
          id: "ops/audit",
          label: "Audit the plane",
          declared_by: "ops",
          runner: "ops",
          cwd: "/plane",
          prompt: "Audit.",
        },
      ],
      left_out: [],
      trouble: null,
    };
    render(
      <Menued
        on={{ on: "root" }}
        offers={catalogued(
          catalogue(
            now({
              workspaces: [OUTSIDE, "alpha"],
              plane: "/plane",
              curations: { subjects: [plane], cannot: null },
            }),
          ),
        )}
        onPress={(offer) => pressed.push(offer.id)}
      >
        <span data-testid="root">root</span>
      </Menued>,
    );

    fireEvent.contextMenu(screen.getByTestId("root"));
    const menu = await screen.findByRole("menu");
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Curate" }));
    const menus = await screen.findAllByRole("menu");
    await userEvent.click(
      within(menus[menus.length - 1]).getByRole("menuitem", { name: "Audit the plane" }),
    );

    expect(pressed).toEqual(["curate:plane/ops/audit"]);
  });
});

describe("a menu from the keyboard (charter-app#174)", () => {
  /** A focusable row with a menu, and a text box inside a second trigger — a pane's terminal
   *  takes its keys the same way. */
  function rows() {
    const pressed: string[] = [];
    const offers = catalogue(
      now({ tabs: openTab(noTabs(), 7, "3 steward"), clones: [{ repo: "svc", path: "/svc" }] }),
    );
    render(
      <>
        <Menued
          on={{ on: "clone", repo: "svc" }}
          offers={catalogued(offers)}
          onPress={(offer) => pressed.push(offer.id)}
        >
          <button type="button">svc</button>
        </Menued>
        <Menued on={{ on: "chat", tab: 1 }} offers={catalogued(offers)} onPress={() => {}}>
          <div data-testid="panes">
            <textarea aria-label="terminal" />
          </div>
        </Menued>
      </>,
    );
    return pressed;
  }

  for (const key of [{ key: "F10", shiftKey: true }, { key: "ContextMenu" }]) {
    it(`opens on ${key.shiftKey ? "Shift+F10" : "the menu key"} on the row that has the keyboard`, async () => {
      // macOS has no keyboard convention for a context menu, so its WebView raises no
      // `contextmenu` for either key; without this a menu there is a pointer's alone.
      const pressed = rows();
      const row = screen.getByRole("button", { name: "svc" });
      row.focus();

      fireEvent.keyDown(row, key);

      const menu = await screen.findByRole("menu");
      expect(menu).toHaveAccessibleName("Repo svc");
      expect(
        within(menu)
          .getAllByRole("menuitem")
          .map((item) => item.textContent),
      ).toEqual(["Focus", "New tab", "New branch…", "Start new chats here"]);
      await userEvent.keyboard("{Enter}");
      expect(pressed).toEqual(["clone.focus:svc"]);
    });
  }

  it("leaves Shift+F10 to whatever inside the trigger has the keyboard", () => {
    // The panes' menu is on the box the terminals are in, and a program in a terminal may want
    // the key. Only the trigger itself having the keyboard opens its menu.
    rows();
    const terminal = screen.getByRole("textbox", { name: "terminal" });
    terminal.focus();

    fireEvent.keyDown(terminal, { key: "F10", shiftKey: true });

    expect(screen.queryByRole("menu")).toBeNull();
  });
});

describe("the browser's own menu", () => {
  function Window() {
    useNoBrowserMenu();
    return (
      <div>
        <button type="button">nothing here has a menu</button>
        <input aria-label="a box" defaultValue="paste me" />
      </div>
    );
  }

  it("is taken away everywhere, including where charter has no menu of its own", () => {
    // A shipped app that answers a right-click with `Reload` and `Inspect Element` is showing
    // the operator the browser it is built on.
    render(<Window />);

    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    screen.getByRole("button").dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
  });

  it("is left alone where the operator is editing text", () => {
    // The platform's Cut/Copy/Paste is not the browser showing through — it is the only
    // pointer route to the clipboard this window has, and `Inspect Element` is off in a
    // release build anyway.
    render(<Window />);

    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    screen.getByRole("textbox", { name: "a box" }).dispatchEvent(event);

    expect(event.defaultPrevented).toBe(false);
  });

  it("is prevented on the whole window of the real app", () => {
    mockIPC(() => null);
    render(<App />);

    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    document.body.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
  });
});

/**
 * **The discriminator**, and the reason it is written out at length rather than folded into
 * the tests above.
 *
 * The scenario run cannot open a context menu: a WebDriver right-click opens nothing on
 * webkit 605.1.15 (macOS) or on WebKitGTK 605.1.15 (Linux) — `e2e/specs/workspace-lifecycle`
 * records the runs. Two engines behaving identically under one driver points at the driver,
 * but "points at" is not a measurement, and the alternative — that charter's own wiring
 * swallows the event — would be a bug the operator meets a minute after installing.
 *
 * So the question is asked where there is **no WebDriver anywhere in the picture**: one
 * `MouseEvent`, constructed and dispatched on a real workspace tab of the real `App`, with
 * `useNoBrowserMenu` mounted — which is the only thing charter has that could prevent it.
 *
 * What this stands guard over is the ordering `Menus.tsx` documents. React 19 attaches its
 * delegated listeners to the root container, BELOW `window`, so Radix's composed
 * `onContextMenu` runs before the window-level suppressor and opens the menu. A capturing
 * suppressor would reverse that: `composeEventHandlers` skips Radix's own handler once the
 * event is `defaultPrevented`, so every context menu in the app would stop opening —
 * silently, because nothing throws and the browser menu would still be gone.
 */
describe("a real contextmenu event, with the suppressor live", () => {
  const PLANE = "/home/dev/plane";

  function aPlane() {
    mockIPC((cmd) => {
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: [],
          persona: null,
          unfiled: [],
          workspaces: [
            { name: "alpha", path: `${PLANE}/workspaces/alpha`, vision: "", todos: [], chats: [] },
          ],
        };
      if (
        cmd === "opened_chats" ||
        cmd === "chats_that_would_not_start" ||
        cmd === "running_sessions" ||
        cmd === "chat_states"
      )
        return [];
      return null;
    });
  }

  it("opens charter's own menu, and still takes the browser's away", async () => {
    aPlane();
    render(<App />);
    const tab = await screen.findByRole("tab", { name: /alpha/ });

    // Not `fireEvent`, deliberately: what a WebView sends is a `MouseEvent`, so that is what
    // is sent, and the event object is kept so the second claim can be made about it.
    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    tab.dispatchEvent(event);

    const menu = await screen.findByRole("menu");
    // The focused workspace's own tab: no Focus row, which could only say it is focused.
    expect(menu).toHaveAccessibleName("Workspace alpha");
    expect(
      Array.from(menu.querySelectorAll('[role^="menuitem"]')).map((row) =>
        row.getAttribute("aria-label"),
      ),
    ).toEqual([
      "Pinned",
      "Settings…",
      "Make live…",
      "Rename…",
      "New shell",
      "New workspace…",
      "Delete",
    ]);
    // Both halves of one event: charter answered it, and the WebView's own menu is still gone.
    expect(event.defaultPrevented).toBe(true);
  });
});
