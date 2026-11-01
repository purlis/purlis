// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { FRESH, guides, next, stepsOf, type Progress } from "./guidedSteps";

const made = { plane: "/p/team", cwd: "/p/team/workspaces/app/app", workspace: "app" };

/** Walks `events` from a fresh machine. */
function walk(...events: Parameters<typeof next>[1][]): Progress {
  return events.reduce(next, FRESH);
}

describe("the guided steps", () => {
  it("guide an empty window, a machine never through them, or a held project; not Add another project", () => {
    const finished: Progress = { ...FRESH, finished_once: true };
    expect(guides(FRESH, false)).toBe(true);
    expect(guides(FRESH, true)).toBe(true);
    expect(guides(finished, false)).toBe(true);
    // The strip's `+` on a window full of projects, on a machine that finished the steps.
    expect(guides(finished, true)).toBe(false);
    expect(guides({ ...finished, step: "project", door: "create" }, true)).toBe(false);
    // A project their harness step holds keeps them guiding, or it would stay hidden.
    const held: Progress = {
      ...finished,
      step: "harness",
      made: { plane: "/p", cwd: "/p", workspace: "" },
    };
    expect(guides(held, true)).toBe(true);
  });

  it("move between the doors from any page, since the doors stay in sight", () => {
    const atCreate = walk({ kind: "door", door: "create" });
    expect(next(atCreate, { kind: "door", door: "get" })).toMatchObject({
      step: "project",
      door: "get",
    });
    const signing = next(atCreate, { kind: "sign-in" });
    expect(signing).toMatchObject({ step: "sign-in", door: null, signing_in: true });
    expect(next(signing, { kind: "door", door: "get" })).toMatchObject({
      step: "project",
      door: "get",
    });
  });

  it("count a project and a harness on a fresh machine, and not the welcome or the end", () => {
    const counted = (progress: Progress) => stepsOf(progress, { harnessMissing: false });
    expect(counted(FRESH)).toEqual({
      steps: ["project", "harness"],
      at: 0,
    });
    const atCreate = walk({ kind: "door", door: "create" });
    expect(atCreate).toMatchObject({ step: "project", door: "create" });
    expect(counted(atCreate).at).toBe(0);
    const holding = next(atCreate, { kind: "arrived", made, harnessMissing: false });
    expect(holding).toMatchObject({ step: "harness", made });
    expect(counted(holding)).toEqual({ steps: ["project", "harness"], at: 1 });
    const done = next(holding, { kind: "finish" });
    expect(done).toEqual({ ...FRESH, finished_once: true });
  });

  it("count the sign-in only when the person took it", () => {
    const signing = walk({ kind: "sign-in" });
    expect(signing.step).toBe("sign-in");
    expect(stepsOf(signing, { harnessMissing: true })).toEqual({
      steps: ["sign-in", "project", "harness"],
      at: 0,
    });
    // Skip for now goes on to the project, and the sign-in stays counted among the steps.
    const skipped = next(signing, { kind: "skip" });
    expect(skipped).toMatchObject({ step: "project", door: null, signing_in: true });
    expect(stepsOf(skipped, { harnessMissing: true }).at).toBe(1);
    // Back from the sign-in is the welcome, without it.
    expect(next(signing, { kind: "back" })).toMatchObject({ step: "welcome", signing_in: false });
  });

  it("add the sign-in when Create on a forge has no account, and come back to Create after it", () => {
    // #1819: GitHub or GitLab picked on Create with nobody signed in there.
    const asked = walk({ kind: "door", door: "create" }, { kind: "needs-sign-in" });
    expect(asked).toMatchObject({ step: "sign-in", door: "create", signing_in: true });
    expect(stepsOf(asked, { harnessMissing: false })).toEqual({
      steps: ["sign-in", "project", "harness"],
      at: 0,
    });
    // Signed in (Continue) or not (Skip for now): back on Create, the sign-in still counted.
    expect(next(asked, { kind: "skip" })).toMatchObject({
      step: "project",
      door: "create",
      signing_in: true,
    });
    // Back comes back to Create too, not to the welcome.
    expect(next(asked, { kind: "back" })).toMatchObject({ step: "project", door: "create" });
    // Only an act on a door's page asks for it.
    expect(next(FRESH, { kind: "needs-sign-in" })).toBe(FRESH);
  });

  it("count the git check once git was found missing, and come back to the project after it", () => {
    const waiting = walk({ kind: "door", door: "get" }, { kind: "git-missing" });
    expect(waiting).toMatchObject({ step: "git", door: "get", git_missing: true });
    expect(stepsOf(waiting, { harnessMissing: false })).toEqual({
      steps: ["project", "git", "harness"],
      at: 1,
    });
    const back = next(waiting, { kind: "git-settled" });
    expect(back).toMatchObject({ step: "project", door: "get", git_missing: true });
    // Still counted: the step happened, and the count does not shrink under the person.
    expect(stepsOf(back, { harnessMissing: false }).steps).toContain("git");
  });

  it("show the harness step after the first run only while no harness is installed", () => {
    const again = { ...FRESH, finished_once: true };
    const create = next(again, { kind: "door", door: "create" });
    expect(stepsOf(create, { harnessMissing: false }).steps).toEqual(["project"]);
    expect(next(create, { kind: "arrived", made, harnessMissing: false })).toEqual(again);
    expect(stepsOf(create, { harnessMissing: true }).steps).toContain("harness");
    expect(next(create, { kind: "arrived", made, harnessMissing: true })).toMatchObject({
      step: "harness",
      made,
    });
  });

  it("let the harness step be skipped and still open the project", () => {
    const holding = walk(
      { kind: "door", door: "open" },
      { kind: "arrived", made, harnessMissing: true },
    );
    expect(next(holding, { kind: "skip" })).toEqual({ ...FRESH, finished_once: true });
  });

  it("go back from a door's page to the welcome, keeping a sign-in that was taken", () => {
    expect(walk({ kind: "door", door: "create" }, { kind: "back" })).toEqual(FRESH);
    expect(
      walk({ kind: "sign-in" }, { kind: "skip" }, { kind: "door", door: "get" }, { kind: "back" }),
    ).toMatchObject({ step: "welcome", door: null, signing_in: true });
  });

  it("start over when the project the harness step held is gone", () => {
    const holding = walk(
      { kind: "door", door: "create" },
      { kind: "arrived", made, harnessMissing: true },
    );
    expect(next(holding, { kind: "start-over" })).toEqual(FRESH);
  });

  it("take no project but at the project or git step", () => {
    for (const progress of [FRESH, walk({ kind: "sign-in" })])
      expect(next(progress, { kind: "arrived", made, harnessMissing: true })).toBe(progress);
  });
});
