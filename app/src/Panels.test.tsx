// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks } from "@tauri-apps/api/mocks";
import { attentionPanels, Panels } from "./Panels";
import { HeadActions } from "./viewHead";
import { ChatsHere, fixedChats, nothingKnown, type State } from "./chatState";
import { drawnWith } from "./cascade.testkit";
import { ReadOnlySaid } from "./ReadOnlyLine";
import { wears } from "./textRoles.testkit";
import { CATALOGUE, type ViewId } from "./regions";
import { catalogue, catalogued, type Catalogued, type Offer } from "./actions";
import { noTabs } from "./tabs";
import type {
  ExtensionView,
  PanelRow,
  PanelView,
  Panels as PanelsModel,
  VaultSummary,
} from "./bindings";
import type { WorkspaceState } from "./workspaceState";
import { hoverTipOf, tipOf } from "./ui/tooltip/tooltip.testkit";

afterEach(() => {
  cleanup();
  clearMocks();
});

/** The project these answers are for. A persona belongs to a plane, and a window holds
 *  several: two of them can both have a `steward`. */
const PLANE = "/home/dev/plane";

// ---------------------------------------------------------------------------------------
// The contributions, as `app/src-tauri/src/panels.rs` builds them
// ---------------------------------------------------------------------------------------

/**
 * **These fixtures are `purlis_core::panel` values, and that is the change.**
 *
 * Until this file was rewritten it asserted that `Panels.tsx` drew a todo out of a `todos`
 * field. There is no such field in the renderer now: todos and personas are contributions, so
 * what this file tests is the thing that draws a panel, against the vocabulary it draws from.
 *
 * Where the other half went — that charter's own producer emits *these* values off a real
 * plane — is two places, both closer to the fact than a mock here could be:
 * `app/src-tauri/src/panels.rs`'s tests, against a plane on disk, and `regions.e2e.ts`, against
 * the real core through the real window.
 */
function row(key: string, text: string, over: Partial<PanelRow> = {}): PanelRow {
  return {
    key,
    text,
    note: null,
    mark: "dot",
    tone: "plain",
    detail: null,
    runs: null,
    actions: [],
    ...over,
  };
}

function todosPanel(rows: PanelRow[], refused?: string): PanelView {
  return {
    key: "charter/todos",
    title: "Todos",
    order: 10,
    mark: "todo",
    from: null,
    about: null,
    blocks: [
      ...(refused === undefined ? [] : [{ kind: "note" as const, text: refused, tone: "trouble" }]),
      {
        kind: "list" as const,
        rows,
        empty: { headline: "Nothing to do", body: null, offer: null },
      },
    ],
  };
}

function personasPanel(names: string[], fallback: string | null): PanelView {
  return {
    key: "charter/personas",
    title: "Personas",
    order: 20,
    mark: "persona",
    from: null,
    about: "personas",
    blocks: [
      {
        kind: "list",
        rows: names.map((name) =>
          row(name, name, {
            mark: "persona",
            tone: name === fallback ? "default" : "plain",
            note: name === fallback ? "default · 2 memories" : "2 memories",
            // No card: the row opens the persona's own tab (`panels.rs`, `charters_own`).
            detail: null,
            runs: `persona.show:${name}`,
          }),
        ),
        empty: { headline: "No personas on this plane", body: null, offer: null },
      },
    ],
  };
}

/** What an approved extension contributes: declared rows, no verbs, and its id on the panel. */
function contributedPanel(over: Partial<PanelView> = {}): PanelView {
  return {
    key: "ext/acme/reviews",
    title: "Reviews",
    order: 15,
    mark: "note",
    from: "acme",
    about: null,
    blocks: [
      {
        kind: "list",
        rows: [
          row("a", "Land the panel contribution contract", {
            note: "2026-09-23",
            detail: { kind: "text", text: "The whole of what this review is about." },
          }),
        ],
        empty: { headline: "Nothing to review", body: null, offer: null },
      },
    ],
    ...over,
  };
}

const TODO = row("20260302-091400-review", "Review the rollout plan", {
  mark: "todo",
  note: "2026-03-02",
  detail: { kind: "text", text: "The rollout plan, in full." },
});

const PANELS: PanelsModel = {
  workspace: "alpha",
  repos: ["svc", "tool"],
  paths: {},
  absent: [],
  refused: [],
  todos: [
    { slug: "20260302-091400-review", title: "Review the rollout plan", stamp: "2026-03-02" },
  ],
  todos_refused: null,
  done_todos: [],
  personas: ["devops", "steward"],
  persona: "steward",
  sessions: [],
  contributed: [todosPanel([TODO]), personasPanel(["devops", "steward"], "steward")],
};

function state(on: Partial<WorkspaceState> = {}): WorkspaceState {
  return {
    panels: PANELS,
    repos: { workspace: "alpha", repos: [], cache_refused: null },
    pieces: {},
    piecesRefused: {},
    reading: false,
    ...on,
  };
}

/**
 * The panel, with the window's own state around it.
 *
 * **Which row's card is open belongs to the window** (charter-app#174, generalised): opening a
 * persona's card is a catalogue row, so a context menu and the palette can open it from outside
 * this panel. The wrapper here is that window, in as few lines as the claim needs.
 */
function draw(
  on: {
    workspace?: string;
    state?: WorkspaceState;
    offers?: Catalogued;
    onPress?: (offer: Offer) => void;
    contributed?: PanelView[];
    vaults?: VaultSummary[];
    onAddTodo?: (workspace: string, text: string) => Promise<string | undefined>;
    /** A read-only project's sentence, lent as the project's window lends it (#826). */
    readOnly?: string;
  } = {},
) {
  /**
   * Every view of the right side, one under another, as the bar would show them one at a time
   * (`RegionFrame` keeps them all mounted): purlis's own, then the extensions' panels. What the
   * bar does with them is `FourRegions.test.tsx`'s; this is what each view holds.
   */
  function Window() {
    const [shownRow, setShownRow] = useState<string>();
    const st = on.state ?? state();
    const contributed = on.contributed ?? [];
    const every: ViewId[] = [
      // The Inbox is a view of its own and no panel (#1692): `Inbox.window.test.tsx` draws it.
      // The right half's views first, as the one region had them above Personas (#1738).
      ...(CATALOGUE["panel-right"].views ?? []),
      ...(CATALOGUE.aside.views ?? []).filter((view) => view !== "inbox"),
      ...attentionPanels(st, contributed).map((one) => one.view),
    ];
    return (
      <div data-testid="panels">
        {every.map((view) => (
          <Panels
            key={view}
            view={view}
            workspace={"workspace" in on ? on.workspace : "alpha"}
            state={st}
            offers={on.offers ?? new Map()}
            onPress={on.onPress ?? (() => {})}
            contributed={contributed}
            shownRow={shownRow}
            onShowRow={setShownRow}
            vaults={on.vaults === undefined ? undefined : { vaults: on.vaults, reload: () => {} }}
            onAddTodo={on.onAddTodo}
          />
        ))}
      </div>
    );
  }
  render(
    <ReadOnlySaid.Provider value={on.readOnly}>
      <Window />
    </ReadOnlySaid.Provider>,
  );
}

describe("the right-hand region", () => {
  it("holds no needs-you queue, which is the title bar's now (charter-app#249)", () => {
    draw();

    expect(within(screen.getByTestId("panels")).queryByLabelText("Needs you")).toBeNull();
    expect(screen.getByTestId("panels").querySelector(".needs-you")).toBeNull();
  });

  it("draws in each view the one panel it is named for, and nothing of another view's (#1678)", () => {
    draw();

    const view = (name: string) => {
      const found = document.querySelector<HTMLElement>(`.panels[data-view="${name}"]`);
      if (found === null) throw new Error(`no ${name} view`);
      return within(found);
    };
    expect(view("todos").getByTestId("panel-todos")).toBeInTheDocument();
    expect(view("todos").queryByTestId("panel-personas")).toBeNull();
    expect(view("personas").getByTestId("panel-personas")).toBeInTheDocument();
    expect(view("personas").queryByTestId("panel-todos")).toBeNull();
    // Each still names the region and the workspace it draws, as the one region did.
    expect(document.querySelector('.panels[data-view="memory"]')?.getAttribute("aria-label")).toBe(
      "Attention · alpha",
    );
  });

  it("holds no alerts section, because alerts cross projects and this region is one project's", () => {
    // ADR 0038 put alerts here; the window's status line has them instead, because the plane
    // with an alert is usually not the one on screen. `Panels.tsx` argues it at length.
    draw();

    expect(screen.getByTestId("panels").textContent).not.toMatch(/alert/i);
  });

  it.each([["alpha"], [undefined]])(
    "draws the plane's vaults with workspace %s focused, since a vault is the plane's",
    async (workspace) => {
      // A vault is registered once per plane, so it is not one workspace's.
      draw({
        workspace,
        vaults: [
          {
            name: "ops",
            provider: "keyring",
            count: 1,
            health: { ok: true, detail: "" },
            reads_on_open: false,
          },
        ],
      });

      const vaults = await screen.findByTestId("panel-vaults");
      expect(within(vaults).getByRole("heading")).toHaveTextContent("Vaults");
      expect(await within(vaults).findByText("ops")).toBeInTheDocument();
    },
  );

  it("draws no workspace answers when none is focused", () => {
    draw({ workspace: undefined });

    // Each view that needs a workspace says so, since it is the one on screen when it is open.
    expect(screen.getAllByText("No workspace focused.")).toHaveLength(4);
    expect(screen.queryByTestId("panel-todos")).toBeNull();
  });

  it("draws the project's personas at the project root, where the other views say what it is (#1686)", () => {
    const personas = personasPanel(["devops", "steward"], "steward");
    const at = (view: ViewId) =>
      render(
        <Panels
          view={view}
          workspace={undefined}
          state={state()}
          offers={new Map()}
          onPress={() => {}}
          contributed={[]}
          shownRow={undefined}
          onShowRow={() => {}}
          atRoot
          rootPanels={[personas]}
        />,
      );

    at("personas");
    const view = within(screen.getByTestId("panel-personas"));
    expect(view.getByText("devops")).toBeInTheDocument();
    expect(view.getByText("steward")).toBeInTheDocument();
    expect(screen.queryByText(/The project root is not a workspace/)).toBeNull();
    cleanup();

    at("todos");
    expect(screen.getByText(/The project root is not a workspace/)).toBeInTheDocument();
    expect(screen.queryByTestId("panel-personas")).toBeNull();
  });

  it("draws the project's personas with no workspace focused and the root not focused (#1686)", () => {
    const personas = personasPanel(["devops", "steward"], "steward");
    const at = (view: ViewId) =>
      render(
        <Panels
          view={view}
          workspace={undefined}
          state={state()}
          offers={new Map()}
          onPress={() => {}}
          contributed={[]}
          shownRow={undefined}
          onShowRow={() => {}}
          rootPanels={[personas]}
        />,
      );

    at("personas");
    expect(within(screen.getByTestId("panel-personas")).getByText("devops")).toBeInTheDocument();
    expect(screen.queryByText("No workspace focused.")).toBeNull();
    cleanup();

    // A view that is a workspace's still says none is focused.
    at("todos");
    expect(screen.getByText("No workspace focused.")).toBeInTheDocument();
    expect(screen.queryByTestId("panel-personas")).toBeNull();
  });

  it("no longer draws repos or CI, which are state and went to the bottom bar", () => {
    draw();

    expect(screen.queryByText("svc")).toBeNull();
    expect(screen.getByTestId("panels").textContent).not.toMatch(/\bCI\b/);
  });

  it("says when the core refused the workspace outright", () => {
    draw({ state: state({ panels: undefined, trouble: "no workspace 'alpha'" }) });

    const alerts = screen.getAllByRole("alert");
    expect(alerts.length).toBeGreaterThan(0);
    for (const alert of alerts) expect(alert).toHaveTextContent("no workspace 'alpha'");
  });

  it("says the plane is still being read rather than saying there is nothing to do", () => {
    // An unanswered ask and an empty answer are the two states this must never merge.
    draw({ state: state({ panels: undefined }) });

    expect(screen.getAllByText(/Reading the project/).length).toBeGreaterThan(0);
    expect(screen.queryByText("Nothing to do")).toBeNull();
  });
});

// ---------------------------------------------------------------------------------------
// What the contract bought: the same renderer for charter's panels and a stranger's
// ---------------------------------------------------------------------------------------

describe("a panel", () => {
  it("draws whatever is contributed, each extension's panel a view after purlis's own (#1678)", () => {
    // charter's todos at 10, an extension's at 15, charter's personas at 20. Each is a view of
    // its own now, and an extension's view goes after purlis's on the bar whatever its `order`,
    // so approving one never moves an icon a person already knows; `order` sorts the
    // extensions' views among themselves.
    draw({
      contributed: [
        contributedPanel(),
        contributedPanel({ key: "ext/acme/early", title: "Early", order: 5 }),
      ],
    });

    const headings = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
    expect(headings).toEqual(["Todos", "Personas", "Early · acme", "Reviews · acme"]);
  });

  it("says whose it is, which is what ADR 0041 item 5 asks the window for", () => {
    // *Show what is in force, after approval and not only at it.* An operator has to be able
    // to tell a panel his own charter draws from one a stranger's extension contributed,
    // without opening a dialog to find out.
    draw({ contributed: [contributedPanel()] });

    expect(screen.getByTestId("panel-ext-acme-reviews")).toHaveAttribute("data-panel-from", "acme");
    expect(screen.getByTestId("panel-todos")).toHaveAttribute("data-panel-from", "charter");
    expect(within(screen.getByTestId("panel-ext-acme-reviews")).getByText(/acme/)).toBeVisible();
  });

  it("heads every section with the same title, charter's, a stranger's and the vaults' alike", () => {
    // A title that reads as a row is what made the region one long list; the fix holds only if
    // no section, however it is contributed, is drawn outside `PanelSection`.
    draw({
      contributed: [contributedPanel()],
      vaults: [
        {
          name: "ops",
          provider: "keyring",
          count: 1,
          health: { ok: true, detail: "" },
          reads_on_open: false,
        },
      ],
    });

    const sections = screen.getByTestId("panels").querySelectorAll(".panels > section");
    expect(sections).toHaveLength(4);
    // One heading each: a view's own title (#3340), or the section label where it is not one.
    for (const section of sections) {
      const heading = within(section as HTMLElement).getByRole("heading", { level: 2 });
      if (section.hasAttribute("data-frame"))
        expect(heading).toHaveClass("ui-text-body", "ui-text-strong");
      else expect(heading).toHaveAttribute("data-part", "title");
    }
  });

  it("gives a contributed panel the list primitive, which is the test of the contract", async () => {
    // **The whole claim, in one test.** An extension declared rows in a manifest and got the
    // shortening, the card, the bound, the load-more and the search — none of which it asked
    // for, wrote, or can see. If this passes, the contract is worth having; if a contributed
    // panel needed one line of its own in `Panels.tsx` to get any of it, it is not.
    const rows = Array.from({ length: 20 }, (_, at) => row(`k${at}`, `Review ${at}`));
    draw({ contributed: [contributedPanel({ blocks: [listOf(rows)] })] });

    const panel = screen.getByTestId("panel-ext-acme-reviews");
    expect(within(panel).getByRole("searchbox")).toBeInTheDocument();
    expect(within(panel).getByRole("button", { name: /Show \d+ more/ })).toBeInTheDocument();
    expect(within(panel).getAllByRole("row").length).toBeLessThan(rows.length);
  });

  it("runs a row's catalogue verb through the catalogue, and an unknown id through nothing", async () => {
    // A row cannot invent a verb: what it names is looked up, and an id the catalogue has
    // stopped offering runs nothing rather than something else. That is `Doer`'s rule for the
    // bar's buttons, applied to a panel — and it is what keeps `actions.ts` the one list even
    // though a row now names one of its members by id.
    const pressed: Offer[] = [];
    draw({
      offers: everyOffer(),
      onPress: (offer) => pressed.push(offer),
      contributed: [
        contributedPanel({
          blocks: [listOf([row("a", "Invented", { runs: "acme.do-a-thing" })])],
        }),
      ],
    });

    await userEvent.click(within(screen.getByTestId("panel-ext-acme-reviews")).getByRole("row"));

    expect(pressed).toEqual([]);
  });

  it("draws a block charter could not fill as charter's own sentence, never as an empty list", () => {
    // A store purlis would not read is not a workspace with nothing to do, and "Nothing to
    // do" is exactly what an empty list would claim about it.
    draw({
      state: state({
        panels: { ...PANELS, contributed: [todosPanel([], "todos/ is a link out of the plane")] },
      }),
    });

    expect(screen.getByRole("alert")).toHaveTextContent("todos/ is a link out of the plane");
  });

  it("says the panel's own empty sentence rather than a shared one", () => {
    draw({
      state: state({
        panels: { ...PANELS, contributed: [todosPanel([]), personasPanel([], null)] },
      }),
    });

    expect(screen.getByText("Nothing to do")).toBeInTheDocument();
    expect(screen.getByText("No personas on this plane")).toBeInTheDocument();
  });
});

/** One list block, for a fixture that wants different rows. */
function listOf(rows: PanelRow[]) {
  return {
    kind: "list" as const,
    rows,
    empty: { headline: "Nothing to review", body: null, offer: null },
  };
}

/** The whole catalogue for this plane, which is what a row's verb is looked up in. */
function everyOffer(): Catalogued {
  return catalogued(
    catalogue({
      tabs: noTabs(),
      workspaces: [],
      plane: PLANE,
      personas: ["devops", "steward"],
      views: [STATISTICS],
      needsYou: [],
      nameOf: String,
    }),
  );
}

// ---------------------------------------------------------------------------------------
// charter's own two, now that they arrive as contributions
// ---------------------------------------------------------------------------------------

describe("charter's own panels", () => {
  it("shows the focused workspace's open todos", () => {
    draw();

    expect(
      within(screen.getByTestId("panel-todos")).getByText(/Review the rollout plan/),
    ).toBeInTheDocument();
  });

  it("draws a persona as its badge and its name, and nothing else on the line (#1674)", () => {
    // The persona's tab holds the rest: how much it remembers and its Statistics. The
    // default persona keeps its star beside the name, and the word is the row's hover.
    draw();

    // The rows, and not the names of the groups they are under (#3483).
    const personas = within(screen.getByTestId("panel-personas"));
    const devops = personas.getByRole("row", { name: "devops" });
    const steward = personas.getByRole("row", { name: "steward" });
    expect(devops).toHaveTextContent(/^devops$/);
    expect(steward).toHaveTextContent(/^steward$/);
    expect(within(screen.getByTestId("panel-personas")).queryByText(/memories/)).toBeNull();
    expect(steward).toHaveClass("is-default");
    expect(devops).not.toHaveClass("is-default");
    expect(tipOf(steward)).toEqual("default · 2 memories");
  });
});

describe("rows on the right are one line (#1674)", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date(2026, 2, 3, 12, 0));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("draws a todo as its title, its date on hover, under the day it was written", () => {
    draw();

    const todos = screen.getByTestId("panel-todos");
    const todo = within(todos).getByRole("row", { name: "Review the rollout plan" });
    expect(todo).toHaveTextContent(/^Review the rollout plan$/);
    expect(tipOf(todo)).toEqual("2026-03-02");
    expect(within(todos).getByRole("rowheader", { name: "Yesterday" })).toBeInTheDocument();
    expect(within(todos).getByRole("rowgroup", { name: "Yesterday" })).toContainElement(todo);
  });

  it("draws a memory as its title and its date short at its end, under the day it was written", () => {
    draw({ state: withMemory(), offers: crudOffers() });

    const memory = within(screen.getByTestId("panel-memory")).getByRole("row", {
      name: "The API returns 418 on Mondays",
    });
    // Its title, and its date short at its end (#3340, the prototype's Q1 A); the whole date is
    // its hover.
    expect(memory).toHaveTextContent(/^The API returns 418 on Mondays03-02$/);
    expect(memory.querySelector(".row-date")).toHaveTextContent("03-02");
    expect(tipOf(memory)).toEqual("2026-03-02 09:12");
    expect(
      within(screen.getByTestId("panel-memory")).getByRole("rowheader", { name: "Yesterday" }),
    ).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------------------
// What a persona row opens: the persona's own tab (the operator's ruling, 2026-09-23)
// ---------------------------------------------------------------------------------------

describe("a persona's row", () => {
  it("runs the catalogue row that opens the persona's tab, and opens no card beside it", async () => {
    const pressed: Offer[] = [];
    draw({ offers: everyOffer(), onPress: (offer) => pressed.push(offer) });

    await userEvent.click(
      within(screen.getByTestId("panel-personas")).getByRole("row", { name: /devops/ }),
    );

    expect(pressed.map((offer) => offer.id)).toEqual(["persona.show:devops"]);
    expect(pressed[0].does).toEqual({
      verb: "openView",
      view: { from: null, view: "persona", key: "devops" },
      title: "devops",
    });
    // A card beside the row would be a second surface for the same persona.
    expect(screen.queryByTestId("row-detail-devops")).toBeNull();
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

/**
 * Right-click on a persona row (charter-app#174).
 *
 * **In jsdom, because a WebDriver right-click sends no `contextmenu` event** — measured on both
 * engines and recorded in `docs/ui-primitives.md`. What is asserted is the wiring: the row has
 * charter's own menu and it lists the catalogue's row for that persona.
 *
 * **And it is charter's own panel that has one.** A contributed panel's rows get no context
 * menu, because a menu names what a row is ABOUT and a row is words, a key and at most a
 * catalogue id — see `PanelList`'s `RowMenu`. That asymmetry is the contract's, written down
 * rather than discovered.
 */
describe("a persona row's menu", () => {
  /** Right-clicks a row, the way a WebView's own pointer does. */
  function rightClick(testid: string, name: string) {
    const row = within(screen.getByTestId(testid)).getByRole("row", {
      name: new RegExp(name),
    });
    row.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
  }

  it("opens on a right-click with the catalogue's row for that persona", async () => {
    draw({ offers: everyOffer() });

    rightClick("panel-personas", "devops");

    const menu = await screen.findByRole("menu");
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual([
      // Short verbs: the menu is about devops and says so once, as its own name.
      "Open",
      "Edit persona.md",
      "Set profile…",
      "New persona…",
      "Delete…",
    ]);
    expect(menu).toHaveAccessibleName("Persona devops");
  });

  it("hands back the row that opens the persona's tab", async () => {
    const pressed: Offer[] = [];
    draw({ offers: everyOffer(), onPress: (offer) => pressed.push(offer) });

    rightClick("panel-personas", "devops");
    await screen.findByRole("menu");
    await userEvent.click(screen.getByRole("menuitem", { name: "Open" }));

    expect(pressed.map((offer) => offer.id)).toEqual(["persona.show:devops"]);
    expect(pressed[0].does).toEqual({
      verb: "openView",
      view: { from: null, view: "persona", key: "devops" },
      title: "devops",
    });
  });

  it("gives a contributed panel's row no menu, which is the contract's asymmetry", async () => {
    draw({ offers: everyOffer(), contributed: [contributedPanel()] });

    rightClick("panel-ext-acme-reviews", "Land the panel");

    await expect(vi.waitFor(() => screen.getByRole("menu"), { timeout: 200 })).rejects.toThrow();
  });
});

// ---------------------------------------------------------------------------------------
// Persona statistics: an extension's view, asked through the executor (ADR 0041 stage 2)
// ---------------------------------------------------------------------------------------

/** The view an approved persona statistics extension offers. */
const STATISTICS: ExtensionView = {
  extension: "persona-statistics",
  id: "statistics",
  title: "Statistics",
  about: "personas",
};

describe("persona statistics", () => {
  it("are not on the personas heading: they are in each persona's tab (#1674)", () => {
    // `Views.test.tsx` holds the tab's half: Statistics beside a persona's own view, opened
    // about that persona. The palette still lists the whole project's.
    draw({ offers: everyOffer() });

    for (const panel of ["panel-personas", "panel-todos"]) {
      expect(
        within(screen.getByTestId(panel)).queryByRole("button", { name: "Statistics" }),
      ).toBeNull();
    }
  });
});

// ---------------------------------------------------------------------------------------
// Making and deleting from the panels (SI-3)
// ---------------------------------------------------------------------------------------

/** The catalogue with a focused workspace, its todos and a vault: every SI-3 row there is. */
function crudOffers(): Catalogued {
  return catalogued(
    catalogue({
      tabs: noTabs(),
      workspaces: ["alpha"],
      focused: "alpha",
      plane: PLANE,
      personas: ["devops", "steward"],
      vaults: ["ops"],
      todos: PANELS.todos,
      needsYou: [],
      nameOf: String,
    }),
  );
}

function rightClickRow(testid: string, name: string) {
  within(screen.getByTestId(testid))
    .getByRole("row", { name: new RegExp(name) })
    .dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
}

describe("the Todos panel's box", () => {
  it("says which workspace a todo goes to, and records it there", async () => {
    const added: string[] = [];
    draw({
      onAddTodo: async (workspace, text) => {
        added.push(`${workspace}:${text}`);
        return undefined;
      },
    });

    const box = within(screen.getByTestId("panel-todos")).getByRole("textbox", {
      name: "New todo in alpha",
    });
    await userEvent.type(box, "Cut the release{Enter}");

    expect(added).toEqual(["alpha:Cut the release"]);
    expect(box).toHaveValue("");
  });

  it("keeps the words and says the core's refusal when the todo is refused", async () => {
    draw({ onAddTodo: async () => "already on the list: Cut the release" });

    const panel = screen.getByTestId("panel-todos");
    const box = within(panel).getByRole("textbox", { name: "New todo in alpha" });
    await userEvent.type(box, "cut the release{Enter}");

    expect(await within(panel).findByRole("alert")).toHaveTextContent("already on the list");
    expect(box).toHaveValue("cut the release");
  });

  it("sends nothing for a box with no words in it", async () => {
    const added: string[] = [];
    draw({
      onAddTodo: async (_workspace, text) => {
        added.push(text);
        return undefined;
      },
    });

    await userEvent.type(
      within(screen.getByTestId("panel-todos")).getByRole("textbox", { name: "New todo in alpha" }),
      "   {Enter}",
    );

    expect(added).toEqual([]);
  });
});

describe("a todo row's menu", () => {
  it("offers to open it and mark it done above the line and to forget it below", async () => {
    const pressed: Offer[] = [];
    draw({ offers: crudOffers(), onPress: (offer) => pressed.push(offer) });

    rightClickRow("panel-todos", "Review the rollout plan");
    const menu = await screen.findByRole("menu");
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual(["Open", "Mark done", "Forget"]);

    await userEvent.click(screen.getByRole("menuitem", { name: "Mark done" }));
    expect(pressed.map((offer) => offer.does)).toEqual([
      { verb: "closeTodo", workspace: "alpha", slug: "20260302-091400-review" },
    ]);
  });
});

/** Picks one of the Todos view's tabs (#3358). */
async function showTodos(tab: "Open" | "Done" | "All") {
  await userEvent.click(
    within(screen.getByTestId("panel-todos")).getByRole("radio", { name: tab }),
  );
}

describe("a todo's box and a done todo (R131, ADR 0106)", () => {
  const DONE = row("done/20260301-090000-ship", "Ship the importer", {
    mark: "todo",
    note: "2026-03-01",
    detail: { kind: "text", text: "Shipped." },
  });
  const withDone = (): WorkspaceState =>
    state({
      panels: {
        ...PANELS,
        done_todos: [{ slug: "20260301-090000-ship", title: "Ship the importer", stamp: "" }],
        contributed: [todosPanel([TODO, DONE]), personasPanel(["devops", "steward"], "steward")],
      },
    });
  const offers = () =>
    catalogued(
      catalogue({
        tabs: noTabs(),
        workspaces: ["alpha"],
        focused: "alpha",
        plane: PLANE,
        personas: ["devops", "steward"],
        vaults: ["ops"],
        todos: PANELS.todos,
        doneTodos: [{ slug: "20260301-090000-ship", title: "Ship the importer" }],
        needsYou: [],
        nameOf: String,
      }),
    );

  it("ticks an open todo's box to mark it done", async () => {
    const pressed: Offer[] = [];
    draw({ state: withDone(), offers: offers(), onPress: (offer) => pressed.push(offer) });

    const box = within(screen.getByTestId("panel-todos")).getByRole("checkbox", {
      name: "Mark done: Review the rollout plan",
    });
    expect(box).not.toBeChecked();
    await userEvent.click(box);

    expect(pressed.map((offer) => offer.does)).toEqual([
      { verb: "closeTodo", workspace: "alpha", slug: "20260302-091400-review" },
    ]);
  });

  it("draws a done todo after the open ones, ticked and struck through, and unticks it to reopen", async () => {
    const pressed: Offer[] = [];
    draw({ state: withDone(), offers: offers(), onPress: (offer) => pressed.push(offer) });
    await showTodos("All");

    const panel = within(screen.getByTestId("panel-todos"));
    const done = panel.getByTestId("panel-row-done/20260301-090000-ship");
    expect(done).toHaveClass("is-checked");
    // Its words step back to the quiet tier, the meta's, by their class (#3917): a colour App.css
    // set on them lost to the body role's later layer, and only the strike drew.
    wears(done.querySelector(".row-text"), "meta", "a done todo's words");
    expect(drawnWith(done.querySelector(".row-text") as Element, "text-decoration")).toBe(
      "line-through",
    );
    expect(panel.getByTestId("panel-row-20260302-091400-review")).not.toHaveClass("is-checked");
    // Under a group of its own, after the days the open ones are under.
    const groups = panel.getAllByRole("rowgroup");
    expect(groups.at(-1)).toBe(panel.getByRole("rowgroup", { name: "Done" }));
    expect(within(groups.at(-1) as HTMLElement).getByText("Ship the importer")).toBeInTheDocument();

    const box = panel.getByRole("checkbox", { name: "Reopen todo: Ship the importer" });
    expect(box).toBeChecked();
    await userEvent.click(box);

    expect(pressed.map((offer) => offer.does)).toEqual([
      { verb: "reopenTodo", workspace: "alpha", slug: "20260301-090000-ship" },
    ]);
  });

  it("offers Reopen above the line and Delete below it on a done todo's menu", async () => {
    const pressed: Offer[] = [];
    draw({ state: withDone(), offers: offers(), onPress: (offer) => pressed.push(offer) });
    await showTodos("Done");

    rightClickRow("panel-todos", "Ship the importer");
    const menu = await screen.findByRole("menu");
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual(["Reopen", "Delete"]);

    await userEvent.click(screen.getByRole("menuitem", { name: "Delete" }));
    expect(pressed.map((offer) => offer.does)).toEqual([
      { verb: "forgetDoneTodo", workspace: "alpha", slug: "20260301-090000-ship" },
    ]);
  });
});

describe("the Todos view's tabs (#3358)", () => {
  const DONE = row("done/20260301-090000-ship", "Ship the importer", { mark: "todo" });
  const both = (): WorkspaceState =>
    state({
      panels: {
        ...PANELS,
        contributed: [todosPanel([TODO, DONE]), personasPanel(["devops", "steward"], "steward")],
      },
    });
  const titles = () =>
    within(screen.getByTestId("panel-todos"))
      .queryAllByRole("row")
      .map((one) => one.textContent ?? "");

  it("lists the open todos first, then the done ones, then both, as Open, Done and All", async () => {
    draw({ state: both() });

    const tabs = within(screen.getByTestId("panel-todos")).getByRole("radiogroup", {
      name: "Show the todos that are",
    });
    expect(
      within(tabs)
        .getAllByRole("radio")
        .map((one) => one.getAttribute("value")),
    ).toEqual(["open", "done", "all"]);
    expect(within(tabs).getByRole("radio", { name: "Open" })).toBeChecked();
    expect(titles().join("|")).toContain("Review the rollout plan");
    expect(titles().join("|")).not.toContain("Ship the importer");

    await showTodos("Done");
    expect(titles().join("|")).toContain("Ship the importer");
    expect(titles().join("|")).not.toContain("Review the rollout plan");

    await showTodos("All");
    expect(titles().join("|")).toContain("Review the rollout plan");
    expect(titles().join("|")).toContain("Ship the importer");
  });

  it("says nothing is done yet on Done when no todo is", async () => {
    draw();

    await showTodos("Done");

    expect(
      within(screen.getByTestId("panel-todos")).getByText("Nothing done yet"),
    ).toBeInTheDocument();
  });
});

describe("the + on a heading", () => {
  it("makes a persona from the Personas panel", async () => {
    const pressed: Offer[] = [];
    draw({ offers: crudOffers(), onPress: (offer) => pressed.push(offer) });

    await userEvent.click(
      within(screen.getByTestId("panel-personas")).getByRole("button", { name: "New persona…" }),
    );

    expect(pressed.map((offer) => offer.id)).toEqual(["persona.create"]);
  });

  it("makes a vault from the Vaults section", async () => {
    const pressed: Offer[] = [];
    draw({
      offers: crudOffers(),
      onPress: (offer) => pressed.push(offer),
      vaults: [
        {
          name: "ops",
          provider: "keyring",
          count: 1,
          health: { ok: true, detail: "" },
          reads_on_open: false,
        },
      ],
    });

    await userEvent.click(
      within(screen.getByTestId("panel-vaults")).getByRole("button", { name: "New vault…" }),
    );

    expect(pressed.map((offer) => offer.id)).toEqual(["vault.create"]);
  });
});

describe("a vault row's menu", () => {
  it("offers to delete the vault, below the line", async () => {
    const pressed: Offer[] = [];
    draw({
      offers: crudOffers(),
      onPress: (offer) => pressed.push(offer),
      vaults: [
        {
          name: "ops",
          provider: "keyring",
          count: 1,
          health: { ok: true, detail: "" },
          reads_on_open: false,
        },
      ],
    });

    rightClickRow("panel-vaults", "ops");
    const menu = await screen.findByRole("menu");
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual(["Open", "New vault…", "Delete…"]);

    await userEvent.click(screen.getByRole("menuitem", { name: "Delete…" }));
    expect(pressed.map((offer) => offer.does)).toEqual([{ verb: "removeVault", vault: "ops" }]);
  });
});

// ---------------------------------------------------------------------------------------
// The workspace's Memory section, and the shared row (SI-9c, ADR 0065 Q5, Q6, Q9)
// ---------------------------------------------------------------------------------------

const MEMORY_SLUG = "20260302-091200-the-api-returns-418-on-mondays";

/** The Memory section as `panels.rs`'s `memory_panel` builds it for `alpha`. */
function memoryPanel(rows: PanelRow[]): PanelView {
  return {
    key: "charter/memory",
    title: "Memory",
    order: 15,
    mark: "note",
    from: null,
    about: null,
    blocks: [
      {
        kind: "list",
        rows,
        empty: { headline: "Nothing remembered yet", body: null, offer: null },
      },
    ],
  };
}

const MEMORY_ROW = row(MEMORY_SLUG, "The API returns 418 on Mondays", {
  mark: "note",
  note: "2026-03-02 09:12",
  detail: { kind: "text", text: "Every Monday, from the gateway." },
  runs: `memory.open:workspace/alpha/${MEMORY_SLUG}`,
});

/** The shared row `panels.rs`'s `shared_row` puts at the foot of the Personas panel. */
const SHARED_ROW = row("_shared", "shared", {
  mark: "note",
  note: "1 memory",
  runs: "memory.shared",
});

function withMemory(): WorkspaceState {
  const personas = personasPanel(["devops", "steward"], "steward");
  const list = personas.blocks[0];
  if (list.kind === "list") list.rows.push(SHARED_ROW);
  return state({
    panels: {
      ...PANELS,
      contributed: [todosPanel([TODO]), memoryPanel([MEMORY_ROW]), personas],
    },
  });
}

describe("the workspace's Memory section", () => {
  it("is drawn first in the Panel's right half, with the focused workspace's memories", () => {
    draw({ state: withMemory(), offers: crudOffers() });

    const sections = [...screen.getByTestId("panels").querySelectorAll("section[data-testid]")].map(
      (one) => one.getAttribute("data-testid"),
    );
    expect(sections.slice(0, 2)).toEqual(["panel-memory", "panel-todos"]);
    expect(
      within(screen.getByTestId("panel-memory")).getByText("The API returns 418 on Mondays"),
    ).toBeInTheDocument();
  });

  it("is not drawn at the plane root, which has no journal, and the root says so", () => {
    draw({ workspace: undefined, state: withMemory() });

    expect(screen.queryByTestId("panel-memory")).toBeNull();
    expect(screen.queryByText("The API returns 418 on Mondays")).toBeNull();
  });

  it("previews a memory in its tab on a click, and keeps it on a double-click", async () => {
    const pressed: Offer[] = [];
    draw({ state: withMemory(), offers: crudOffers(), onPress: (offer) => pressed.push(offer) });
    const memory = within(screen.getByTestId("panel-memory")).getByRole("row", {
      name: /The API returns 418/,
    });

    await userEvent.click(memory);
    await userEvent.dblClick(memory);

    const ref = { scope: { kind: "workspace", name: "alpha" }, slug: MEMORY_SLUG };
    const title = "The API returns 418 on Mondays";
    expect(pressed[0].id).toBe(`memory.open:workspace/alpha/${MEMORY_SLUG}`);
    expect(pressed[0].does).toEqual({ verb: "openMemory", ref, title, keep: false });
    expect(pressed.at(-1)?.does).toEqual({ verb: "openMemory", ref, title, keep: true });
    // A row that runs opens no card.
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("has Open and Edit above the line and Delete below it on a row's menu", async () => {
    const pressed: Offer[] = [];
    draw({ state: withMemory(), offers: crudOffers(), onPress: (offer) => pressed.push(offer) });

    rightClickRow("panel-memory", "The API returns 418");
    const menu = await screen.findByRole("menu");
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual(["Open", "Edit", "Delete"]);

    await userEvent.click(screen.getByRole("menuitem", { name: "Delete" }));
    expect(pressed.map((offer) => offer.does.verb)).toEqual(["archiveMemory"]);
  });

  it("makes a memory in the focused workspace from the + on its heading", async () => {
    const pressed: Offer[] = [];
    draw({ state: withMemory(), offers: crudOffers(), onPress: (offer) => pressed.push(offer) });

    await userEvent.click(
      within(screen.getByTestId("panel-memory")).getByRole("button", {
        name: "New memory in alpha…",
      }),
    );

    expect(pressed.map((offer) => offer.does)).toEqual([
      { verb: "newMemory", scope: { kind: "workspace", name: "alpha" } },
    ]);
  });
});

describe("the shared row in Personas", () => {
  it("says how much is shared and opens the shared list, not a persona", async () => {
    const pressed: Offer[] = [];
    draw({ state: withMemory(), offers: crudOffers(), onPress: (offer) => pressed.push(offer) });

    const shared = within(screen.getByTestId("panel-personas")).getByRole("row", {
      name: /^shared/,
    });
    expect(shared).toHaveTextContent(/^shared$/);
    expect(tipOf(shared)).toEqual("1 memory");
    await userEvent.click(shared);

    expect(pressed.map((offer) => offer.id)).toEqual(["memory.shared"]);
  });

  it("has no persona's menu, because the shared store is not a persona", async () => {
    draw({ state: withMemory(), offers: crudOffers() });

    rightClickRow("panel-personas", "^shared");

    await expect(vi.waitFor(() => screen.getByRole("menu"), { timeout: 200 })).rejects.toThrow();
  });
});

describe("the Todos box on a read-only project (#826)", () => {
  const SENTENCE =
    "this project requires the feature memory-proposals, which purlis 0.8.0 does not have; " +
    "purlis 0.9.0 or later has it. purlis opens the project read-only and writes nothing to it.";

  it("is off with the project's sentence, and records nothing", async () => {
    const added: string[] = [];
    draw({
      readOnly: SENTENCE,
      onAddTodo: async (workspace, text) => {
        added.push(`${workspace}:${text}`);
        return undefined;
      },
    });

    const box = within(screen.getByTestId("panel-todos")).getByRole("textbox", {
      name: "New todo in alpha",
    });
    expect(box).toBeDisabled();
    expect(await hoverTipOf(box)).toEqual(SENTENCE);
    await userEvent.type(box, "Cut the release{Enter}");
    expect(added).toEqual([]);
  });
});

// ---------------------------------------------------------------------------------------
// The panels' words, each on its text role (#1920)
// ---------------------------------------------------------------------------------------

describe("a panel's words, each on its text role (#1920)", () => {
  it("draws a row's words as a body, and the default's at the strong weight", () => {
    draw({ state: withMemory(), offers: crudOffers() });

    const memory = screen.getByTestId("panel-memory");
    expect(memory.closest(".panels")).toHaveClass("ui-text-body");
    wears(memory.querySelector(".row-text"), "body", "a memory's title");

    const personas = screen.getByTestId("panel-personas");
    const [devops, steward] = [...personas.querySelectorAll(".panel-row")];
    wears(devops.querySelector(".row-text"), "body", "a persona's name");
    wears(steward.querySelector(".row-text"), "body", "the default persona's name", true);
  });

  it("heads a section with its title, and whose panel it is beside it as data", () => {
    draw({ contributed: [contributedPanel()] });

    // A view's title is a strong body, as the prototype's Q1 A draws it (#3340); a section that
    // is not a view keeps the one section label.
    for (const heading of screen.getByTestId("panels").querySelectorAll(".panels h2"))
      if (heading.closest("[data-frame]") !== null)
        wears(heading, "body", `the heading ${heading.textContent}`, true);
      else wears(heading, "label", `the heading ${heading.textContent}`);
    wears(
      screen.getByTestId("panels").querySelector(".panel-from"),
      "secondary",
      "whose panel it is",
    );
  });

  it("says there is nothing to show as a secondary sentence, at its full strength", () => {
    draw({ workspace: undefined });

    const [empty] = screen.getAllByText("No workspace focused.");
    wears(empty, "secondary", "the empty sentence");
    // The role's colour is what makes it quiet: no rule fades it (#3790).
    expect(drawnWith(empty, "opacity")).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------------------
// The sidebar view frame (#3340, the sidebar-views prototype's Q1 A)
// ---------------------------------------------------------------------------------------

describe("a view on the sidebar view frame (#3340)", () => {
  /** The view's top: what is held in place while the rows scroll. */
  const topOf = (testid: string) => {
    const top = screen.getByTestId(testid).querySelector<HTMLElement>('[data-part="top"]');
    if (top === null) throw new Error(`${testid} has no top`);
    return top;
  };

  it("draws every panel as a view: Memory, Todos, Sessions, Personas, Vaults and an extension's (#3341)", () => {
    const sessions: PanelView = { ...memoryPanel([]), key: "charter/sessions", title: "Sessions" };
    const memory = memoryPanel([MEMORY_ROW]);
    const personas = personasPanel(["devops", "steward"], "steward");
    draw({
      state: state({
        panels: { ...PANELS, contributed: [todosPanel([TODO]), memory, personas, sessions] },
      }),
      contributed: [contributedPanel()],
      vaults: [
        {
          name: "ops",
          provider: "keyring",
          count: 1,
          health: { ok: true, detail: "" },
          reads_on_open: false,
        },
      ],
    });

    for (const testid of [
      "panel-memory",
      "panel-todos",
      "panel-sessions",
      "panel-personas",
      "panel-vaults",
      "panel-ext-acme-reviews",
    ]) {
      expect(screen.getByTestId(testid)).toHaveAttribute("data-frame");
      expect(screen.getByTestId(testid).closest("aside")).toHaveClass("panels-view");
    }
  });

  it("holds the filter of Personas, Vaults and an extension's panel in the top, by its one title (#3341)", () => {
    draw({
      contributed: [contributedPanel()],
      vaults: [
        {
          name: "ops",
          provider: "keyring",
          count: 1,
          health: { ok: true, detail: "" },
          reads_on_open: false,
        },
      ],
    });

    // A list of things named is filtered, as the prototype's Q1 A says it (#3341).
    for (const [testid, name, placeholder] of [
      ["panel-personas", "Filter Personas", "Filter personas"],
      ["panel-vaults", "Filter Vaults", "Filter vaults"],
      ["panel-ext-acme-reviews", "Search Reviews", "Search reviews"],
    ]) {
      const top = topOf(testid);
      expect(within(top).getAllByRole("heading")).toHaveLength(1);
      expect(within(top).getByRole("searchbox", { name })).toHaveAttribute(
        "placeholder",
        placeholder,
      );
    }
  });

  it("holds the heading, its actions and the search in the top, before the rows", () => {
    draw({ state: withMemory(), offers: crudOffers() });

    const top = topOf("panel-memory");
    expect(within(top).getByRole("heading", { name: "Memory" })).toBeInTheDocument();
    expect(within(top).getByRole("button", { name: "New memory in alpha…" })).toBeInTheDocument();
    // The search is there from the first row in a view, and it is the list's own box.
    expect(within(top).getByRole("searchbox", { name: "Search Memory" })).toBeInTheDocument();
    // The rows are under the top and not in it.
    const grid = within(screen.getByTestId("panel-memory")).getByRole("grid");
    expect(top.contains(grid)).toBe(false);
    expect(top.compareDocumentPosition(grid) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("still searches the list from the box in its top", async () => {
    draw({ state: withMemory(), offers: crudOffers() });

    await userEvent.type(
      within(topOf("panel-memory")).getByRole("searchbox", { name: "Search Memory" }),
      "nothing like it",
    );

    expect(
      within(screen.getByTestId("panel-memory")).getByText(/Nothing here matches/),
    ).toBeInTheDocument();
  });

  it("holds the Todos box in the top, and no search under a page", () => {
    draw({ onAddTodo: async () => undefined });

    const top = topOf("panel-todos");
    expect(within(top).getByRole("textbox", { name: "New todo in alpha" })).toBeInTheDocument();
    expect(within(top).queryByRole("searchbox")).toBeNull();
  });

  it("reaches the heading's actions, then the field, then the rows, with Tab", async () => {
    draw({ state: withMemory(), offers: crudOffers() });
    const memory = screen.getByTestId("panel-memory");

    // From just before the view: its controls come in the order the prototype stacks them.
    const before = document.createElement("button");
    memory.before(before);
    before.focus();
    const add = within(memory).getByRole("button", { name: "New memory in alpha…" });
    while (document.activeElement !== add) await userEvent.tab();
    // Then the heading's other actions (the archive), still on its line.
    const line = add.closest('[data-part="head"]');
    while (line?.contains(document.activeElement)) await userEvent.tab();
    expect(within(memory).getByRole("searchbox", { name: "Search Memory" })).toHaveFocus();
    await userEvent.tab();
    expect(document.activeElement?.closest('[role="grid"]')).not.toBeNull();
    before.remove();
  });

  it("in a half, gives its actions to the half's line and draws no heading of its own", () => {
    const line = document.createElement("span");
    document.body.append(line);
    render(
      <HeadActions.Provider value={line}>
        <Panels
          view="memory"
          workspace="alpha"
          state={withMemory()}
          offers={crudOffers()}
          onPress={() => {}}
          contributed={[]}
          shownRow={undefined}
          onShowRow={() => {}}
        />
      </HeadActions.Provider>,
    );

    const top = topOf("panel-memory");
    expect(within(top).queryByRole("heading")).toBeNull();
    expect(within(top).getByRole("searchbox", { name: "Search Memory" })).toBeInTheDocument();
    expect(within(line).getByRole("button", { name: "New memory in alpha…" })).toBeInTheDocument();
    line.remove();
  });
});

describe("the right side's rows on the frame (#3341)", () => {
  it("draws a vault as its name and count, under its provider, with no shared mark", () => {
    draw({
      vaults: [
        {
          name: "forge",
          provider: "keyring",
          count: 3,
          health: { ok: true, detail: "" },
          reads_on_open: false,
        },
        {
          name: "devops",
          provider: "file",
          count: 1,
          health: { ok: true, detail: "" },
          reads_on_open: false,
        },
      ],
    });

    const vaults = screen.getByTestId("panel-vaults");
    const forge = within(vaults).getByRole("row", { name: /forge/ });
    expect(forge.querySelector(".row-text")).toHaveTextContent("forge");
    expect(forge.querySelector(".row-date")).toHaveTextContent("3 secrets");
    expect(forge.querySelector('[data-part="lead"]')).toBeNull();
    // Under its provider, each a group of its own.
    expect(
      within(vaults)
        .getAllByRole("rowgroup")
        .map((group) => group.getAttribute("aria-label") ?? group.textContent?.split(/\d/)[0]),
    ).toHaveLength(2);
  });

  it("marks a persona whose chat is at work with that state's own mark, and one at rest with none", () => {
    const personas = personasPanel(["devops", "steward"], "steward");
    render(
      <ChatsHere.Provider
        value={fixedChats({ ...nothingKnown, bySession: { 7: "running" }, needsYou: [] })}
      >
        <Panels
          view="personas"
          workspace="alpha"
          state={state({ panels: { ...PANELS, contributed: [personas] } })}
          offers={new Map()}
          onPress={() => {}}
          contributed={[]}
          shownRow={undefined}
          onShowRow={() => {}}
          personaChats={new Map([["steward", [7]]])}
        />
      </ChatsHere.Provider>,
    );

    const steward = screen.getByRole("row", { name: /steward/ });
    const mark = steward.querySelector(".persona-at-work .shown-state");
    expect(mark).toHaveAttribute("data-state", "working");
    // Its shape, and its word for a screen reader.
    expect(mark?.querySelector('[data-shape="ring"]')).not.toBeNull();
    expect(steward).toHaveTextContent(/working/);
    expect(
      screen.getByRole("row", { name: /devops/ }).querySelector(".persona-at-work"),
    ).toBeNull();
  });
});

describe("the Personas view's groups (#3483, the prototype's Q1 A)", () => {
  const personas = personasPanel(["devops", "ops", "release", "steward"], "steward");
  /** A session record of `persona` in the focused workspace: who has worked here. */
  const record = (persona: string) => ({
    path: `workspaces/alpha/sessions/2026-10-27-1000-${persona}.md`,
    title: `${persona}'s work`,
    when: "2026-10-27 10:00",
    persona,
    harness: null,
    resumable: false,
  });
  const drawn = (
    bySession: Record<number, State>,
    workingHere = new Set<string>(),
    { names, offers = new Map() }: { names?: string[]; offers?: Catalogued } = {},
  ) => (
    <ChatsHere.Provider value={fixedChats({ ...nothingKnown, bySession, needsYou: [] })}>
      <Panels
        view="personas"
        workspace="alpha"
        state={state({
          panels: {
            ...PANELS,
            sessions: [record("devops")],
            contributed: [names === undefined ? personas : personasPanel(names, "steward")],
          },
        })}
        offers={offers}
        onPress={() => {}}
        contributed={[]}
        shownRow={undefined}
        onShowRow={() => {}}
        personaChats={
          new Map([
            ["steward", [7]],
            ["release", [8]],
          ])
        }
        chatPersonasHere={workingHere}
      />
    </ChatsHere.Provider>
  );
  /** Each group's name and its rows' names, in the order they are drawn. */
  const groups = () =>
    screen.getAllByRole("rowgroup").map((group) => ({
      name: within(group).getByRole("rowheader").firstElementChild?.textContent,
      count: group.querySelector('[data-part="header"] [data-count]')?.textContent,
      rows: [...group.querySelectorAll('[data-part="root"] .row-text')].map(
        (row) => row.textContent,
      ),
    }));

  it("draws Working now, In this workspace and On the plane, each with its count", () => {
    render(drawn({ 7: "running" }, new Set(["ops"])));

    expect(groups()).toEqual([
      { name: "Working now", count: "1", rows: ["steward"] },
      // Who has a record here (devops) or a chat at rest here (ops).
      { name: "In this workspace", count: "2", rows: ["devops", "ops"] },
      // release's chat is at rest, and not in this workspace.
      { name: "On the plane", count: "1", rows: ["release"] },
    ]);
  });

  it("moves a persona to Working now when one of its chats starts, and back when they rest", () => {
    const { rerender } = render(drawn({}));
    expect(groups().map((group) => group.name)).toEqual(["In this workspace", "On the plane"]);

    rerender(drawn({ 8: "running" }));
    expect(groups()[0]).toEqual({ name: "Working now", count: "1", rows: ["release"] });

    rerender(drawn({ 8: "done" }));
    expect(groups().map((group) => group.name)).toEqual(["In this workspace", "On the plane"]);
  });

  describe("held still while the pointer is over the list or the keyboard is in it (#3566)", () => {
    const list = () => screen.getByTestId("list-personas");
    const rowOf = (name: string) =>
      within(list()).getByRole("row", { name: new RegExp(`^${name}`) });

    it("moves no row under the pointer, and moves it once the pointer has left", () => {
      const { rerender } = render(drawn({}));
      fireEvent.pointerEnter(list());

      // release's chat starts while the pointer is over the list.
      rerender(drawn({ 8: "running" }));
      expect(groups().map((group) => group.name)).toEqual(["In this workspace", "On the plane"]);

      fireEvent.pointerLeave(list());
      expect(groups()[0]).toEqual({ name: "Working now", count: "1", rows: ["release"] });
    });

    it("moves no row while the keyboard is in the list, and moves it once the keyboard has left", async () => {
      const { rerender } = render(drawn({}));
      act(() => rowOf("devops").focus());
      await userEvent.keyboard("{ArrowDown}");

      rerender(drawn({ 8: "running" }));
      expect(groups().map((group) => group.name)).toEqual(["In this workspace", "On the plane"]);

      act(() => (document.activeElement as HTMLElement).blur());
      expect(groups()[0]).toEqual({ name: "Working now", count: "1", rows: ["release"] });
    });

    it("waits for both the pointer and the keyboard to leave", async () => {
      const { rerender } = render(drawn({}));
      fireEvent.pointerEnter(list());
      act(() => rowOf("devops").focus());
      await userEvent.keyboard("{ArrowDown}");

      rerender(drawn({ 8: "running" }));
      fireEvent.pointerLeave(list());
      expect(groups().map((group) => group.name)).toEqual(["In this workspace", "On the plane"]);

      act(() => (document.activeElement as HTMLElement).blur());
      expect(groups()[0].name).toBe("Working now");
    });

    it("draws another workspace's groups at once, though the pointer is over the list", () => {
      const { rerender } = render(drawn({}));
      fireEvent.pointerEnter(list());
      rerender(
        <ChatsHere.Provider value={fixedChats({ ...nothingKnown, bySession: {}, needsYou: [] })}>
          <Panels
            view="personas"
            workspace="beta"
            state={state({ panels: { ...PANELS, sessions: [], contributed: [personas] } })}
            offers={new Map()}
            onPress={() => {}}
            contributed={[]}
            shownRow={undefined}
            onShowRow={() => {}}
            chatPersonasHere={new Set(["ops"])}
          />
        </ChatsHere.Provider>,
      );
      expect(groups()[0]).toEqual({ name: "In this workspace", count: "1", rows: ["ops"] });
    });

    it("is not held by the keyboard in its Filter box, which is drawn in the view's top", async () => {
      const { rerender } = render(drawn({}));
      const filter = screen.getByRole("searchbox", { name: "Filter Personas" });
      await userEvent.click(filter);
      await userEvent.keyboard("d{Backspace}");
      expect(filter).toHaveFocus();

      rerender(drawn({ 8: "running" }));
      expect(groups()[0]).toEqual({ name: "Working now", count: "1", rows: ["release"] });
    });

    it("is held while the keyboard is in a row's menu", async () => {
      const show: Offer = {
        id: "persona.show:devops",
        title: "Show devops",
        available: true,
        reason: "",
        does: { verb: "openProject" },
      };
      const offers = new Map([[show.id, show]]);
      const { rerender } = render(drawn({}, undefined, { offers }));
      act(() => rowOf("devops").focus());
      await userEvent.keyboard("{Shift>}{F10}{/Shift}");
      const menu = await screen.findByRole("menu");
      await waitFor(() => expect(menu.contains(document.activeElement)).toBe(true));

      rerender(drawn({ 8: "running" }, undefined, { offers }));
      expect(groups().map((group) => group.name)).toEqual(["In this workspace", "On the plane"]);

      // The menu closed, and the keyboard is no longer in the list: the next draw moves the row.
      await userEvent.keyboard("{Escape}");
      await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
      act(() => (document.activeElement as HTMLElement).blur());
      rerender(drawn({ 8: "running" }, undefined, { offers }));
      expect(groups()[0].name).toBe("Working now");
    });

    it("draws a persona added or removed under the hold at once, in the group it has now", () => {
      const { rerender } = render(drawn({}));
      fireEvent.pointerEnter(list());

      rerender(drawn({}, new Set(["qa"]), { names: ["devops", "ops", "qa", "steward"] }));
      expect(groups()).toEqual([
        // qa came under the hold: drawn at once, in the group it has now.
        { name: "In this workspace", count: "2", rows: ["devops", "qa"] },
        { name: "On the plane", count: "2", rows: ["ops", "steward"] },
      ]);
      // release was removed, and is gone at once.
      expect(within(list()).queryByRole("row", { name: /^release/ })).toBeNull();
    });

    it("lets go when the window is left, as a pointer parked over it sends no leave", () => {
      const { rerender } = render(drawn({}));
      fireEvent.pointerEnter(list());
      rerender(drawn({ 8: "running" }));

      act(() => {
        window.dispatchEvent(new Event("blur"));
      });
      expect(groups()[0].name).toBe("Working now");
    });
  });
});
