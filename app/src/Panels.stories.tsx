// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Memory, Todos and Sessions as `Panels` draws them** (#3340), on the sidebar view frame: a
 * side panel 276px wide whose view is its one scroll, the title and the field held at the top,
 * each day's name held under them with its count, 30px rows. A long list and a short one of each,
 * walked by the keyboard in the order the top stacks them. And the Todos view's tabs, Open, Done
 * and All, on All: done todos after the open ones, ticked and struck through (R131, ADR 0106).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useMemo, useState } from "react";
import { catalogue, catalogued } from "./actions";
import { noTabs } from "./tabs";
import { DONE_ROW } from "./todos";
import { expect, waitFor } from "storybook/test";
import { ownPanelKey, Panels } from "./Panels";
import { expectFocusInSight } from "./stories.testkit";
import type { PanelRow, PanelView, Panels as PanelsModel, VaultSummary } from "./bindings";
import { panelView, type OwnViewId } from "./sideViews";
import type { WorkspaceState } from "./workspaceState";
import { Panel, PanelTitle } from "./ui/panel/Panel";
import { HeadActions } from "./viewHead";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";

const two = (n: number) => String(n).padStart(2, "0");
/** A day `back` days before today, as the store stamps it, so the days read the same any day. */
function daysAgo(back: number): string {
  const when = new Date();
  when.setDate(when.getDate() - back);
  return `${when.getFullYear()}-${two(when.getMonth() + 1)}-${two(when.getDate())}`;
}

const TITLES = [
  "Follow main CI after every merge",
  "Merge only PRs tested on current main",
  "gh pr ready starts more checks",
  "Sandboxed agents cannot run terminal-bound tests",
  "Throttle parallel agents by machine load",
  "A copied node_modules hangs on macOS",
  "Never run an unguarded cleanup script",
  "Agent worktree targets fill the disk",
  "Worktree git config writes the shared clone",
  "Grill and decide, escalate only major calls",
  "Never idle the loop on pending asks",
  "A stale PR is yours to land",
];

/** `count` rows of `mark`, a few today, a few yesterday, the rest earlier. */
function rowsOf(count: number, mark: string): PanelRow[] {
  // A persona's row: its mark, its name, the default's star (#1449, #1674).
  if (mark === "persona")
    return PERSONAS.slice(0, count).map((name, at) => ({
      key: name,
      text: name,
      note: "2 memories",
      mark,
      tone: at === 0 ? "default" : "plain",
      detail: null,
      runs: `persona.show:${name}`,
      actions: [],
    }));
  return TITLES.slice(0, count).map((text, at) => ({
    key: `row-${at}`,
    text,
    // A few today, at a time of day, and the rest on fixed days long past: a row's short date
    // is its time today and its day before, so the picture reads the same on any day.
    note: at < 4 ? `${daysAgo(0)} ${two(9 + at)}:${two(12 + at)}` : `2026-03-${two(20 - at)}`,
    mark,
    tone: "plain",
    detail: null,
    runs: null,
    actions: [],
  }));
}

const KEYS: Record<"memory" | "todos" | "sessions" | "personas", [string, string]> = {
  memory: ["Memory", "note"],
  todos: ["Todos", "todo"],
  sessions: ["Sessions", "note"],
  personas: ["Personas", "persona"],
};

/** `count` done todos, as `panels.rs` keys them: `done/<slug>`. */
function doneOf(count: number): PanelRow[] {
  // `slice(-0)` is the whole list: none is none.
  return (count === 0 ? [] : TITLES.slice(-count)).map((text, at) => ({
    key: `${DONE_ROW}done-${at}`,
    text,
    note: daysAgo(20 + at),
    mark: "todo",
    tone: "plain",
    detail: null,
    runs: null,
    actions: [],
  }));
}

const PERSONAS = ["steward", "designer", "release", "security", "reviewer", "docs", "ops"];

function panelOf(view: keyof typeof KEYS, count: number, done = 0): PanelView {
  const [title, mark] = KEYS[view];
  return {
    key: ownPanelKey(view) ?? view,
    title,
    order: 10,
    mark,
    from: null,
    about: null,
    blocks: [
      {
        kind: "list",
        rows: [...rowsOf(count, mark), ...doneOf(done)],
        empty: { headline: `No ${title.toLowerCase()} yet`, body: null, offer: null },
      },
    ],
  };
}

function stateOf(view: keyof typeof KEYS, count: number, done = 0): WorkspaceState {
  const panels: PanelsModel = {
    workspace: "alpha",
    repos: [],
    paths: {},
    absent: [],
    refused: [],
    todos: [],
    todos_refused: null,
    done_todos: [],
    personas: [],
    persona: null,
    sessions: [],
    contributed: [panelOf(view, count, done)],
  };
  return { panels, repos: undefined, pieces: {}, piecesRefused: {}, reading: false };
}

/** The view in a side panel, its region the one scroll, as `RegionFrame` lays a view out. */
/** The plane's vaults, as `vault_list` answers: names and counts, never a value. */
const VAULTS: VaultSummary[] = ["forge", "reddit", "crates-io", "devops", "release-signing"].map(
  (name, at) => ({
    name,
    provider: at < 3 ? "keyring" : "file",
    count: at + 1,
    health: { ok: true, detail: "" },
    reads_on_open: false,
  }),
);

/**
 * The view in a half of the Panel, as the window draws one (`RegionFrame`'s `Half`): the half's
 * line holds the view's one title and its actions (#3355), and the view under it is the one
 * scroll, its top held at the top of it.
 */
/** Two personas' chats, as the Chats list hands them to the Personas view (#3341). */
const AT_WORK = new Map([
  ["steward", [1]],
  ["designer", [2]],
]);

/** Who works in the workspace (#3483): two personas whose chats there are at rest. */
const HERE = new Set(["security", "reviewer"]);

/** What the board says of them: steward's at work, designer's asking for you. */
const WORKING = fixedChats({
  ...nothingKnown,
  bySession: { 1: "running", 2: "waiting" },
  needsYou: [2],
});

/** An approved extension's panel (#3341): declared rows, its own marks, on the same frame. */
const EXTENSION: PanelView = {
  key: "ext/acme/reviews",
  title: "Reviews",
  order: 15,
  mark: "note",
  from: "acme",
  about: null,
  blocks: [
    {
      kind: "list",
      rows: ["Land the panel contract", "Review the rollout plan", "Read the audit"].map(
        (text, at) => ({
          key: `review-${at}`,
          text,
          note: null,
          mark: at === 0 ? "trouble" : "note",
          tone: "plain",
          detail: { kind: "text", text: `${text}, in full.` },
          runs: null,
          actions: [],
        }),
      ),
      empty: { headline: "Nothing to review", body: null, offer: null },
    },
  ],
};

function View({
  view,
  count,
  done = 0,
  atWork = false,
}: {
  view: keyof typeof KEYS | "vaults" | "extension";
  count: number;
  /** Done todos after the open ones, each with its box ticked (the Todos view only). */
  done?: number;
  /** Personas whose chats are at work: steward's working, designer's asking for you. */
  atWork?: boolean;
}) {
  const title =
    view === "vaults" ? "Vaults" : view === "extension" ? EXTENSION.title : KEYS[view][0];
  const [shown, setShown] = useState<string>();
  // The catalogue's Mark done and Reopen rows, which a todo's box runs: without them a row keeps
  // its mark, as it does where the catalogue offers none.
  const offers = useMemo(
    () =>
      view !== "todos" || done === 0
        ? new Map()
        : catalogued(
            catalogue({
              tabs: noTabs(),
              workspaces: ["alpha"],
              focused: "alpha",
              plane: "/home/dev/plane",
              todos: rowsOf(count, "todo").map((row) => ({ slug: row.key, title: row.text })),
              doneTodos: doneOf(done).map((row) => ({
                slug: row.key.slice(DONE_ROW.length),
                title: row.text,
              })),
              needsYou: [],
              nameOf: String,
            }),
          ),
    [view, count, done],
  );
  const [line, setLine] = useState<HTMLElement | null>(null);
  return (
    <Panel variant="bottom" aria-label={title} role="region">
      <div
        data-testid="region"
        style={{
          display: "flex",
          flexDirection: "column",
          blockSize: "22rem",
          inlineSize: "276px",
        }}
      >
        <div className="half-head">
          <PanelTitle className="side-title">{title}</PanelTitle>
          <span className="view-actions" ref={setLine} />
        </div>
        {/* A column, as the window's region view is: the view takes the half's whole width. */}
        <div style={{ display: "flex", flexDirection: "column", flex: "1 1 0", minBlockSize: 0 }}>
          <ChatsHere.Provider value={atWork ? WORKING : fixedChats()}>
            <HeadActions.Provider value={line}>
              <Panels
                view={view === "extension" ? panelView(EXTENSION.key) : (view as OwnViewId)}
                workspace="alpha"
                state={stateOf(
                  view === "vaults" || view === "extension" ? "memory" : view,
                  count,
                  done,
                )}
                offers={offers}
                onPress={() => {}}
                contributed={view === "extension" ? [EXTENSION] : []}
                shownRow={shown}
                onShowRow={setShown}
                onAddTodo={async () => undefined}
                vaults={{ vaults: VAULTS.slice(0, count), reload: () => {} }}
                personaChats={atWork ? AT_WORK : undefined}
                chatPersonasHere={atWork ? HERE : undefined}
              />
            </HeadActions.Provider>
          </ChatsHere.Provider>
        </div>
      </div>
    </Panel>
  );
}

const meta = { component: View } satisfies Meta<typeof View>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The keyboard reaches the field in the top, then what else the top holds (the Todos view's
 *  tabs), then the rows; the top stays put as they scroll. */
async function walk(
  canvasElement: HTMLElement,
  tab: () => Promise<void>,
  field: HTMLElement,
  long: boolean,
  then?: HTMLElement,
) {
  await tab();
  await expect(field).toHaveFocus();
  expectFocusInSight();
  if (then !== undefined) {
    await tab();
    await expect(then).toHaveFocus();
    expectFocusInSight();
  }
  await tab();
  await expect(document.activeElement?.closest('[role="grid"]')).not.toBeNull();
  expectFocusInSight();
  const scroller = canvasElement.querySelector<HTMLElement>(".panels");
  const top = canvasElement.querySelector<HTMLElement>('[data-part="top"]');
  if (scroller === null || top === null) throw new Error("the view has no scroll or no top");
  // One scroll: the list is no scroller of its own.
  const grid = canvasElement.querySelector<HTMLElement>('[role="grid"]');
  await expect((grid?.scrollHeight ?? 0) - (grid?.clientHeight ?? 0)).toBeLessThanOrEqual(1);
  if (long) {
    const at = top.getBoundingClientRect().top;
    scroller.scrollTop = 160;
    await waitFor(() => expect(scroller.scrollTop).toBeGreaterThan(0));
    await expect(Math.round(top.getBoundingClientRect().top)).toBe(Math.round(at));
    // Back to the start for the picture, which is then the same in every run.
    scroller.scrollTop = 0;
    await waitFor(() => expect(scroller.scrollTop).toBe(0));
  }
  // The picture is the view at rest: no row's tooltip, which comes up a while after the
  // keyboard lands on a row, so a picture taken then would differ from run to run; and the
  // held top's height measured and drawn, two frames on.
  (document.activeElement as HTMLElement | null)?.blur();
  await new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)));
}

export const MemoryLong: Story = {
  args: { view: "memory", count: 12 },
  play: async ({ canvas, canvasElement, userEvent }) => {
    await expect(canvas.getByRole("heading", { name: "Memory" })).toBeInTheDocument();
    await walk(
      canvasElement,
      () => userEvent.tab(),
      canvas.getByRole("searchbox", { name: "Search Memory" }),
      true,
    );
  },
};

export const MemoryShort: Story = {
  args: { view: "memory", count: 2 },
  play: async ({ canvas, canvasElement, userEvent }) => {
    await walk(
      canvasElement,
      () => userEvent.tab(),
      canvas.getByRole("searchbox", { name: "Search Memory" }),
      false,
    );
  },
};

export const TodosLong: Story = {
  args: { view: "todos", count: 12 },
  play: async ({ canvas, canvasElement, userEvent }) => {
    await walk(
      canvasElement,
      () => userEvent.tab(),
      canvas.getByRole("textbox", { name: "New todo in alpha" }),
      true,
      canvas.getByRole("radio", { name: "Open" }),
    );
  },
};

export const TodosShort: Story = {
  args: { view: "todos", count: 2 },
  play: async ({ canvas, canvasElement, userEvent }) => {
    await walk(
      canvasElement,
      () => userEvent.tab(),
      canvas.getByRole("textbox", { name: "New todo in alpha" }),
      false,
      canvas.getByRole("radio", { name: "Open" }),
    );
  },
};

export const TodosWithDone: Story = {
  args: { view: "todos", count: 3, done: 2 },
  play: async ({ canvas, userEvent }) => {
    // Open, Done and All in the top, under the box (#3358); Open is the one picked.
    await expect(canvas.getByRole("radio", { name: "Open" })).toBeChecked();
    await expect(canvas.getAllByRole("checkbox")).toHaveLength(3);
    await userEvent.click(canvas.getByRole("radio", { name: "All" }));
    // The open ones' boxes are clear, the done ones' ticked, and those are under Done, last.
    await expect(canvas.getAllByRole("checkbox", { checked: false })).toHaveLength(3);
    await expect(canvas.getAllByRole("checkbox", { checked: true })).toHaveLength(2);
    await expect(canvas.getAllByRole("rowgroup").at(-1)).toBe(
      canvas.getByRole("rowgroup", { name: "Done" }),
    );
  },
};

export const SessionsLong: Story = {
  args: { view: "sessions", count: 12 },
  play: async ({ canvas, canvasElement, userEvent }) => {
    await walk(
      canvasElement,
      () => userEvent.tab(),
      canvas.getByRole("searchbox", { name: "Search Sessions" }),
      true,
    );
  },
};

export const SessionsShort: Story = {
  args: { view: "sessions", count: 2 },
  play: async ({ canvas, canvasElement, userEvent }) => {
    await walk(
      canvasElement,
      () => userEvent.tab(),
      canvas.getByRole("searchbox", { name: "Search Sessions" }),
      false,
    );
  },
};

/** Personas on the frame (#3341): one title, the filter held in the top, one scroll; each row
 *  keeps its persona's mark and the default's star. */
export const Personas: Story = {
  args: { view: "personas", count: 7 },
  play: async ({ canvas, canvasElement, userEvent }) => {
    await walk(
      canvasElement,
      () => userEvent.tab(),
      canvas.getByRole("searchbox", { name: "Filter Personas" }),
      false,
    );
  },
};

/** Personas at work (#3341): a persona's row ends with its chats' state, as its own mark — the
 *  ring of one at work, the hand of one asking for you; one at rest shows none. And the rows
 *  under Working now, In this workspace and On the plane, each with its count (#3483). */
export const PersonasAtWork: Story = {
  args: { view: "personas", count: 7, atWork: true },
  play: async ({ canvas }) => {
    const mark = (name: string) =>
      canvas
        .getByRole("row", { name: new RegExp(`^${name}`) })
        .querySelector(".persona-at-work .shown-state");
    await expect(mark("steward")).toHaveAttribute("data-state", "working");
    await expect(mark("designer")).toHaveAttribute("data-state", "needs-you");
    await expect(mark("release")).toBeNull();
    const groups = canvas.getAllByRole("rowheader").map((head) => head.textContent);
    await expect(groups).toEqual(["Working now2", "In this workspace2", "On the plane3"]);
  },
};

/** Vaults on the frame (#3341): its filter held in the top, one scroll. */
export const Vaults: Story = {
  args: { view: "vaults", count: 5 },
  play: async ({ canvas, canvasElement, userEvent }) => {
    await walk(
      canvasElement,
      () => userEvent.tab(),
      canvas.getByRole("searchbox", { name: "Filter Vaults" }),
      false,
    );
  },
};

/** An extension's panel on the frame (#3341): its one title on the half's head, whose panel it is
 *  beside it, the search held in the top, its rows with the marks it declared. */
export const ExtensionPanel: Story = {
  args: { view: "extension", count: 3 },
  play: async ({ canvas, canvasElement, userEvent }) => {
    await expect(canvas.getByRole("heading", { name: "Reviews" })).toBeInTheDocument();
    await walk(
      canvasElement,
      () => userEvent.tab(),
      canvas.getByRole("searchbox", { name: "Search Reviews" }),
      false,
    );
  },
};
