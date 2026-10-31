// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import {
  forgetProjectsBrowsers,
  gridCounts,
  keepBrowsersScope,
  keptBrowsersScope,
  onProjectsBrowsers,
  openTile,
  projectsBrowsers,
  tellProjectBrowsers,
  tilesIn,
  type GridTile,
  type ProjectBrowsers,
} from "./browsersGrid";
import { forgetProjectViews } from "./projectViews";

const HERE = "/home/dev/plane";
const THERE = "/home/dev/other";

function tile(plane: string, program: string, workspace: string): GridTile {
  return {
    plane,
    program,
    own: false,
    name: "b1",
    chat: "steward 1",
    workspace,
    driver: null,
    pool: null,
  };
}

function project(plane: string, tiles: GridTile[]): ProjectBrowsers {
  return { plane, project: plane.split("/").at(-1) ?? plane, tiles, open: () => undefined };
}

const PROJECTS = [
  project(HERE, [tile(HERE, "A1", "alpha"), tile(HERE, "A2", "beta")]),
  project(THERE, [tile(THERE, "B1", "alpha")]),
];

afterEach(() => {
  forgetProjectsBrowsers();
  forgetProjectViews();
});

describe("which browsers the grid shows", () => {
  it("shows every project's for All projects", () => {
    expect(tilesIn("all", PROJECTS, HERE, "alpha").map((one) => one.program)).toEqual([
      "A1",
      "A2",
      "B1",
    ]);
  });

  it("shows this project's alone for This project", () => {
    expect(tilesIn("project", PROJECTS, HERE, "alpha").map((one) => one.program)).toEqual([
      "A1",
      "A2",
    ]);
  });

  it("shows those of this project's workspace in front for This workspace", () => {
    // B1 is filed in a workspace of the same name, in another project: not this one's.
    expect(tilesIn("workspace", PROJECTS, HERE, "alpha").map((one) => one.program)).toEqual(["A1"]);
  });

  it("counts each scope, and offers no workspace's where none is in front", () => {
    expect(gridCounts(PROJECTS, HERE, "beta")).toEqual({ all: 3, project: 2, workspace: 1 });
    expect(gridCounts(PROJECTS, HERE, undefined)).toEqual({ all: 3, project: 2 });
  });
});

describe("what each project tells the grid", () => {
  it("lists each project once, with what it told last, until it is forgotten", () => {
    tellProjectBrowsers(PROJECTS[0]);
    tellProjectBrowsers(PROJECTS[1]);
    tellProjectBrowsers(project(HERE, []));
    expect(projectsBrowsers().map((one) => [one.plane, one.tiles.length])).toEqual([
      [HERE, 0],
      [THERE, 1],
    ]);
    forgetProjectsBrowsers(HERE);
    expect(projectsBrowsers().map((one) => one.plane)).toEqual([THERE]);
  });
});

describe("telling the grid again", () => {
  it("tells nobody when a project tells the browsers it told last, and opens by its latest way", () => {
    const heard = vi.fn();
    const first = vi.fn();
    const latest = vi.fn();
    tellProjectBrowsers({ ...project(HERE, [tile(HERE, "A1", "alpha")]), open: first });
    const stop = onProjectsBrowsers(heard);
    // A change to its tabs that shows no other browser: the same browsers, built afresh.
    tellProjectBrowsers({ ...project(HERE, [tile(HERE, "A1", "alpha")]), open: latest });
    expect(heard).not.toHaveBeenCalled();
    openTile(tile(HERE, "A1", "alpha"));
    expect(latest).toHaveBeenCalledTimes(1);
    expect(first).not.toHaveBeenCalled();

    tellProjectBrowsers({ ...project(HERE, [tile(HERE, "A1", "beta")]), open: latest });
    expect(heard).toHaveBeenCalledTimes(1);
    stop();
  });
});

describe("whose a browser is", () => {
  it("is never the person's own in one project where another project's chat uses it", () => {
    // Project HERE's tabs show a view of THERE's chat's browser B1.
    tellProjectBrowsers(
      project(HERE, [
        tile(HERE, "A1", "alpha"),
        { ...tile(HERE, "B1", "alpha"), own: true, name: null, chat: null },
      ]),
    );
    tellProjectBrowsers(project(THERE, [tile(THERE, "B1", "alpha")]));
    expect(
      projectsBrowsers().flatMap((one) => one.tiles.map((t) => `${t.plane} ${t.program} ${t.own}`)),
    ).toEqual([`${HERE} A1 false`, `${THERE} B1 false`]);
  });
});

describe("the scope the person picked", () => {
  it("is this project's until another is picked, and is kept per project", () => {
    expect(keptBrowsersScope(HERE)).toBe("project");
    keepBrowsersScope(HERE, "all");
    expect(keptBrowsersScope(HERE)).toBe("all");
    expect(keptBrowsersScope(THERE)).toBe("project");
  });
});
