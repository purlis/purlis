// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { StatusLine, projectName, runningIn } from "./StatusLine";
import type { DoctorState } from "./Doctor";
import type { DoctorReport, DoctorRow, FactBadge, Panels, PanelTodo, Piece } from "./bindings";
import type { WorkspaceState } from "./workspaceState";
import { tipOf } from "./ui/tooltip/tooltip.testkit";

/**
 * **The status line's own rules**, on the component and nothing else.
 *
 * The window-level questions — that it is under every region, that the project's path is no
 * longer on the bar — are in `FourRegions.test.tsx` against the whole app, and the ones that
 * need a browser that really lays out are in `app/e2e/specs/status-line.e2e.ts`. What is here
 * is what a props-in/markup-out test can hold: which counts are drawn, which are dropped, and
 * what the Notices button says when purlis has nothing to count.
 */

afterEach(cleanup);

const PLANE = "/home/dev/plane";

function todo(slug: string): PanelTodo {
  return { slug, title: slug, stamp: "2026-09-22" };
}

function piece(name: string): Piece {
  return {
    piece: name,
    path: `${PLANE}/${name}`,
    branch: name,
    wired: true,
    stale: false,
    said: "",
  };
}

function panels(over: Partial<Panels> = {}): Panels {
  return {
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
    contributed: [],
    ...over,
  };
}

function state(over: Partial<WorkspaceState> = {}): WorkspaceState {
  return { pieces: {}, piecesRefused: {}, reading: false, ...over };
}

type Props = Parameters<typeof StatusLine>[0];

/** The line, with everything read and nothing to count unless a test says so. */
function draw(over: Partial<Props> = {}) {
  const props: Props = {
    plane: PLANE,
    read: true,
    where: "alpha",
    workspaces: 2,
    state: state(),
    ...over,
  };
  return render(<StatusLine {...props} />);
}

/** What the line says, as one string with its spacing normalised. */
const words = () => (screen.getByTestId("status-line").textContent ?? "").replace(/\s+/g, " ");

describe("the status line says where you are", () => {
  it("names the workspace the window is on", () => {
    draw({ where: "alpha" });

    expect(words()).toContain("alpha");
  });

  it("says it is still reading rather than naming no workspace", () => {
    // Before the plane has answered there is no workspace to name, and an empty cell there
    // would read as "you are nowhere" — which is a different claim from "charter has not
    // looked yet".
    draw({ read: false, where: undefined, workspaces: undefined });

    expect(words()).toContain("reading the project…");
    // And `ws 0` is not drawn either, for the same reason: nobody has counted yet.
    expect(screen.queryByTestId("status-workspaces")).toBeNull();
  });

  it("says the window is on no workspace once the plane has answered with none", () => {
    // A plane that holds no workspaces answers perfectly well. "reading the project…" under it
    // would be charter waiting forever for something that has already happened.
    draw({ read: true, where: undefined, workspaces: 0 });

    expect(words()).toContain("no workspace");
    expect(words()).not.toContain("reading");
    // Zero IS drawn for `ws`: an empty plane is a fact, not an absence of one.
    expect(within(screen.getByTestId("status-workspaces")).getByText("0")).toBeInTheDocument();
  });

  it("says how many workspaces the project has", () => {
    draw({ workspaces: 7 });

    expect(within(screen.getByTestId("status-workspaces")).getByText("7")).toBeInTheDocument();
  });
});

describe("a count is drawn only when charter can stand behind it", () => {
  it("counts the focused workspace's open todos", () => {
    draw({ state: state({ panels: panels({ todos: [todo("one"), todo("two")] }) }) });

    expect(words()).toContain("todo 2");
  });

  it("draws no todo cell at zero, because presence is the signal", () => {
    // charter's own footer drops the cell rather than printing `todo 0`: a zero present every
    // turn is furniture, and a real `todo 7` in that spot then draws no more attention.
    draw({ state: state({ panels: panels({ todos: [] }) }) });

    expect(screen.queryByTestId("status-todos")).toBeNull();
    expect(words()).not.toContain("todo");
  });

  it("draws no todo cell when the store refused, even though it read some", () => {
    // **The refusal is partial, on purpose.** `todos_refused` says where charter would not
    // look, and it can be set with todos already in hand — so `todos.length` is a real number
    // and a wrong one. A cell drawn from it would be the smaller, quieter answer, and it would
    // contradict the refusal the right-hand region draws in full. A test that paired the
    // refusal with an empty list would pass with this guard deleted, because the zero rule
    // above already drops it: measured, by deleting the guard and watching this stay green.
    draw({
      state: state({
        panels: panels({
          todos: [todo("one"), todo("two")],
          todos_refused: "one todo store is a link out of the plane",
        }),
      }),
    });

    expect(screen.queryByTestId("status-todos")).toBeNull();
  });

  it("counts every clone's worktrees together", () => {
    draw({
      state: state({
        panels: panels({ repos: ["svc", "tool"] }),
        pieces: { svc: [piece("one"), piece("two")], tool: [piece("three")] },
      }),
    });

    expect(words()).toContain("branches 3");
  });

  it("draws no piece count while a clone has not answered yet", () => {
    // `worktree_list` runs per clone and comes back per clone. A total taken with `tool` still
    // listing is smaller than the truth and carries no mark saying so.
    draw({
      state: state({
        panels: panels({ repos: ["svc", "tool"] }),
        pieces: { svc: [piece("one"), piece("two")] },
      }),
    });

    expect(screen.queryByTestId("status-pieces")).toBeNull();
  });

  it("draws no piece count when a clone's listing was refused", () => {
    draw({
      state: state({
        panels: panels({ repos: ["svc", "tool"] }),
        pieces: { svc: [piece("one")] },
        piecesRefused: { tool: "not a git repository" },
      }),
    });

    expect(screen.queryByTestId("status-pieces")).toBeNull();
  });

  it("draws no piece count for a workspace whose clones have none", () => {
    draw({ state: state({ panels: panels({ repos: ["svc"] }), pieces: { svc: [] } }) });

    expect(screen.queryByTestId("status-pieces")).toBeNull();
  });
});

describe("the Notices button (#1695)", () => {
  it("is disabled and says so when no Inbox is behind it", () => {
    // A control that answered a press with nothing would teach the operator that nothing is
    // said. The window always wires the Inbox; a status line drawn on its own does not.
    draw();

    const button = screen.getByRole("button", { name: "Notices — nothing to open here" });
    // Unavailable, and still a Tab stop: the bar's menu always opens from Notices (#2913), and
    // its tooltip says why (#2908).
    expect(button).toHaveAttribute("aria-disabled", "true");
    expect(button).not.toBeDisabled();
    expect(button).toHaveAttribute("data-disabled", "true");
    expect(button).toHaveAttribute("tabindex", "0");
    expect(button).toHaveAccessibleDescription("This status line has no Inbox behind it.");
  });

  it("drops a count purlis cannot stand behind and still opens the Inbox", async () => {
    // The alerts are not read yet, or purlis stopped looking: a partial total is a wrong
    // total. The dash is not a zero, and the Inbox is where the reason is.
    const open = vi.fn();
    draw({ alerts: { count: undefined, open } });

    const button = screen.getByRole("button", { name: "Notices: not counted" });
    expect(button).toBeEnabled();
    expect(button).not.toHaveTextContent(/\d/);
    expect(button).toHaveTextContent("—");
    await userEvent.click(button);
    expect(open).toHaveBeenCalledTimes(1);
  });

  it("draws no badge at zero, and says none to a screen reader", () => {
    // The footer's rule: a `0` there every day is furniture, and a real `2` in that spot then
    // draws no more attention than the zero did.
    draw({ alerts: { count: 0, open: vi.fn() } });

    const button = screen.getByRole("button", { name: "Notices: none" });
    expect(button).toBeEnabled();
    expect(button).not.toHaveTextContent("0");
    expect(button).not.toHaveTextContent("—");
  });

  it("draws the count and opens the Inbox", async () => {
    const open = vi.fn();
    draw({ alerts: { count: 3, open } });

    const button = screen.getByRole("button", { name: "Notices: 3" });
    expect(button).toBeEnabled();
    await userEvent.click(button);

    expect(open).toHaveBeenCalledTimes(1);
  });
});

/**
 * **The project is its name, and its directory is the tooltip** (qw357). The operator:
 * *"project directory is very large and can be just compacted and showed only project name, and
 * on hovering just show directory"*.
 */
describe("the project, as its name (qw357)", () => {
  const project = () => screen.getByTestId("status-project");

  it("shows the directory's last segment, and not the directory", () => {
    draw({ plane: "/Users/dev/IdeaProjects/charter" });

    expect(project()).toHaveTextContent(/^charter$/);
    expect(words()).not.toContain("/Users/dev");
    // Named for what it is, in words a screen reader reads: the mark is an image called Project.
    expect(within(project()).getByRole("img", { name: "Project" })).toBeInTheDocument();
  });

  it.each([
    ["/home/dev/plane", "plane"],
    ["/home/dev/plane/", "plane"],
    ["C:\\Users\\dev\\plane", "plane"],
    ["/", "/"],
  ])("reads %s as %s", (directory, name) => {
    expect(projectName(directory)).toBe(name);
  });

  it("says the whole directory in its tooltip, under the pointer", async () => {
    draw();
    expect(screen.queryByRole("tooltip")).toBeNull();

    await userEvent.hover(project());

    expect(await screen.findByRole("tooltip")).toHaveTextContent(PLANE);
  });

  it("is reached by the keyboard, which opens the same tooltip", async () => {
    // A path that only a hover shows is one a keyboard never reads.
    draw();
    expect(project()).toHaveAttribute("tabindex", "0");

    await userEvent.tab();

    expect(project()).toHaveFocus();
    expect(await screen.findByRole("tooltip")).toHaveTextContent(PLANE);
  });

  it("hands the directory to whoever reads the window, whole", () => {
    // The scenario specs learn which project is open from here (`app/e2e`).
    draw();

    expect(project()).toHaveAttribute("data-directory", PLANE);
  });

  it("gives way first, a long name cut with an ellipsis, with a line after it", () => {
    // The layout itself is the story's to measure (`ui/status/StatusItem.stories.tsx`,
    // NarrowWithAPinArriving): jsdom lays nothing out.
    draw();
    expect(project()).toHaveAttribute("data-shrinks");
    expect(project().closest('[data-part="slot"]')?.nextElementSibling).toHaveAttribute(
      "data-part",
      "separator",
    );
  });
});

/**
 * **One bar, two ends** (qw357): context on the left, health on the right, and what can be
 * pressed is a status item, the set's small ghost button, and not a box of its own.
 */
describe("the line is one status bar (qw357)", () => {
  const row = (name: string, status: DoctorRow["status"]): DoctorRow => ({
    name,
    status,
    detail: "",
    hint: "",
    fix: null,
    settings: null,
    checked: true,
  });
  const doctor = (rows: DoctorRow[]): DoctorState => ({
    report: { rows, full: false } as unknown as DoctorReport,
    running: false,
    run: vi.fn(),
  });

  it("puts where you are on the left and what wants a look on the right", () => {
    draw({ alerts: { count: 2, open: vi.fn() }, doctor: doctor([row("git", "warn")]) });

    const [context, health, ...more] = [...screen.getByTestId("status-line").children];
    expect(more).toEqual([]);
    expect(context).toHaveAttribute("data-part", "start");
    expect(context.firstElementChild?.firstElementChild).toBe(screen.getByTestId("status-project"));
    expect(context).toHaveTextContent("alpha");
    expect(health).toHaveAttribute("data-part", "end");
    // The doctor inside, Notices outermost: the one that always stays never moves (#2913).
    expect(within(health as HTMLElement).getAllByRole("button")).toEqual([
      screen.getByTestId("status-doctor"),
      screen.getByTestId("status-alerts"),
    ]);
  });

  it("draws Notices and the doctor as status items, flat on the bar", () => {
    draw({ alerts: { count: 2, open: vi.fn() }, doctor: doctor([row("git", "warn")]) });

    for (const item of [screen.getByTestId("status-alerts"), screen.getByTestId("status-doctor")]) {
      expect(item).toHaveAttribute("data-scope", "status-item");
      expect(item).toHaveAttribute("data-variant", "bar");
    }
    // And no rule of the window's own boxes them again: the look is the family's, whole.
    const css = readFileSync(join(process.cwd(), "src/App.css"), "utf8");
    for (const boxed of [".status-alerts {", ".status-doctor {", ".status-pin {"]) {
      expect(css).not.toContain(`\n${boxed}`);
    }
  });

  it("says the doctor's count in words, in its verdict's colour class, and nothing at zero", () => {
    const { unmount } = draw({ doctor: doctor([row("git", "warn"), row("gh", "warn")]) });
    const warned = screen.getByRole("button", { name: "Doctor: 2 warnings" });
    expect(warned.querySelector('[data-verdict="waiting"]')).toHaveTextContent(/^2 warnings$/);
    unmount();

    draw({ doctor: doctor([row("git", "ok")]) });
    const clean = screen.getByTestId("status-doctor");
    expect(clean.querySelector("[data-verdict]")).toBeNull();
    expect(clean).toHaveAccessibleName(/nothing wrong/);
  });

  it("announces the Notices count in the item's name, with the number drawn beside the bell", () => {
    draw({ alerts: { count: 6, open: vi.fn() } });

    const notices = screen.getByRole("button", { name: "Notices: 6" });
    expect(notices.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
    expect(notices).toHaveTextContent(/Notices\s*6/);
  });
});

/**
 * **The region toggles are gone from the line** (qw357). The operator: *"bottom toggle icons
 * are redundant now … they can be removed fully"*. What they did is each side's activity bar's,
 * its key's and the palette's: `FourRegions.test.tsx` holds a side put away and brought back by
 * each, and `actions.test.ts` the palette's rows.
 */
describe("the region toggles (qw357)", () => {
  it("are not on the line: nothing on it is a toggle", () => {
    draw({ alerts: { count: 0, open: vi.fn() } });

    expect(screen.getByTestId("status-line").querySelector("[aria-pressed]")).toBeNull();
    expect(document.querySelector(".regions-doing, .region-toggle")).toBeNull();
  });

  it("left no rule and no component behind", () => {
    const read = (file: string) => readFileSync(join(process.cwd(), "src", file), "utf8");

    expect(read("App.css")).not.toMatch(/\.regions-doing|\.region-toggle/);
    expect(read("RegionFrame.tsx")).not.toContain("RegionToggle");
  });
});

describe("an extension's badges (charter-app#340)", () => {
  const badge = (over: Partial<FactBadge> = {}): FactBadge => ({
    extension: "prs",
    name: "Pull requests",
    id: "open",
    label: "PRs",
    value: "3",
    age_seconds: 20,
    stale: false,
    ...over,
  });

  it("draws each badge's label and value, from its facts file", () => {
    draw({ badges: [badge()] });

    const drawn = screen.getByTestId("status-badge-prs-open");
    expect(drawn).toHaveTextContent("PRs 3");
    expect(drawn).not.toHaveAttribute("data-tone");
    expect(tipOf(drawn)).toContain("Pull requests");
  });

  it("dims a value older than its extension declared fresh, and says how old it is", () => {
    draw({ badges: [badge({ stale: true, age_seconds: 7200 })] });

    const drawn = screen.getByTestId("status-badge-prs-open");
    expect(drawn).toHaveAttribute("data-tone", "quiet");
    expect(drawn).toHaveTextContent("PRs 3 · 2h ago");
  });

  it("says why an extension shows nothing, rather than letting its badge vanish", () => {
    draw({ factNotes: ["Pull requests changed since you approved it"] });

    const said = screen.getByTestId("status-fact-notes");
    expect(said).toHaveTextContent("1 extension note");
    expect(tipOf(said)).toBe("Pull requests changed since you approved it");
  });

  it("draws no badge when no extension has one", () => {
    draw({ badges: [] });

    expect(screen.queryByTestId(/^status-badge-/)).toBeNull();
  });
});

describe("what `N chats running` counts", () => {
  const chat = (state: string) => ({ state });

  it("counts a chat that is running and no other state", () => {
    // The operator runs many chats at once and most of them are sitting still. `open` would
    // say 50 all day; `running` is the number that changes when something is happening.
    const report = {
      settled: true,
      ending: [
        chat("running"),
        chat("waiting"),
        chat("done"),
        chat("failed"),
        chat("unknown"),
        chat("running"),
      ],
    };

    expect(runningIn(report)).toBe(2);
  });

  it("counts nothing at all until the project has said what it had open", () => {
    // An empty `ending` before the core has answered is "not yet", never zero — the same
    // distinction `PlaneReport.settled` exists for, and a quit that got it wrong would end
    // every chat it had not heard about.
    expect(runningIn({ settled: false, ending: [] })).toBeUndefined();
    expect(runningIn({ settled: false, ending: [chat("running")] })).toBeUndefined();
    expect(runningIn(undefined)).toBeUndefined();
  });

  it("counts zero once it has settled on nothing, which is an answer", () => {
    expect(runningIn({ settled: true, ending: [] })).toBe(0);
  });
});
