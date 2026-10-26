// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Memory, Todos and Sessions as `Panels` draws them** (#3340), on the sidebar view frame: a
 * side panel 276px wide whose view is its one scroll, the title and the field held at the top,
 * each day's name held under them with its count, 30px rows. A long list and a short one of each,
 * walked by the keyboard in the order the top stacks them. And the Todos view with done todos
 * after the open ones, ticked and struck through (R131, ADR 0106).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useMemo, useState } from "react";
import { catalogue, catalogued } from "./actions";
import { noTabs } from "./tabs";
import { DONE_ROW } from "./todos";
import { expect, waitFor } from "storybook/test";
import { ownPanelKey, Panels } from "./Panels";
import { expectFocusInSight } from "./stories.testkit";
import type { PanelRow, PanelView, Panels as PanelsModel } from "./bindings";
import type { OwnViewId } from "./sideViews";
import type { WorkspaceState } from "./workspaceState";
import { Panel, PanelTitle } from "./ui/panel/Panel";
import { HeadActions } from "./viewHead";

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

const KEYS: Record<"memory" | "todos" | "sessions", [string, string]> = {
  memory: ["Memory", "note"],
  todos: ["Todos", "todo"],
  sessions: ["Sessions", "note"],
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
/**
 * The view in a half of the Panel, as the window draws one (`RegionFrame`'s `Half`): the half's
 * line holds the view's one title and its actions (#3355), and the view under it is the one
 * scroll, its top held at the top of it.
 */
function View({
  view,
  count,
  done = 0,
}: {
  view: keyof typeof KEYS;
  count: number;
  /** Done todos after the open ones, each with its box ticked (the Todos view only). */
  done?: number;
}) {
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
    <Panel variant="bottom" aria-label={KEYS[view][0]} role="region">
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
          <PanelTitle className="side-title">{KEYS[view][0]}</PanelTitle>
          <span className="view-actions" ref={setLine} />
        </div>
        <div style={{ display: "flex", flex: "1 1 0", minBlockSize: 0 }}>
          <HeadActions.Provider value={line}>
            <Panels
              view={view as OwnViewId}
              workspace="alpha"
              state={stateOf(view, count, done)}
              offers={offers}
              onPress={() => {}}
              contributed={[]}
              shownRow={shown}
              onShowRow={setShown}
              onAddTodo={async () => undefined}
            />
          </HeadActions.Provider>
        </div>
      </div>
    </Panel>
  );
}

const meta = { component: View } satisfies Meta<typeof View>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The keyboard reaches the field in the top, then the rows; the top stays put as they scroll. */
async function walk(
  canvasElement: HTMLElement,
  tab: () => Promise<void>,
  field: HTMLElement,
  long: boolean,
) {
  await tab();
  await expect(field).toHaveFocus();
  expectFocusInSight();
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
    );
  },
};

export const TodosWithDone: Story = {
  args: { view: "todos", count: 3, done: 2 },
  play: async ({ canvas }) => {
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
