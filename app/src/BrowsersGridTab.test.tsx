// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { BrowsersGrid } from "./BrowsersGridTab";
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

beforeEach(() => {
  watched = [];
  mockIPC((cmd, args) => {
    if (cmd === "watch_browser") {
      const a = args as { program: string; role: string };
      watched.push({ program: a.program, role: a.role });
      return { view: watched.length, width: 1280, height: 800, person_drives: false };
    }
    return null;
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetProjectsBrowsers();
  forgetProjectViews();
});

function tile(plane: string, program: string, name: string, workspace: string): GridTile {
  return { plane, program, name, chat: "steward 1", workspace, driver: null, pool: null };
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
    render(<BrowsersGrid plane={HERE} workspace="alpha" />);
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
    render(<BrowsersGrid plane={HERE} workspace="alpha" />);
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
    render(<BrowsersGrid plane={HERE} workspace="alpha" />);
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
    render(<BrowsersGrid plane={HERE} workspace="alpha" />);
    expect(screen.getByText("No browser runs in this project")).toBeInTheDocument();
    expect(screen.getByText("1 browser runs in all projects.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Show all projects" }));
    expect(tiles()).toEqual(["B1"]);
  });

  it("says how a browser comes to be when none runs anywhere", () => {
    render(<BrowsersGrid plane={HERE} workspace="alpha" />);
    expect(screen.getByText("No browser runs in this project")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Show/ })).toBeNull();
  });
});
