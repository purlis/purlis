// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * What the Skills view lists (SK-55, #3266): its groups and their order, the one mark a row
 * carries and what it says on hover, and what each row's menu can do, from the Skill studio's
 * own answer.
 */
import { describe, expect, it } from "vitest";
import type { StudioSkill } from "./bindings";
import { skill, studio } from "./skillsListing.testkit";
import {
  BUILT_IN_STAYS,
  groupsOf,
  listedOf,
  markOf,
  menuRowsOf,
  personaRowsOf,
  personasOf,
  tipOf,
  turnOf,
  type ListedSkill,
} from "./skillsListing";

const row = (one: StudioSkill, more: Partial<ListedSkill> = {}): ListedSkill => ({
  key: one.name,
  skill: one,
  ...more,
});

describe("the one mark a row carries", () => {
  it("is what matters most: a folder to accept, then off, then left out, then lint", () => {
    expect(markOf(skill("plain"))).toBeUndefined();
    expect(markOf(skill("linted", { lint: ["It widens the chat."] }))).toBe("lint");
    expect(markOf(skill("out", { on: false, left_out: "Its grant does not reach ops." }))).toBe(
      "left-out",
    );
    expect(markOf(skill("off", { on: false, turned_off: true, lint: ["x"] }))).toBe("off");
    expect(
      markOf(skill("drafted", { on: false, turned_off: true, accept: "workspaces/ops/skills" })),
    ).toBe("needs-review");
  });

  it("says on hover what the skill does, then why it has its mark", () => {
    const out = skill("out", { on: false, left_out: "Its grant does not reach ops." });
    expect(tipOf(row(out))).toEqual(["What out does.", "Its grant does not reach ops."]);
    expect(tipOf(row(skill("plain")))).toEqual(["What plain does."]);
    expect(
      tipOf(row(skill("linted", { lint: ["It widens the chat.", "Asks for Bash."] }))),
    ).toEqual(["What linted does.", "It widens the chat. Asks for Bash."]);
    expect(tipOf(row(skill("theirs", { level: "persona" }), { persona: "reviewer" }))).toEqual([
      "What theirs does.",
      "reviewer brings it into every workspace.",
    ]);
  });
});

describe("the list's groups", () => {
  const all = [
    row(skill("secrets", { level: "built-in", folder: "/app/skills/secrets" })),
    row(skill("deploy")),
    row(skill("ops-only", { level: "workspace", folder: "workspaces/ops/skills/ops-only" })),
    row(skill("mine", { level: "workspace", local: true })),
    row(skill("review", { level: "persona" }), { persona: "reviewer" }),
    row(skill("lint-rs", { level: "repo", folder: "" })),
    row(skill("pdf", { level: "installed", folder: "" }), { program: "Claude Code" }),
    row(skill("drafted", { accept: "skills" })),
  ];

  it("are Needs you on top, then each level, then each program's own, in that order", () => {
    const groups = groupsOf(all, { root: false, chat: false });
    expect(groups.map((one) => [one.title, one.rows.map((r) => r.skill.name)])).toEqual([
      ["Needs you", ["drafted"]],
      ["Built-in", ["secrets"]],
      ["Project", ["deploy"]],
      ["Workspace", ["ops-only"]],
      ["Personas", ["review"]],
      ["Repo", ["lint-rs"]],
      ["Local", ["mine"]],
      ["Claude Code", ["pdf"]],
    ]);
    // A program's name is data; purlis's own words are not.
    expect(groups.find((one) => one.title === "Claude Code")?.named).toBe(true);
    expect(groups.find((one) => one.title === "Project")?.named).toBe(false);
  });

  it("draw no Needs you while nothing needs the person", () => {
    const groups = groupsOf(all.slice(0, -1), { root: false, chat: false });
    expect(groups[0]?.title).toBe("Built-in");
  });

  it("are the project's and the personas' alone at the project's root", () => {
    const groups = groupsOf(all, { root: true, chat: false });
    expect(groups.map((one) => one.title)).toEqual(["Needs you", "Project", "Personas", "Local"]);
  });

  it("keep, for one chat, what it does not get under one group at the end", () => {
    const chat = [
      row(skill("deploy")),
      row(skill("out", { on: false, left_out: "Its grant does not reach ops." })),
      row(skill("off", { on: false, turned_off: true })),
    ];
    const groups = groupsOf(chat, { root: false, chat: true });
    expect(groups.map((one) => [one.title, one.rows.map((r) => r.skill.name)])).toEqual([
      ["Project", ["deploy"]],
      ["Not in this chat", ["out", "off"]],
    ]);
  });
});

describe("the rows of a studio answer", () => {
  it("are its skills, then each program's own, a repo's skill once however many find it", () => {
    const repo = skill("lint-rs", {
      level: "repo",
      folder: "",
      owner: { kind: "repo", name: "svc" },
    });
    const answer = studio([skill("deploy")], {
      harnesses: [
        {
          harness: "claude",
          title: "Claude Code",
          unsupported: null,
          only_off: null,
          trouble: null,
          plugins: [],
          skills: [skill("pdf", { level: "installed", folder: "" }), repo],
          repo_left_out: [],
        },
        {
          harness: "codex",
          title: "Codex",
          unsupported: null,
          only_off: null,
          trouble: null,
          plugins: [],
          skills: [repo],
          repo_left_out: [],
        },
      ],
    });
    const listed = listedOf(answer);
    expect(listed.map((one) => [one.skill.name, one.program])).toEqual([
      ["deploy", undefined],
      ["pdf", "Claude Code"],
      ["lint-rs", undefined],
    ]);
    expect(new Set(listed.map((one) => one.key)).size).toBe(listed.length);
  });

  it("name each persona the project lists, once, and mark a persona's own as its", () => {
    const answer = studio([skill("deploy"), skill("review", { level: "persona" })], {
      levels: [
        { level: { at: "project", local: false }, label: "project" },
        { level: { at: "persona", name: "reviewer", local: false }, label: "persona reviewer" },
        {
          level: { at: "persona", name: "reviewer", local: true },
          label: "local persona reviewer",
        },
      ],
    });
    expect(personasOf(answer)).toEqual(["reviewer"]);
    expect(personaRowsOf("reviewer", answer)).toEqual([
      expect.objectContaining({
        persona: "reviewer",
        skill: expect.objectContaining({ name: "review" }),
      }),
    ]);
  });
});

describe("a row's menu", () => {
  it("offers Use in this chat only while a chat is in front", () => {
    const chat = menuRowsOf(row(skill("deploy")), { workspace: "ops", chat: true });
    expect(chat.map((one) => one.title)).toEqual([
      "Use in this chat",
      "Turn off in ops",
      "Show in Explorer",
    ]);
    const none = menuRowsOf(row(skill("deploy")), { workspace: "ops", chat: false });
    expect(none.map((one) => one.act)).toEqual(["turn", "explorer"]);
  });

  it("turns a project skill on or off in the title bar's workspace, as the studio does", () => {
    expect(turnOf(skill("deploy"), "ops")).toMatchObject({ label: "Turn off in ops", on: false });
    expect(turnOf(skill("deploy", { on: false, turned_off: true }), "ops")).toMatchObject({
      label: "Turn on in ops",
      on: true,
    });
  });

  it("says why it cannot turn one where it cannot", () => {
    expect(turnOf(skill("secrets", { level: "built-in" }), "ops")).toEqual({
      reason: BUILT_IN_STAYS,
    });
    expect(turnOf(skill("deploy"), undefined)).toEqual({
      reason: "A project skill is turned on or off for one workspace. Pick one in the title bar.",
    });
    expect(
      turnOf(skill("deploy", { not_granted: "Its grant does not reach ops." }), "ops"),
    ).toEqual({ reason: "Its grant does not reach ops." });
    const mine = menuRowsOf(row(skill("mine", { level: "workspace" })), {
      workspace: "ops",
      chat: false,
    });
    expect(mine[0]).toMatchObject({
      act: "turn",
      available: false,
      reason: "Only a project skill is turned on or off for one workspace.",
    });
  });

  it("shows a skill's folder in Explorer at the project's root, and says why not elsewhere", () => {
    const at = (where: string | undefined, one = skill("deploy")) =>
      menuRowsOf(row(one), { workspace: where, chat: false }).find((r) => r.act === "explorer");
    expect(at(undefined)).toMatchObject({ available: true });
    expect(at("ops")).toMatchObject({ available: false, reason: expect.stringMatching(/root/) });
    expect(at(undefined, skill("secrets", { level: "built-in", folder: "/app/x" }))).toMatchObject({
      available: false,
      reason: "Its folder is not one of the project's files.",
    });
  });
});
