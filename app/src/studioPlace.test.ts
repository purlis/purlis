// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { SkillHidden, StudioClone, StudioRouter } from "./bindings";
import { skill, studio } from "./skillsListing.testkit";
import { cloneChosen, placesOf, studioIn } from "./studioPlace";

const OVER: SkillHidden = {
  router: "built-in",
  why: "over-budget",
  said: "over this chat's skill budget, so it is in the index skill",
};

const PURLIS: StudioClone = {
  clone: "purlis",
  repo_skills: 12,
  preview:
    "The rows' hidden marks are what the built-in router hides for a chat started in purlis: its 12 repo skills count against the budget too.",
};

const ROUTER = { preview: "At the root." } as StudioRouter;

/** `changelog`, listed at the root and hidden on Claude Code in purlis; `tdd`, hidden at the root
 *  on Claude Code and replaced on Codex in purlis. */
const read = studio(
  [
    skill("changelog", {
      hidden_in: [{ clone: "purlis", harness: "claude", title: "Claude Code", hidden: OVER }],
    }),
    skill("tdd", {
      hidden: [{ harness: "claude", title: "Claude Code", hidden: OVER }],
      replaced_in: [{ clone: "purlis", harness: "codex", title: "Codex" }],
    }),
  ],
  { places: [PURLIS], router: ROUTER, workspace: "smart-ide" },
);

describe("the studio as a chat started in one place sees it (#4227)", () => {
  it("marks hidden what a chat started in the chosen clone hides, and says that place", () => {
    const shown = studioIn(read, "purlis");
    expect(shown.skills.map((one) => [one.name, one.hidden.map((h) => h.title)])).toEqual([
      ["changelog", ["Claude Code"]],
      ["tdd", []],
    ]);
    expect(shown.router?.preview).toBe(PURLIS.preview);
  });

  it("at the workspace's root, or for a clone that holds no repo skills, is the studio as read", () => {
    expect(studioIn(read, undefined)).toBe(read);
    expect(studioIn(read, "docs-site")).toBe(read);
    expect(cloneChosen(read, "docs-site")).toBeUndefined();
  });

  it("lists, for a skill's page, whether it is listed or hidden in each place", () => {
    expect(placesOf(read.skills[0], read.places)).toEqual([
      { clone: undefined, said: "Listed", listed: true },
      { clone: "purlis", said: "Hidden by the router on Claude Code", listed: false },
    ]);
    expect(placesOf(read.skills[1], read.places)).toEqual([
      { clone: undefined, said: "Hidden by the router on Claude Code", listed: false },
      { clone: "purlis", said: "Replaced by the repo's own tdd on Codex", listed: false },
    ]);
  });

  it("lists no place where no clone holds repo skills, or for a skill a chat does not get", () => {
    expect(placesOf(read.skills[0], [])).toEqual([]);
    expect(placesOf(skill("off", { on: false }), [PURLIS])).toEqual([]);
  });
});
