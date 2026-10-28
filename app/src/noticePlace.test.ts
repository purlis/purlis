// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { OUTSIDE } from "./actions";
import { aboutOf, chipsOf, placeOf, type About, type Chip, type Place } from "./noticePlace";

/** The title bar's chips in each state the placement reads (spec #3647). */
const PINNED: Chip = { workspace: "gamma", look: "pinned" };
const DASHED: Chip = { workspace: "gamma", look: "unpinned" };
const DORMANT: Chip = { workspace: "gamma", look: "dormant" };
const ANOTHER: Chip = { workspace: "beta", look: "pinned" };

const ABOUT_GAMMA: About = { workspace: "gamma" };
const A_PROJECT_RESULT: About = { project: true };
const A_CHAT: About = { pane: 3 };

describe("where a notice is drawn (spec #3647, the one test seam)", () => {
  const table: [string, About, readonly Chip[], Place][] = [
    [
      "about a pinned workspace: its chip's card",
      ABOUT_GAMMA,
      [ANOTHER, PINNED],
      { at: "chip", workspace: "gamma" },
    ],
    [
      "about the current, unpinned workspace: its dashed chip's card",
      ABOUT_GAMMA,
      [ANOTHER, DASHED],
      { at: "chip", workspace: "gamma" },
    ],
    [
      "about a dormant workspace: its dormant chip's card",
      ABOUT_GAMMA,
      [DORMANT],
      { at: "chip", workspace: "gamma" },
    ],
    [
      "about a workspace with no chip: the Notices centre, with Open",
      ABOUT_GAMMA,
      [ANOTHER],
      { at: "centre", open: "gamma" },
    ],
    [
      "about a workspace while the plane root is in front and nothing is pinned",
      ABOUT_GAMMA,
      [],
      { at: "centre", open: "gamma" },
    ],
    [
      "a project or app action's result: the status bar's card",
      A_PROJECT_RESULT,
      [PINNED],
      { at: "status" },
    ],
    ["a chat's: its pane band, unchanged", A_CHAT, [PINNED], { at: "pane", pane: 3 }],
  ];
  it.each(table)("%s", (_, about, chips, place) => {
    expect(placeOf(about, chips)).toEqual(place);
  });

  it("finds a workspace's chip by its whole name, never a prefix", () => {
    expect(placeOf({ workspace: "gam" }, [PINNED])).toEqual({ at: "centre", open: "gam" });
  });
});

describe("what a result is about", () => {
  it("is the workspace it names, as a workspace made names it", () => {
    expect(
      aboutOf({ from: "workspace.create", refused: false, words: "", workspace: "gamma" }),
    ).toEqual(ABOUT_GAMMA);
  });

  it("is the project, for a result that names no workspace", () => {
    expect(aboutOf({ from: "workspace.pin", refused: true, words: "no" })).toEqual(
      A_PROJECT_RESULT,
    );
  });
});

describe("the title bar's chips", () => {
  it("are the pins, then the current workspace dashed while it is not pinned", () => {
    expect(chipsOf({ pins: ["beta"], focused: "gamma" })).toEqual([ANOTHER, DASHED]);
  });

  it("draw a pinned current workspace once, as a pin", () => {
    expect(chipsOf({ pins: ["beta", "gamma"], focused: "gamma" })).toEqual([ANOTHER, PINNED]);
  });

  it("are only the pins while the plane root is in front, or nothing is", () => {
    expect(chipsOf({ pins: ["beta"], focused: OUTSIDE })).toEqual([ANOTHER]);
    expect(chipsOf({ pins: ["beta"], focused: undefined })).toEqual([ANOTHER]);
  });
});
