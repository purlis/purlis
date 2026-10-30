// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  cloneTrack,
  forThePerson,
  reposCloning,
  reposNotCloned,
  reposStopped,
  resultOf,
  saidInPlainWords,
  workspaceMade,
} from "./actionResult";

/** What `purlis workspace create` says, as `wscmd/create.rs` writes it. */
const MADE_LIVE = [
  "Workspace 'git-ci-cd' ready (LIVE — charter + manifest + memory committed + shared + auto-saved) → workspaces/git-ci-cd/",
  '⬢ No vision yet — ask the developer what this workspace is for, then record it: purlis workspace vision "<the goal>"  (it seeds workspaces/git-ci-cd/workspace.md, the living charter a fork inherits).',
];
const MADE_WITH_VISION = [
  "Workspace 'docs' ready (LOCAL — private (nothing committed); `purlis workspace live` to share) → workspaces/docs/",
  "Vision recorded → workspaces/docs/workspace.md",
];

const WAYS = { onAddVision: () => undefined };

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

  it("offers to add the vision it lacks, filled, and never to open what is open (#3648)", () => {
    // Making a workspace opens it (#3648): an Open would offer what has already happened.
    let asked = 0;
    const said = workspaceMade("git-ci-cd", MADE_LIVE, { onAddVision: () => (asked += 1) });
    expect(said.ways?.map((way) => [way.label, way.primary === true])).toEqual([
      ["Add a vision", true],
    ]);
    said.ways?.[0].onPress();
    expect(asked).toBe(1);
  });

  it("offers no way on where a vision was recorded: its Details are its one", () => {
    const said = workspaceMade("docs", MADE_WITH_VISION, WAYS);
    expect(said.ways ?? []).toEqual([]);
    expect(said.details).toBe(MADE_WITH_VISION.join("\n"));
  });

  it("is about the workspace it made, so it hangs from that workspace's chip (#3648)", () => {
    expect(workspaceMade("git-ci-cd", MADE_LIVE, WAYS).workspace).toBe("git-ci-cd");
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

describe("a result on one line (the palette's)", () => {
  it("says the title and the sentence, and never a line for an agent", () => {
    const said = workspaceMade("git-ci-cd", MADE_LIVE, WAYS);
    expect(saidInPlainWords(said)).toBe(
      "Workspace git-ci-cd is ready. It has no vision yet. Add one so its chats know what it is for.",
    );
    expect(
      saidInPlainWords({ from: "x", refused: false, words: "Done. ⬢ ask the developer why" }),
    ).toBe("Done.");
  });
});

describe("a workspace's repos cloning, on its chip (#4079)", () => {
  const press = () => undefined;

  it("draws the batch's overall percent and its stage, with Cancel", () => {
    const said = reposCloning("gamma", ["api", "web"], 1, 1, {
      moved: { stage: "Downloading", percent: 50 },
      onCancel: press,
    });
    expect(said.meter).toEqual({ label: "Downloading", percent: 75 });
    expect(said.ways).toEqual([{ label: "Cancel", onPress: press }]);
  });

  it("has a meter from its first line, before git has said anything", () => {
    const said = reposCloning("gamma", ["api", "web"], 0, 1, { onCancel: press });
    expect(said.meter).toEqual({ label: "Connecting", percent: 0 });
  });

  it("sends a sign-in failure to Accounts, and any other to the workspace's repos", () => {
    const signIn = reposNotCloned(
      "gamma",
      [{ repo: "api", said: "api: Sign in to git.corp.example.", signIn: "git.corp.example" }],
      press,
      1,
    );
    expect(signIn.settings).toEqual({
      label: "Open Accounts",
      link: { group: "you.accounts" },
    });
    const other = reposNotCloned("gamma", [{ repo: "api", said: "no" }], press, 1);
    expect(other.settings?.link).toEqual({ group: "workspace.repos", workspace: "gamma" });
  });

  it("says a stopped clone in its own words, with Retry, and is no refusal", () => {
    const said = reposStopped("gamma", ["api"], press, 1);
    expect(said.refused).toBe(false);
    expect(said.title).toBe("Stopped cloning api");
    expect(said.ways).toEqual([{ label: "Retry", onPress: press, primary: true }]);
    expect(said.track).toBe(cloneTrack("gamma", 1));
  });
});
