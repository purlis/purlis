// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { forThePerson, resultOf, workspaceMade } from "./actionResult";

/** What `purlis workspace create` says, as `wscmd/create.rs` writes it. */
const MADE_LIVE = [
  "Workspace 'git-ci-cd' ready (LIVE — charter + manifest + memory committed + shared + auto-saved) → workspaces/git-ci-cd/",
  '⬢ No vision yet — ask the developer what this workspace is for, then record it: purlis workspace vision "<the goal>"  (it seeds workspaces/git-ci-cd/workspace.md, the living charter a fork inherits).',
];
const MADE_WITH_VISION = [
  "Workspace 'docs' ready (LOCAL — private (nothing committed); `purlis workspace live` to share) → workspaces/docs/",
  "Vision recorded → workspaces/docs/workspace.md",
];

const WAYS = { onOpen: () => undefined, onAddVision: () => undefined };

describe("a workspace made (#3344)", () => {
  it("says it in the app's own words, a title and one plain sentence", () => {
    const said = workspaceMade("git-ci-cd", MADE_LIVE, WAYS);
    expect(said.title).toBe("Workspace git-ci-cd is ready");
    expect(said.words).toBe("It has no vision yet. Add one so its chats know what it is for.");
    expect(said.refused).toBe(false);
    expect(said.from).toBe("workspace.create");
  });

  it("says nothing of a vision once one was recorded", () => {
    const said = workspaceMade("docs", MADE_WITH_VISION, WAYS);
    expect(said.words).toBe("Its vision is recorded in its workspace.md.");
  });

  it("keeps the core's lines in its details, but never a line written for an agent", () => {
    const said = workspaceMade("git-ci-cd", MADE_LIVE, WAYS);
    expect(said.details).toBe(MADE_LIVE[0]);
    expect(said.details).not.toMatch(/ask the developer/);
  });

  it("offers to open the workspace, first and filled, and to add the vision it lacks", () => {
    let opened = 0;
    let asked = 0;
    const said = workspaceMade("git-ci-cd", MADE_LIVE, {
      onOpen: () => (opened += 1),
      onAddVision: () => (asked += 1),
    });
    expect(said.ways?.map((way) => [way.label, way.primary === true])).toEqual([
      ["Open git-ci-cd", true],
      ["Add a vision", false],
    ]);
    said.ways?.[0].onPress();
    said.ways?.[1].onPress();
    expect([opened, asked]).toEqual([1, 1]);
  });

  it("offers no vision to add where one was recorded", () => {
    const said = workspaceMade("docs", MADE_WITH_VISION, WAYS);
    expect(said.ways?.map((way) => way.label)).toEqual(["Open docs"]);
  });

  it("adds why the pin was refused to its sentence", () => {
    const said = workspaceMade("git-ci-cd", MADE_LIVE, {
      ...WAYS,
      pinRefused: "The pin store is read-only.",
    });
    expect(said.words).toBe(
      "It has no vision yet. Add one so its chats know what it is for. " +
        "The pin store is read-only.",
    );
  });
});

describe("what any result says", () => {
  it("is titled by its words when it has no title of its own", () => {
    expect(resultOf({ from: "workspace.pin", refused: true, words: "Not pinned." })).toEqual({
      title: "Not pinned.",
      says: undefined,
      details: undefined,
    });
  });

  it("keeps a title, a sentence and details apart", () => {
    expect(resultOf({ from: "x", refused: false, title: "T", words: "S.", details: "D" })).toEqual({
      title: "T",
      says: "S.",
      details: "D",
    });
  });

  it("never shows a line the core wrote for an agent, whatever result it came with", () => {
    const said = {
      from: "workspace.rename",
      refused: false,
      words:
        "✓ Renamed workspace 'a' to 'b'. ⬢ Tell the developer to ask the developer what it is for.",
      details: "✓ Renamed\n⬢ ask the developer what it is for",
    };
    expect(resultOf(said)).toEqual({
      title: "✓ Renamed workspace 'a' to 'b'.",
      says: undefined,
      details: "✓ Renamed",
    });
  });

  it("drops the lines the core writes for an agent", () => {
    expect(
      forThePerson(["Done.", "⬢ No vision yet — ask the developer what it is for", "More."]),
    ).toEqual(["Done.", "More."]);
  });
});
