// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { OUTSIDE } from "./actions";
import { reposCloned, reposCloning, reposNotCloned, workspaceMade } from "./actionResult";
import type { Said } from "./actions";
import type { Shown } from "./bindings";
import {
  aboutOf,
  chipsOf,
  kindOf,
  needsOn,
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
/** Chat 3 asking for gamma's removal: a consent, answered on its band (#3652). */
const ASKS_TO_REMOVE_GAMMA: About = { pane: 3, removes: "gamma" };

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
    [
      "a chat asking to remove a pinned workspace: its band, and a needs-you dot on the chip (#3652)",
      ASKS_TO_REMOVE_GAMMA,
      [ANOTHER, PINNED],
      { at: "pane", pane: 3, needs: "gamma" },
    ],
    [
      "a chat asking to remove the current, unpinned workspace: the dot on its dashed chip",
      ASKS_TO_REMOVE_GAMMA,
      [ANOTHER, DASHED],
      { at: "pane", pane: 3, needs: "gamma" },
    ],
    [
      "a chat asking to remove a dormant workspace: the dot on its dormant chip",
      ASKS_TO_REMOVE_GAMMA,
      [DORMANT],
      { at: "pane", pane: 3, needs: "gamma" },
    ],
    [
      "a chat asking to remove a workspace with no chip: its band alone, no dot over the wrong chip",
      ASKS_TO_REMOVE_GAMMA,
      [ROOT, ANOTHER],
      { at: "pane", pane: 3 },
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

/** Chat `session`'s ask, as the registry lists it, to remove `workspace` (qw356). */
const removal = (workspace: string, session: number): Shown => ({
  session,
  ask: `removal:${session}:${workspace}`,
  says: `Removing workspace ${workspace}`,
  options: [
    { id: "remove", label: "Remove…", allows: true },
    { id: "keep", label: "Keep", allows: false },
  ],
  source: "workspace-removal",
  chain: [`Safe remove · ${workspace}`],
  answer: { via: "workspace-removal", workspace, memories: 0, todos: 0 },
});

describe("the chips a chat's removal question puts a needs-you dot on (#3652)", () => {
  it("is each asked workspace that has a chip, with the chat asking first", () => {
    const asks = [removal("gamma", 4), removal("beta", 5), removal("gamma", 6)];
    expect([...needsOn(asks, [ROOT, ANOTHER, PINNED])]).toEqual([
      ["gamma", 4],
      ["beta", 5],
    ]);
  });

  it("is none for a workspace with no chip, nor for any other kind of ask", () => {
    const other: Shown = {
      ...removal("beta", 7),
      source: "proposed-change",
      answer: { via: "proposed-change", id: 1, path: "beta" },
    };
    expect([...needsOn([removal("gamma", 4), other], [ROOT, ANOTHER])]).toEqual([]);
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
