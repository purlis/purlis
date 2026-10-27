// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { BrowsersGridTab, LIVE_AT_MOST, tileTitle } from "./BrowsersGridTab";
import {
  forgetProjectsBrowsers,
  keptBrowsersScope,
  tellProjectBrowsers,
  type GridTile,
} from "./browsersGrid";
import { forgetProjectViews } from "./projectViews";

/**
 * **The Browsers grid** (#2027): every browser in scope as a live tile, the Inbox's scope
 * switch over them, and a press opening the browser's view in its own project.
 */

const HERE = "/home/dev/plane";
const THERE = "/home/dev/other";

/** What each `watch_browser` asked: the program and the role it was watched as. */
let watched: { program: string; role: string }[] = [];
/** The views let go of, by program. */
let unwatched: string[] = [];

beforeEach(() => {
  watched = [];
  unwatched = [];
  mockIPC((cmd, args) => {
    if (cmd === "unwatch_browser") {
      unwatched.push((args as { program: string }).program);
      return null;
    }
    if (cmd === "watch_browser") {
      const a = args as { program: string; role: string };
      watched.push({ program: a.program, role: a.role });
      return { view: watched.length, width: 1280, height: 800, person_drives: false };
    }
    return null;
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
  cleanup();
  clearMocks();
  forgetProjectsBrowsers();
  forgetProjectViews();
});

function tile(plane: string, program: string, name: string, workspace: string): GridTile {
  return {
    plane,
    program,
    own: false,
    name,
    chat: "steward 1",
    workspace,
    driver: null,
    pool: null,
  };
}

const opened: GridTile[] = [];

function tell() {
  opened.length = 0;
  const open = (one: GridTile) => opened.push(one);
  tellProjectBrowsers({
    plane: HERE,
    project: "plane",
    tiles: [tile(HERE, "A1", "b1", "alpha"), tile(HERE, "A2", "b2", "beta")],
    open,
  });
  tellProjectBrowsers({
    plane: THERE,
    project: "other",
    tiles: [tile(THERE, "B1", "b1", "alpha")],
    open,
  });
}

const grid = () => screen.getByRole("listbox", { name: "Browsers" });
const tiles = () =>
  within(grid())
    .queryAllByRole("option")
    .map((one) => one.getAttribute("data-program"));

describe("the Browsers grid", () => {
  it("shows this project's browsers as live thumbnails, at first", async () => {
    tell();
    render(<BrowsersGridTab plane={HERE} workspace="alpha" />);
    expect(tiles()).toEqual(["A1", "A2"]);
    const option = within(grid()).getByRole("option", { name: /Browser b1 of steward 1/ });
    expect(option).toBeInTheDocument();
    // Watched as thumbnails: about a frame a second each (spec #2008).
    await vi.waitFor(() =>
      expect(watched).toEqual([
        { program: "A1", role: "thumbnail" },
        { program: "A2", role: "thumbnail" },
      ]),
    );
  });

  it("narrows and widens with the Inbox's scope switch, each scope counted", async () => {
    tell();
    render(<BrowsersGridTab plane={HERE} workspace="alpha" />);
    const scopes = screen.getByRole("radiogroup", { name: "Show the browsers of" });
    expect(
      within(scopes)
        .getAllByRole("radio")
        .map((one) => one.closest("label")?.textContent),
    ).toEqual(["All projects3", "This project2", "This workspace1"]);

    await userEvent.click(within(scopes).getByRole("radio", { name: /This workspace/ }));
    expect(tiles()).toEqual(["A1"]);
    await userEvent.click(within(scopes).getByRole("radio", { name: /All projects/ }));
    expect(tiles()).toEqual(["A1", "A2", "B1"]);
    // Among every project's, a tile says which project it is of.
    const there = within(grid()).getAllByRole("option")[2];
    expect(within(there).getByText(/^other · alpha/)).toBeInTheDocument();
    // Kept for the project, as the Inbox's pick is.
    expect(keptBrowsersScope(HERE)).toBe("all");
  });

  it("opens a pressed browser in its own project, by the keyboard too", async () => {
    tell();
    render(<BrowsersGridTab plane={HERE} workspace="alpha" />);
    await userEvent.click(within(grid()).getAllByRole("option")[1]);
    expect(opened.map((one) => one.program)).toEqual(["A2"]);

    act(() => within(grid()).getAllByRole("option")[0].focus());
    await userEvent.keyboard("{Enter}");
    expect(opened.map((one) => one.program)).toEqual(["A2", "A1"]);
  });

  it("says where browsers run when none runs in the scope, and widens in one press", async () => {
    tellProjectBrowsers({
      plane: THERE,
      project: "other",
      tiles: [tile(THERE, "B1", "b1", "alpha")],
      open: () => undefined,
    });
    render(<BrowsersGridTab plane={HERE} workspace="alpha" />);
    expect(screen.getByText("No browser runs in this project")).toBeInTheDocument();
    expect(screen.getByText("1 browser runs in all projects.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Show all projects" }));
    expect(tiles()).toEqual(["B1"]);
  });

  it("says how a browser comes to be when none runs anywhere", () => {
    render(<BrowsersGridTab plane={HERE} workspace="alpha" />);
    expect(screen.getByText("No browser runs in this project")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Show/ })).toBeNull();
  });
});

/**
 * **An `IntersectionObserver` the test moves**: `show(program, true)` says that tile came into
 * sight, as a scroll would.
 */
function sight() {
  const observers: { callback: IntersectionObserverCallback; elements: Set<Element> }[] = [];
  class Seeing {
    elements = new Set<Element>();
    constructor(public callback: IntersectionObserverCallback) {
      observers.push(this);
    }
    observe(element: Element) {
      this.elements.add(element);
    }
    unobserve(element: Element) {
      this.elements.delete(element);
    }
    disconnect() {
      this.elements.clear();
    }
  }
  vi.stubGlobal("IntersectionObserver", Seeing);
  return {
    show: (program: string, inSight: boolean) =>
      act(() => {
        for (const one of observers)
          for (const element of one.elements)
            if (element.getAttribute("data-program") === program)
              one.callback(
                [
                  {
                    target: element,
                    isIntersecting: inSight,
                  } as unknown as IntersectionObserverEntry,
                ],
                one as unknown as IntersectionObserver,
              );
      }),
  };
}

describe("live tiles are bounded", () => {
  it("asks for frames only for the tiles in sight, and lets one go as it leaves", async () => {
    const seeing = sight();
    tell();
    render(<BrowsersGridTab plane={HERE} workspace="alpha" />);
    expect(tiles()).toEqual(["A1", "A2"]);
    expect(watched).toEqual([]);

    seeing.show("A2", true);
    await vi.waitFor(() => expect(watched.map((one) => one.program)).toEqual(["A2"]));
    seeing.show("A2", false);
    await vi.waitFor(() => expect(unwatched).toEqual(["A2"]));
    // A tile out of sight keeps its picture: nothing drew over it, and it asks for nothing.
    expect(watched.map((one) => one.program)).toEqual(["A2"]);
  });

  it("asks for frames for at most so many tiles at once", async () => {
    const many = Array.from({ length: LIVE_AT_MOST + 2 }, (_, at) =>
      tile(HERE, `P${at}`, `b${at + 1}`, "alpha"),
    );
    tellProjectBrowsers({ plane: HERE, project: "plane", tiles: many, open: () => undefined });
    render(<BrowsersGridTab plane={HERE} workspace="alpha" />);
    expect(tiles()).toHaveLength(LIVE_AT_MOST + 2);
    await vi.waitFor(() => expect(watched).toHaveLength(LIVE_AT_MOST));
    expect(watched.map((one) => one.program)).not.toContain(`P${LIVE_AT_MOST}`);
  });
});

describe("what a tile is called", () => {
  it("names a pinned browser whose chat's name was not kept by its own name", () => {
    expect(tileTitle({ ...tile(HERE, "A1", "b1", "alpha"), chat: null })).toBe("Browser b1");
    expect(tileTitle({ ...tile(HERE, "A1", "b1", "alpha"), chat: "" })).toBe("Browser b1");
  });

  it("calls a browser the person's own only by whose it is", () => {
    expect(
      tileTitle({ ...tile(HERE, "A1", "b1", "alpha"), own: true, name: null, chat: null }),
    ).toBe("Browser");
    // A chat's browser stays its chat's, whatever else it lacks.
    expect(tileTitle({ ...tile(HERE, "A1", "b1", "alpha"), chat: null })).not.toBe("Browser");
  });
});

describe("the scope switch", () => {
  it("says each scope's count in words", () => {
    tell();
    render(<BrowsersGridTab plane={HERE} workspace="alpha" />);
    const scopes = screen.getByRole("radiogroup", { name: "Show the browsers of" });
    expect(within(scopes).getByRole("radio", { name: "All projects, 3 browsers" })).toBeTruthy();
    expect(within(scopes).getByRole("radio", { name: "This workspace, 1 browser" })).toBeTruthy();
  });
});
