// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * What the Skills view lists (SK-55, #3266): its groups and their order, the one mark a row
 * carries and what it says on hover, and what each row's menu can do, from the Skill studio's
 * own answer.
 */
import { describe, expect, it } from "vitest";
import type { ChatSkill, StudioSkill } from "./bindings";
import { skill, studio } from "./skillsListing.testkit";
import {
  BUILT_IN_STAYS,
  groupsOf,
  listedOf,
  markOf,
  menuRowsOf,
  narrowedTo,
  personaRowsOf,
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
    expect(markOf(row(skill("plain")))).toBeUndefined();
    expect(markOf(row(skill("linted", { lint: ["It widens the chat."] })))).toBe("lint");
    expect(
      markOf(row(skill("out", { on: false, left_out: "Its grant does not reach ops." }))),
    ).toBe("left-out");
    expect(markOf(row(skill("off", { on: false, turned_off: true, lint: ["x"] })))).toBe("off");
    expect(
      markOf(
        row(skill("drafted", { on: false, turned_off: true, accept: "workspaces/ops/skills" })),
      ),
    ).toBe("needs-review");
    // The chat in front's set decides, where it was read.
    expect(markOf(row(skill("plain"), { inChat: false }))).toBe("left-out");
    expect(markOf(row(skill("persona-only", { on: false }), { inChat: true }))).toBeUndefined();
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

  it("mark each persona's own as its, from one read of every persona", () => {
    expect(
      personaRowsOf([
        { persona: "reviewer", skills: [skill("review", { level: "persona" })] },
        { persona: "ops", skills: [skill("triage", { level: "persona" })] },
      ]).map((one) => [one.persona, one.skill.name]),
    ).toEqual([
      ["reviewer", "review"],
      ["ops", "triage"],
    ]);
  });
});

describe("the rows narrowed to one chat's real set", () => {
  const given = (name: string, level: ChatSkill["level"] = "project"): ChatSkill => ({
    name,
    level,
    label: level,
    description: `What ${name} does.`,
    manual: false,
    takes_arguments: false,
    arguments: [],
    hint: null,
  });
  const rows = [
    row(skill("deploy")),
    row(skill("old", { on: true })),
    row(skill("pdf", { level: "installed", folder: "", lint: ["It asks to pre-approve Bash."] }), {
      program: "Claude Code",
      harness: "claude",
    }),
    row(skill("plan", { level: "installed", folder: "" }), { program: "Codex", harness: "codex" }),
    row(skill("lint-rs", { level: "repo", folder: "" })),
    row(skill("review", { level: "persona" }), { persona: "reviewer" }),
    row(skill("triage", { level: "persona" }), { persona: "ops" }),
  ];

  // What `chat_skills` really holds: the plane's and the built-in skills, never a program's own
  // or a repo's (`skills::of_a_chat`).
  const CLAUDE = { persona: "reviewer", harness: "claude" as const };

  it("marks the plane's skills by the chat's set, and keeps its program's own and each repo's as the studio marks them", () => {
    const set = [given("deploy"), given("review", "persona")];
    const narrowed = narrowedTo(rows, set, CLAUDE);
    expect(narrowed.map((one) => [one.skill.name, one.inChat])).toEqual([
      ["deploy", true],
      ["old", false],
      ["pdf", undefined],
      ["lint-rs", undefined],
      ["review", true],
    ]);
    const groups = groupsOf(narrowed, { root: false, chat: true });
    expect(groups.map((one) => [one.title, one.rows.map((r) => r.skill.name)])).toEqual([
      ["Project", ["deploy"]],
      ["Personas", ["review"]],
      ["Repo", ["lint-rs"]],
      ["Claude Code", ["pdf"]],
      ["Not in this chat", ["old"]],
    ]);
    // The program's own skill keeps the studio's own mark, and a repo's can be used.
    expect(markOf(narrowed[2])).toBe("lint");
    expect(menuRowsOf(narrowed[3], { workspace: "ops", chat: true })[0]).toMatchObject({
      act: "use",
      available: true,
    });
    expect(tipOf(narrowed[1])).toEqual(["What old does.", "The chat in front started without it."]);
  });

  it("lists a skill of the set the workspace's rows do not hold, as the set says it", () => {
    const narrowed = narrowedTo([], [given("extra", "workspace")], {
      persona: null,
      harness: "claude",
    });
    expect(narrowed).toEqual([
      expect.objectContaining({
        inChat: true,
        skill: expect.objectContaining({ name: "extra", level: "workspace", on: true }),
      }),
    ]);
  });

  it("keeps every group at the project's root, for a chat at the root", () => {
    const set = [given("secrets", "built-in")];
    const narrowed = narrowedTo([row(skill("secrets", { level: "built-in" }))], set, {
      persona: null,
      harness: undefined,
    });
    expect(groupsOf(narrowed, { root: true, chat: true }).map((one) => one.title)).toEqual([
      "Built-in",
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

  it("cannot use in the chat a skill it does not get, and says why", () => {
    for (const one of [
      row(skill("out", { on: false, left_out: "Its grant does not reach ops." })),
      row(skill("deploy"), { inChat: false }),
    ])
      expect(menuRowsOf(one, { workspace: "ops", chat: true })[0]).toEqual({
        act: "use",
        title: "Use in this chat",
        available: false,
        reason: "The chat in front does not get it.",
      });
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
