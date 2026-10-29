// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { OUTSIDE } from "./actions";
import { reposCloned, reposCloning, reposNotCloned, workspaceMade } from "./actionResult";
import type { Said } from "./actions";
import {
  aboutOf,
  chipsOf,
  kindOf,
  placeOf,
  type About,
  type Chip,
  type Place,
} from "./noticePlace";
import type { NoticeKind } from "./ui/titlebar/TitleBarWorkspaces";

/** The title bar's chips in each state the placement reads (spec #3647). */
const PINNED: Chip = { workspace: "gamma", look: "pinned" };
const DASHED: Chip = { workspace: "gamma", look: "unpinned" };
const DORMANT: Chip = { workspace: "gamma", look: "dormant" };
const ANOTHER: Chip = { workspace: "beta", look: "pinned" };
const ROOT: Chip = { workspace: OUTSIDE, look: "root" };

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
      "about a dormant workspace among live pins: its dormant chip's card, not another's",
      ABOUT_GAMMA,
      [ROOT, ANOTHER, DORMANT],
      { at: "chip", workspace: "gamma" },
    ],
    [
      "about the current, unpinned workspace once it is pinned: the same chip's card",
      ABOUT_GAMMA,
      [ROOT, ANOTHER, PINNED],
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
      [ROOT],
      { at: "centre", open: "gamma" },
    ],
    [
      "a project or app action's result: the status bar's card",
      A_PROJECT_RESULT,
      [PINNED],
      { at: "status" },
    ],
    ["a chat's: its pane band, unchanged", A_CHAT, [PINNED], { at: "pane", pane: 3 }],
    // **The project's findings about one workspace** (#3650): each is about that workspace, so
    // it hangs from its chip, and lists in the Notices centre alone while it has none.
    [
      "a repo off its default branch, in a pinned workspace: its chip's card",
      ABOUT_GAMMA,
      [ROOT, ANOTHER, PINNED],
      { at: "chip", workspace: "gamma" },
    ],
    [
      "a repo on a moved remote, in the current unpinned workspace: its dashed chip's card",
      ABOUT_GAMMA,
      [ROOT, ANOTHER, DASHED],
      { at: "chip", workspace: "gamma" },
    ],
    [
      "a layout behind, in a workspace with no chip: the Notices centre alone",
      ABOUT_GAMMA,
      [ROOT, ANOTHER],
      { at: "centre", open: "gamma" },
    ],
    [
      "about the plane root: its house chip's card, outside the strip (the dispatcher's ruling)",
      { workspace: OUTSIDE },
      [ROOT, ANOTHER],
      { at: "chip", workspace: OUTSIDE },
    ],
  ];
  it.each(table)("%s", (_, about, chips, place) => {
    expect(placeOf(about, chips)).toEqual(place);
  });

  it("finds a workspace's chip by its whole name, never a prefix", () => {
    expect(placeOf({ workspace: "gam" }, [PINNED])).toEqual({ at: "centre", open: "gam" });
  });
});

describe("where each line of a new workspace's setup is drawn, and its kind (#3649)", () => {
  const nothing = () => undefined;
  const lines: [string, Said, readonly Chip[], Place, NoticeKind][] = [
    [
      "its ready notice: its chip's card, news",
      workspaceMade("gamma", [], { onAddVision: nothing }),
      [ROOT, PINNED],
      { at: "chip", workspace: "gamma" },
      "info",
    ],
    [
      "its repos cloning, on its dashed chip: that chip's card, news",
      reposCloning("gamma", ["api"], 0, 1),
      [ROOT, DASHED],
      { at: "chip", workspace: "gamma" },
      "info",
    ],
    [
      "its repos cloned: its chip's card, news",
      reposCloned("gamma", ["api"], 1),
      [ROOT, PINNED],
      { at: "chip", workspace: "gamma" },
      "info",
    ],
    [
      "its repos not cloned: its chip's card, a warning",
      reposNotCloned("gamma", [{ repo: "api", said: "No access." }], nothing, 1),
      [ROOT, PINNED],
      { at: "chip", workspace: "gamma" },
      "warning",
    ],
    [
      "its repos not cloned, with no chip left: the Notices centre, a warning",
      reposNotCloned("gamma", [{ repo: "api", said: "No access." }], nothing, 1),
      [ROOT, ANOTHER],
      { at: "centre", open: "gamma" },
      "warning",
    ],
    [
      "a project's refusal: the status bar's card, a warning",
      { from: "workspace.pin", refused: true, words: "no" },
      [ROOT, PINNED],
      { at: "status" },
      "warning",
    ],
  ];
  it.each(lines)("%s", (_, said, chips, place, kind) => {
    expect(placeOf(aboutOf(said), chips)).toEqual(place);
    expect(kindOf(said)).toBe(kind);
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
  it("are the root's house, the pins, then the current workspace dashed while it is not pinned", () => {
    expect(chipsOf({ pins: ["beta"], focused: "gamma" })).toEqual([ROOT, ANOTHER, DASHED]);
  });

  it("draw a pinned current workspace once, as a pin", () => {
    expect(chipsOf({ pins: ["beta", "gamma"], focused: "gamma" })).toEqual([ROOT, ANOTHER, PINNED]);
  });

  it("draw a dormant pin as a dormant chip, after the live pins and before the dashed one", () => {
    expect(chipsOf({ pins: ["beta"], dormant: ["gamma"], focused: "delta" })).toEqual([
      ROOT,
      ANOTHER,
      DORMANT,
      { workspace: "delta", look: "unpinned" },
    ]);
  });

  it("draw a gone workspace in front once, dormant, and no dashed chip for it", () => {
    expect(chipsOf({ pins: ["beta"], dormant: ["gamma"], focused: "gamma" })).toEqual([
      ROOT,
      ANOTHER,
      DORMANT,
    ]);
  });

  it("draw a name once, as a live pin, where the store names it both pinned and gone", () => {
    expect(chipsOf({ pins: ["gamma"], dormant: ["gamma"], focused: "gamma" })).toEqual([
      ROOT,
      PINNED,
    ]);
  });

  it("are the root's house and the pins while the root is in front, or nothing is", () => {
    expect(chipsOf({ pins: ["beta"], focused: OUTSIDE })).toEqual([ROOT, ANOTHER]);
    expect(chipsOf({ pins: ["beta"], focused: undefined })).toEqual([ROOT, ANOTHER]);
  });
});
