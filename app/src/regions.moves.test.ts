// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, renderHook } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  DEFAULT_ARRANGEMENT,
  drawnNow,
  forgetThisLaunch,
  inSlots,
  MOST_PROJECTS,
  movesTo,
  openView,
  panelShown,
  picked,
  placesOf,
  PROJECTS_MOST_BYTES,
  regionOf,
  remembered,
  settleLayout,
  showing,
  shownIn,
  tabsOf,
  useArrangement,
  viewMoved,
  viewsOf,
  type Arrangement,
  type RegionId,
} from "./regions";
import {
  aboutThisMachine,
  GLOBAL,
  sayAboutThisMachine,
  usingTheDefaultLayout,
  type Reading,
} from "./windowprefs";

/**
 * **Moving a view between its side's sidebar and its half of the Panel** (#1740, spec #1736,
 * P-3, P-4): "Move to panel" and "Move to sidebar", within a side only; the Inbox pinned to the
 * right sidebar; an extension's panel moving like any view of its side; where each view lives
 * kept per project, and Use the default layout putting every view back.
 */

const PATH = "/home/op/.config/charter/layout.json";
const handed = (layout: Partial<Reading>) => {
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: { path: PATH, found: false, document: null, trouble: null, ...layout },
    theme: { path: "", found: false, document: null, trouble: null },
  };
};
const put = (document: unknown) => handed({ found: true, document });

let sent: { cmd: string; args: unknown }[] = [];
const lastWritten = () => {
  const writes = sent.filter((one) => one.cmd === "write_layout");
  return JSON.parse((writes[writes.length - 1].args as { text: string }).text);
};

beforeEach(() => {
  globalThis.localStorage.clear();
  Reflect.deleteProperty(globalThis, GLOBAL);
  forgetThisLaunch();
  sayAboutThisMachine("layout", undefined);
  sent = [];
  mockIPC((cmd, args) => {
    sent.push({ cmd, args });
    return null;
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
});

const of = (arrangement: Arrangement, id: RegionId) => {
  const found = arrangement.find((one) => one.id === id);
  if (found === undefined) throw new Error(`no ${id}`);
  return found;
};
const BURN = "panel:ext/stats/burn";

describe("where a view can move (P-3, P-4)", () => {
  it("is the other place on its own side: a sidebar's view to its half, a half's to its sidebar", () => {
    expect(movesTo(DEFAULT_ARRANGEMENT, "search")).toBe("panel-left");
    expect(movesTo(DEFAULT_ARRANGEMENT, "chats")).toBe("panel-left");
    expect(movesTo(DEFAULT_ARRANGEMENT, "repos")).toBe("navigation");
    expect(movesTo(DEFAULT_ARRANGEMENT, "changes")).toBe("navigation");
    expect(movesTo(DEFAULT_ARRANGEMENT, "personas")).toBe("panel-right");
    expect(movesTo(DEFAULT_ARRANGEMENT, "memory")).toBe("aside");
  });

  it("is nowhere for the Inbox, pinned to the right sidebar", () => {
    expect(movesTo(DEFAULT_ARRANGEMENT, "inbox")).toBeUndefined();
  });

  it("is the right half for an extension's panel, which lives on the right", () => {
    expect(movesTo(DEFAULT_ARRANGEMENT, BURN)).toBe("panel-right");
    const down = viewMoved(DEFAULT_ARRANGEMENT, BURN, "panel-right");
    expect(movesTo(down, BURN)).toBe("aside");
  });

  it("follows the view once it has moved, so the next move takes it back", () => {
    const down = viewMoved(DEFAULT_ARRANGEMENT, "search", "panel-left");

    expect(regionOf("search", down)).toBe("panel-left");
    expect(movesTo(down, "search")).toBe("navigation");
  });
});

describe("moving a view (#1740)", () => {
  it("puts a sidebar's view in its half, after the half's own, open there and the half out", () => {
    const away = DEFAULT_ARRANGEMENT.map((one) =>
      one.id === "panel-left" ? { ...one, collapsed: true } : one,
    );

    const moved = viewMoved(away, "search", "panel-left");

    expect(viewsOf(of(moved, "navigation"))).toEqual(["chats", "explorer", "skills"]);
    expect(viewsOf(of(moved, "panel-left"))).toEqual(["repos", "changes", "search"]);
    expect(of(moved, "panel-left")).toMatchObject({ view: "search", collapsed: false });
    expect(regionOf("search", moved)).toBe("panel-left");
  });

  it("puts a half's view in its sidebar, open there", () => {
    const moved = viewMoved(DEFAULT_ARRANGEMENT, "memory", "aside");

    expect(viewsOf(of(moved, "aside"))).toEqual(["inbox", "personas", "vaults", "memory"]);
    expect(viewsOf(of(moved, "panel-right"))).toEqual(["todos", "sessions"]);
    expect(openView(of(moved, "aside"))).toBe("memory");
    // The half opens on what it has left.
    expect(openView(of(moved, "panel-right"))).toBe("todos");
  });

  it("lets the place it left open on its first view, when the moved one was open there", () => {
    const onSearch = showing(DEFAULT_ARRANGEMENT, "search");

    const moved = viewMoved(onSearch, "search", "panel-left");

    expect(openView(of(moved, "navigation"))).toBe("chats");
    expect(of(moved, "navigation").collapsed).toBe(false);
  });

  it("is undone by the move back, which leaves the arrangement as it started", () => {
    const there = viewMoved(DEFAULT_ARRANGEMENT, "explorer", "panel-left");

    const back = viewMoved(there, "explorer", "navigation");

    expect(viewsOf(of(back, "navigation"))).toEqual(["chats", "explorer", "search", "skills"]);
    expect(viewsOf(of(back, "panel-left"))).toEqual(["repos", "changes"]);
    expect(back.map(({ id, side, order, collapsed }) => ({ id, side, order, collapsed }))).toEqual(
      DEFAULT_ARRANGEMENT,
    );
    expect(back.every((one) => one.views === undefined)).toBe(true);
  });

  it("is opened and put away by its icon where it lives now, and by its key", () => {
    const moved = viewMoved(DEFAULT_ARRANGEMENT, "search", "panel-left");

    const shut = picked(moved, "search");
    expect(of(shut, "panel-left").collapsed).toBe(true);
    expect(of(shut, "navigation").collapsed).toBe(false);

    const shown = showing(shut, "search");
    expect(of(shown, "panel-left")).toMatchObject({ view: "search", collapsed: false });
  });

  it("empties a sidebar whose every view moved down, which is then not drawn, and fills it again", () => {
    let all = DEFAULT_ARRANGEMENT;
    for (const view of ["chats", "explorer", "search", "skills"] as const)
      all = viewMoved(all, view, "panel-left");

    expect(viewsOf(of(all, "navigation"))).toEqual([]);
    expect(shownIn(inSlots(all).left)).toEqual([]);

    const one = viewMoved(all, "chats", "navigation");
    expect(shownIn(inSlots(one).left).map((placed) => placed.id)).toEqual(["navigation"]);
  });

  it("brings the Panel back on a half that was emptied and has a view again", () => {
    let none = DEFAULT_ARRANGEMENT;
    for (const view of ["repos", "changes", "memory", "todos", "sessions"] as const)
      none = viewMoved(none, view, view === "repos" || view === "changes" ? "navigation" : "aside");
    expect(panelShown(none)).toBe(false);

    expect(panelShown(viewMoved(none, "todos", "panel-right"))).toBe(true);
  });

  it("moves an extension's panel to the right half, out of the right sidebar's bar, and back", () => {
    const down = viewMoved(DEFAULT_ARRANGEMENT, BURN, "panel-right");

    expect(tabsOf(down, "aside", [BURN])).toEqual(["inbox", "personas", "vaults"]);
    expect(tabsOf(down, "panel-right", [BURN])).toEqual(["memory", "todos", "sessions", BURN]);
    expect(of(down, "panel-right").view).toBe(BURN);
    // Not drawn while its extension contributes nothing, and kept for when it does again.
    expect(tabsOf(down, "panel-right", [])).toEqual(["memory", "todos", "sessions"]);

    const up = viewMoved(down, BURN, "aside");
    expect(tabsOf(up, "aside", [BURN])).toEqual(["inbox", "personas", "vaults", BURN]);
    expect(up.every((one) => one.views === undefined)).toBe(true);
  });
});

describe("a move that is refused (P-3, P-4)", () => {
  it("never takes a view to the other side, either way", () => {
    expect(viewMoved(DEFAULT_ARRANGEMENT, "chats", "panel-right")).toBe(DEFAULT_ARRANGEMENT);
    expect(viewMoved(DEFAULT_ARRANGEMENT, "chats", "aside")).toBe(DEFAULT_ARRANGEMENT);
    expect(viewMoved(DEFAULT_ARRANGEMENT, "memory", "navigation")).toBe(DEFAULT_ARRANGEMENT);
    expect(viewMoved(DEFAULT_ARRANGEMENT, "repos", "panel-right")).toBe(DEFAULT_ARRANGEMENT);
    expect(viewMoved(DEFAULT_ARRANGEMENT, BURN, "panel-left")).toBe(DEFAULT_ARRANGEMENT);
  });

  it("never moves the Inbox", () => {
    expect(viewMoved(DEFAULT_ARRANGEMENT, "inbox", "panel-right")).toBe(DEFAULT_ARRANGEMENT);
    expect(viewMoved(DEFAULT_ARRANGEMENT, "inbox", "panel-left")).toBe(DEFAULT_ARRANGEMENT);
  });

  it("does nothing for a move to where the view already is", () => {
    expect(viewMoved(DEFAULT_ARRANGEMENT, "chats", "navigation")).toBe(DEFAULT_ARRANGEMENT);
  });

  it("does nothing for a press that asks for the place the view is in, or for the Inbox", () => {
    const { result } = renderHook(() => useArrangement("/one"));
    const before = result.current.arrangement;

    act(() => result.current.moveView("chats", "sidebar"));
    act(() => result.current.moveView("memory", "panel"));
    act(() => result.current.moveView("inbox", "panel"));

    expect(result.current.arrangement).toEqual(before);
    expect(regionOf("inbox", result.current.arrangement)).toBe("aside");
  });
});

describe("where each view lives, kept (#1740, B-11)", () => {
  it("is written under the project, as what moved into each place, and read back the next launch", async () => {
    const { result } = renderHook(() => useArrangement("/one"));

    act(() => result.current.moveView("search", "panel"));
    act(() => result.current.moveView("memory", "sidebar"));
    act(() => result.current.moveView("explorer", "panel"));

    await vi.waitFor(() => expect(sent).toHaveLength(3));
    const regions = lastWritten().projects["/one"].regions as Record<string, unknown>[];
    expect(Object.fromEntries(regions.map((one) => [one.id, one.moved]))).toEqual({
      navigation: undefined,
      aside: ["memory"],
      "panel-left": ["search", "explorer"],
      "panel-right": undefined,
    });
    expect(regions.some((one) => "views" in one)).toBe(false);

    forgetThisLaunch();
    put(lastWritten());
    const again = remembered("/one");
    expect(regionOf("search", again)).toBe("panel-left");
    expect(regionOf("memory", again)).toBe("aside");
    await settleLayout();
    expect(aboutThisMachine()).toEqual([]);
  });

  it("is the project's: another project's file entry keeps its own", async () => {
    put({
      version: 2,
      regions: [],
      projects: {
        "/two": {
          regions: [{ id: "navigation", side: "left", moved: ["changes"] }],
        },
      },
    });
    const { result } = renderHook(() => useArrangement("/one"));

    act(() => result.current.moveView("search", "panel"));

    expect(regionOf("changes", remembered("/two"))).toBe("navigation");
    expect(regionOf("search", remembered("/two"))).toBe("navigation");
    expect(regionOf("search", remembered("/one"))).toBe("panel-left");
  });

  it("is forgotten by Use the default layout: every view back where it starts, the Panel too", () => {
    const { result } = renderHook(() => useArrangement("/one"));
    act(() => result.current.moveView("repos", "sidebar"));
    act(() => result.current.moveView("changes", "sidebar"));
    act(() => result.current.moveView("sessions", "sidebar"));

    act(() => usingTheDefaultLayout());

    expect(remembered("/one")).toEqual(DEFAULT_ARRANGEMENT);
    expect(regionOf("repos", remembered("/one"))).toBe("panel-left");
    expect(panelShown(remembered("/one"))).toBe(true);
  });

  it("keeps every project's moves inside the projects' budget in the file", async () => {
    // Every project the file can hold, each with its Panel dragged and a view moved: all are
    // sent. (A new project starts from the machine's arrangement, so each holds the move.)
    for (let at = 0; at < MOST_PROJECTS; at += 1) {
      const { result, unmount } = renderHook(() =>
        useArrangement(`/home/someone/projects/a-project-with-a-long-name-${at}`),
      );
      act(() => result.current.resized({ panel: 41.25, left: 22.5, right: 27.75 }));
      act(() => result.current.moveView("search", "panel"));
      unmount();
    }

    await vi.waitFor(() => expect(sent).toHaveLength(MOST_PROJECTS * 2));
    const projects = lastWritten().projects as Record<string, unknown>;
    expect(Object.keys(projects)).toHaveLength(MOST_PROJECTS);
    const bytes = new TextEncoder().encode(JSON.stringify(projects, null, 2)).length;
    expect(bytes).toBeLessThanOrEqual(PROJECTS_MOST_BYTES);
  });

  it("stays inside the budget with every view of every project moved, letting the oldest go", async () => {
    for (let at = 0; at < MOST_PROJECTS; at += 1) {
      const { result, unmount } = renderHook(() =>
        useArrangement(`/home/someone/projects/a-project-with-a-long-name-${at}`),
      );
      act(() => result.current.resized({ panel: 41.25, left: 22.5, right: 27.75 }));
      for (const view of ["chats", "explorer", "search"] as const)
        act(() => result.current.moveView(view, "panel"));
      for (const view of ["memory", "todos", "sessions"] as const)
        act(() => result.current.moveView(view, "sidebar"));
      act(() => result.current.moveView(BURN, "panel"));
      unmount();
    }

    await vi.waitFor(() => expect(sent).toHaveLength(MOST_PROJECTS * 8));
    const projects = lastWritten().projects as Record<string, unknown>;
    const bytes = new TextEncoder().encode(JSON.stringify(projects, null, 2)).length;
    expect(bytes).toBeLessThanOrEqual(PROJECTS_MOST_BYTES);
    // The project arranged last is always kept.
    expect(projects).toHaveProperty(
      `/home/someone/projects/a-project-with-a-long-name-${MOST_PROJECTS - 1}`,
    );
  });
});

describe("a file's `moved`, read field by field", () => {
  const read = (regions: unknown[]) => {
    put({ version: 2, regions });
    return remembered();
  };

  it("draws each place's own views first and what moved into it after, in the file's order", () => {
    const got = read([{ id: "panel-left", side: "left", moved: ["search", "chats"] }]);

    expect(viewsOf(of(got, "navigation"))).toEqual(["explorer", "skills"]);
    expect(viewsOf(of(got, "panel-left"))).toEqual(["repos", "changes", "search", "chats"]);
  });

  it("keeps the Inbox in the right sidebar whatever the file says, and says so", async () => {
    const got = read([{ id: "panel-right", side: "right", moved: ["inbox", "personas"] }]);

    expect(regionOf("inbox", got)).toBe("aside");
    expect(regionOf("personas", got)).toBe("panel-right");
    await settleLayout();
    expect(aboutThisMachine()[0].detail).toContain('"inbox" is pinned to aside');
  });

  it("refuses a view of the other side, and one this build does not have, and says so", async () => {
    const got = read([{ id: "panel-left", side: "left", moved: ["memory", "nonesuch", 7] }]);

    expect(viewsOf(of(got, "panel-left"))).toEqual(["repos", "changes"]);
    expect(regionOf("memory", got)).toBe("panel-right");
    await settleLayout();
    const said = aboutThisMachine()[0].detail;
    expect(said).toContain('"memory" is of the right side');
    expect(said).toContain('"nonesuch"');
    expect(said).toContain("7");
  });

  it("reads past a place's own view in its list, and a name said twice, without a word", async () => {
    const got = read([{ id: "panel-left", side: "left", moved: ["repos", "chats", "chats"] }]);

    expect(viewsOf(of(got, "panel-left"))).toEqual(["repos", "changes", "chats"]);
    await settleLayout();
    expect(aboutThisMachine()).toEqual([]);
  });

  it("refuses `moved` that is not a list, and says so", async () => {
    const got = read([{ id: "panel-left", side: "left", moved: "chats" }]);

    expect(regionOf("chats", got)).toBe("navigation");
    await settleLayout();
    expect(aboutThisMachine()[0].detail).toContain("panel-left's moved");
  });

  it("opens a place on a view moved into it, and the place it left on its own, without a word", async () => {
    const got = read([
      { id: "navigation", side: "left", view: "search" },
      { id: "panel-left", side: "left", moved: ["search"], view: "search" },
    ]);

    expect(openView(of(got, "navigation"))).toBe("chats");
    expect(openView(of(got, "panel-left"))).toBe("search");
    await settleLayout();
    expect(aboutThisMachine()).toEqual([]);
  });

  it("keeps an extension's panel moved to the right half, contributed or not", () => {
    const got = read([{ id: "panel-right", side: "right", moved: [BURN] }]);

    expect(regionOf(BURN, got)).toBe("panel-right");
    expect(tabsOf(got, "aside", [BURN])).toEqual(["inbox", "personas", "vaults"]);
    expect(tabsOf(got, "panel-right", [BURN])).toEqual(["memory", "todos", "sessions", BURN]);
  });

  it("refuses an extension's panel moved to the left, which takes none", async () => {
    const got = read([{ id: "panel-left", side: "left", moved: [BURN] }]);

    expect(regionOf(BURN, got)).toBe("aside");
    await settleLayout();
    expect(aboutThisMachine()[0].detail).toContain("is of the right side");
  });
});

describe("a half whose moved panels are not contributed now (#1752, train 64's review)", () => {
  const BURN = "panel:ext/stats/burn" as const;
  /** The right side's own views moved out of its half, and an extension's panel moved in. */
  const onlyAPanel = (): Arrangement => {
    let moved: Arrangement = DEFAULT_ARRANGEMENT;
    for (const view of ["memory", "todos", "sessions"] as const)
      moved = viewMoved(moved, view, "aside");
    return viewMoved(moved, BURN, "panel-right");
  };

  it("is not drawn while its panel is not contributed", () => {
    const drawn = drawnNow(onlyAPanel(), []);
    const half = drawn.find((one) => one.id === "panel-right");

    expect(half && shownIn([half])).toEqual([]);
  });

  it("is drawn again once its panel is", () => {
    const drawn = drawnNow(onlyAPanel(), [BURN]);
    const half = drawn.find((one) => one.id === "panel-right");

    expect(half && shownIn([half]).length).toBe(1);
    expect(half && viewsOf(half)).toEqual([BURN]);
  });

  it("keeps what is stored: only the drawn arrangement leaves the panel out", () => {
    const kept = onlyAPanel();
    drawnNow(kept, []);

    expect(kept.find((one) => one.id === "panel-right")?.views).toEqual([BURN]);
    expect(drawnNow(DEFAULT_ARRANGEMENT, [])).toBe(DEFAULT_ARRANGEMENT);
  });
});

describe("where each view lives, for the palette (#1752)", () => {
  it("lists every view drawn, by region, with the move it has", () => {
    const moved = viewMoved(DEFAULT_ARRANGEMENT, "search", "panel-left");
    const places = placesOf(moved, [{ view: "panel:ext/stats/burn", name: "Burn rate" }]);

    expect(places.map((one) => [one.view, one.region, one.to])).toEqual([
      ["chats", "navigation", "panel"],
      ["explorer", "navigation", "panel"],
      ["inbox", "aside", undefined],
      ["personas", "aside", "panel"],
      ["vaults", "aside", "panel"],
      ["panel:ext/stats/burn", "aside", "panel"],
      ["repos", "panel-left", "sidebar"],
      ["changes", "panel-left", "sidebar"],
      ["search", "panel-left", "sidebar"],
      ["memory", "panel-right", "sidebar"],
      ["todos", "panel-right", "sidebar"],
      ["sessions", "panel-right", "sidebar"],
    ]);
    expect(places.find((one) => one.view === "panel:ext/stats/burn")?.name).toBe("Burn rate");
    expect(places.find((one) => one.view === "search")?.name).toBe("Search");
  });
});
