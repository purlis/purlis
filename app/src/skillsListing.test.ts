// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * What the Skills view lists (SK-55, #3266): its groups and their order, the one mark a row
 * carries and what it says on hover, and what each row's menu can do, from the Skill studio's
 * own answer.
 */
import { describe, expect, it } from "vitest";
import type { ChatSkill, SkillPinsRefused, StudioSkill } from "./bindings";
import { skill, studio } from "./skillsListing.testkit";
import {
  BUILT_IN_STAYS,
  cardReasonOf,
  explorerPlaceOf,
  filePlaceOf,
  fixOf,
  groupsOf,
  listedOf,
  markOf,
  menuRowsOf,
  narrowedTo,
  personaRowsOf,
  tipOf,
  turnOf,
  viewRowsOf,
  type ListedSkill,
  type MenuAt,
} from "./skillsListing";

const row = (one: StudioSkill, more: Partial<ListedSkill> = {}): ListedSkill => ({
  key: one.name,
  skill: one,
  ...more,
});

/** A skill of a chat's set, as Use skill… reads it. */
const given = (name: string, level: ChatSkill["level"] = "project"): ChatSkill => ({
  name,
  level,
  label: level,
  description: `What ${name} does.`,
  manual: false,
  takes_arguments: false,
  arguments: [],
  hint: null,
  hidden: null,
});

const names = (rows: readonly ListedSkill[]) => rows.map((one) => one.skill.name);

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

describe("the hidden-by-the-router mark", () => {
  const HIDDEN = {
    router: "built-in",
    why: "over-budget",
    said: "hidden by the built-in router: over this chat's skill budget, so it is in the index skill",
  } as const;
  const hiddenOn = (...titles: string[]): StudioSkill["hidden"] =>
    titles.map((title) => ({
      harness: title === "Codex" ? "codex" : "claude",
      title,
      hidden: HIDDEN,
    }));

  it("comes after left out and before lint", () => {
    const hidden = skill("big", { hidden: hiddenOn("Claude Code"), lint: ["x"] });
    expect(markOf(row(hidden))).toBe("hidden");
    expect(markOf(row({ ...hidden, on: false, left_out: "Its grant does not reach ops." }))).toBe(
      "left-out",
    );
    expect(markOf(row({ ...hidden, on: false, turned_off: true }))).toBe("off");
    expect(markOf(row(skill("plain", { lint: ["x"] })))).toBe("lint");
  });

  it("says which programs each of the core's lines is for, where they differ", () => {
    const other = { ...HIDDEN, said: "hidden by the built-in router: another reason" };
    const hidden = [
      ...hiddenOn("Claude Code"),
      { harness: "codex" as const, title: "Codex", hidden: other },
    ];
    expect(tipOf(row(skill("big", { hidden })))[1]).toBe(
      "Hidden by the built-in router: over this chat's skill budget, so it is in the index skill (on Claude Code). Hidden by the built-in router: another reason (on Codex).",
    );
  });

  it("says the core's line on hover, and the programs whose chats it is hidden from", () => {
    expect(tipOf(row(skill("big", { hidden: hiddenOn("Claude Code", "Codex") })))).toEqual([
      "What big does.",
      "Hidden by the built-in router: over this chat's skill budget, so it is in the index skill (on Claude Code, Codex).",
    ]);
  });

  it("is what the chat in front's set says, while the list is narrowed to it", () => {
    const rows = [row(skill("big", { hidden: hiddenOn("Codex") })), row(skill("small"))];
    const [big, small] = narrowedTo(rows, [given("big"), { ...given("small"), hidden: HIDDEN }], {
      persona: null,
      harness: "claude",
      repos: [],
    });
    expect(markOf(big)).toBeUndefined();
    expect(markOf(small)).toBe("hidden");
    expect(tipOf(small)).toEqual([
      "What small does.",
      "Hidden by the built-in router: over this chat's skill budget, so it is in the index skill.",
    ]);
  });
});

describe("the view's groups", () => {
  const all = [
    row(skill("secrets", { level: "built-in", folder: "/app/skills/secrets" })),
    row(skill("deploy")),
    row(skill("ops-only", { level: "workspace", folder: "workspaces/ops/skills/ops-only" })),
    row(skill("old", { on: false, turned_off: true, left_out: "ops turned it off." })),
    row(skill("pdf", { level: "installed", folder: "", lint: ["It pre-approves Bash."] }), {
      program: "Claude Code",
    }),
    row(skill("drafted", { accept: "skills", on: false })),
    row(skill("out", { on: false, left_out: "Its grant does not reach ops." })),
    row(skill("hid"), { hiddenInChat: { router: "built-in", why: "over-budget", said: "hidden" } }),
  ];

  it("are Needs you, In this chat with its built-in skills apart, and Left out (S7)", () => {
    const groups = groupsOf(all);
    expect(names(groups.needs)).toEqual(["pdf", "drafted"]);
    expect(names(groups.inChat)).toEqual(["deploy", "ops-only"]);
    expect(names(groups.builtIns)).toEqual(["secrets"]);
    expect(names(groups.leftOut)).toEqual(["old", "out", "hid"]);
  });

  it("say why a card is as it is in place of its description, in its tone", () => {
    expect(cardReasonOf(all[1])).toBeUndefined();
    expect(cardReasonOf(all[3])).toEqual({ says: "ops turned it off.", tone: "quiet" });
    expect(cardReasonOf(all[4])).toEqual({ says: "It pre-approves Bash.", tone: "warning" });
    expect(cardReasonOf(all[5])).toEqual({
      says: "A change to it waits for your review.",
      tone: "warning",
    });
    expect(cardReasonOf(all[7])).toEqual({ says: "Hidden.", tone: "quiet" });
  });

  it("offer one fix where a card has one", () => {
    expect(fixOf(all[3], "ops")).toEqual({ act: "turn", label: "Turn on in ops" });
    expect(fixOf(all[3], undefined)).toBeUndefined();
    expect(fixOf(all[5], "ops")).toEqual({ act: "review", label: "Review the change" });
    expect(fixOf(all[7], "ops")).toEqual({ act: "routing", label: "Why the router hid it" });
    expect(fixOf(all[6], "ops")).toBeUndefined();
    expect(fixOf(all[1], "ops")).toBeUndefined();
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
          skills: [skill("pdf", { level: "installed", folder: "" }), repo],
          repo_left_out: [],
          own: null,
        },
        {
          harness: "codex",
          title: "Codex",
          skills: [repo],
          repo_left_out: [],
          own: null,
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

  it("list a persona's own once, as the persona's, where the studio was read for that persona (#3462)", () => {
    const answer = studio([skill("deploy"), skill("review", { level: "persona" })]);
    const personas = [{ persona: "reviewer", skills: [skill("review", { level: "persona" })] }];
    expect(viewRowsOf(answer, personas).map((one) => [one.skill.name, one.persona])).toEqual([
      ["deploy", undefined],
      ["review", "reviewer"],
    ]);
    // Where every persona's own could not be read, the studio's are listed all the same.
    expect(viewRowsOf(answer, undefined).map((one) => one.skill.name)).toEqual([
      "deploy",
      "review",
    ]);
  });
});

describe("the rows narrowed to one chat's real set", () => {
  const rows = [
    row(skill("deploy")),
    row(skill("old", { on: true })),
    row(skill("pdf", { level: "installed", folder: "", lint: ["It asks to pre-approve Bash."] }), {
      program: "Claude Code",
      harness: "claude",
    }),
    row(skill("plan", { level: "installed", folder: "" }), { program: "Codex", harness: "codex" }),
    row(skill("lint-rs", { level: "repo", folder: "", owner: { kind: "repo", name: "svc" } })),
    row(skill("fmt", { level: "repo", folder: "", owner: { kind: "repo", name: "web" } })),
    row(skill("review", { level: "persona" }), { persona: "reviewer" }),
    row(skill("triage", { level: "persona" }), { persona: "ops" }),
  ];

  // What `chat_skills` really holds: the plane's and the built-in skills, never a program's own
  // or a repo's (`skills::of_a_chat`).
  const CLAUDE = { persona: "reviewer", harness: "claude" as const, repos: ["svc"] };

  it("marks the plane's skills by the chat's set, and keeps its program's own and its clone's as the studio marks them", () => {
    const set = [given("deploy"), given("review", "persona")];
    const narrowed = narrowedTo(rows, set, CLAUDE);
    expect(narrowed.map((one) => [one.skill.name, one.inChat])).toEqual([
      ["deploy", true],
      ["old", false],
      ["pdf", undefined],
      ["lint-rs", undefined],
      ["review", true],
    ]);
    const groups = groupsOf(narrowed);
    expect(names(groups.needs)).toEqual(["pdf"]);
    expect(names(groups.inChat)).toEqual(["deploy", "lint-rs", "review"]);
    expect(names(groups.leftOut)).toEqual(["old"]);
    // The program's own skill keeps the studio's own mark, and a repo's can be used.
    expect(markOf(narrowed[2])).toBe("lint");
    expect(
      menuRowsOf(narrowed[3], { workspace: "ops", explorer: "ops", chat: true })[0],
    ).toMatchObject({
      act: "use",
      available: true,
    });
    expect(tipOf(narrowed[1])).toEqual(["What old does.", "The chat in front started without it."]);
  });

  it("lists a skill of the set the workspace's rows do not hold, as the set says it", () => {
    const narrowed = narrowedTo([], [given("extra", "workspace")], {
      persona: null,
      harness: "claude",
      repos: [],
    });
    expect(narrowed).toEqual([
      expect.objectContaining({
        inChat: true,
        skill: expect.objectContaining({ name: "extra", level: "workspace", on: true }),
      }),
    ]);
  });

  it("keeps a built-in skill of the set for a chat at the project's root", () => {
    const set = [given("secrets", "built-in")];
    const narrowed = narrowedTo([row(skill("secrets", { level: "built-in" }))], set, {
      persona: null,
      harness: undefined,
      repos: [],
    });
    expect(names(groupsOf(narrowed).builtIns)).toEqual(["secrets"]);
  });

  it("lists the repo skills of the chat's own clone alone, and none for a chat in no clone (#3462)", () => {
    const repoNames = (repos: string[]) =>
      narrowedTo(rows, [], { ...CLAUDE, repos })
        .filter((one) => one.skill.level === "repo")
        .map((one) => one.skill.name);
    expect(repoNames(["svc"])).toEqual(["lint-rs"]);
    expect(repoNames(["web"])).toEqual(["fmt"]);
    expect(repoNames([])).toEqual([]);
  });
});

describe("a row's menu", () => {
  it("offers Use, the switch, the pin, Explorer, the file and the studio, in that order (S7)", () => {
    const chat = menuRowsOf(row(skill("deploy")), {
      workspace: "ops",
      explorer: "ops",
      chat: true,
    });
    expect(chat.map((one) => one.title)).toEqual([
      "Use in this chat",
      "Turn off in ops",
      "Pin to this workspace",
      "Show in the Explorer view",
      "Open the file",
      "Open in Skill studio",
    ]);
    const none = menuRowsOf(row(skill("deploy")), {
      workspace: "ops",
      explorer: "ops",
      chat: false,
    });
    expect(none[0]).toEqual({
      act: "use",
      title: "Use in this chat",
      available: false,
      reason: "No chat is in front to use it in.",
    });
  });

  it("opens a skill's own file, the project's or its clone's, and says why not a built-in's", () => {
    expect(filePlaceOf(skill("deploy"))).toEqual({
      workspace: "",
      repo: "",
      path: "skills/deploy/SKILL.md",
    });
    expect(
      filePlaceOf(skill("fmt", { level: "repo", folder: "workspaces/ops/web/.claude/skills/fmt" })),
    ).toEqual({ workspace: "ops", repo: "web", path: ".claude/skills/fmt/SKILL.md" });
    expect(
      filePlaceOf(skill("secrets", { level: "built-in", folder: "/app/skills/secrets" })),
    ).toEqual({
      reason: "It ships with purlis: Make my own copy in the Skill studio to edit one.",
    });
    expect(filePlaceOf(skill("pdf", { level: "installed", folder: "" }))).toEqual({
      reason: "Its folder is not one of the project's files.",
    });
  });

  it("cannot use in the chat a skill it does not get, and says why", () => {
    for (const one of [
      row(skill("out", { on: false, left_out: "Its grant does not reach ops." })),
      row(skill("deploy"), { inChat: false }),
    ])
      expect(menuRowsOf(one, { workspace: "ops", explorer: "ops", chat: true })[0]).toEqual({
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
      explorer: "ops",
      chat: false,
    });
    expect(mine[1]).toMatchObject({
      act: "turn",
      available: false,
      reason: "Only a project skill is turned on or off for one workspace.",
    });
  });

  it("pins a skill to the title bar's workspace, or unpins it while that workspace pins it", () => {
    const pin = (one: StudioSkill, workspace: string | undefined = "ops") =>
      menuRowsOf(row(one), { workspace, explorer: workspace, chat: true }).find(
        (r) => r.act === "pin",
      );
    expect(
      menuRowsOf(row(skill("deploy")), { workspace: "ops", explorer: "ops", chat: true }).map(
        (r) => r.act,
      ),
    ).toEqual(["use", "turn", "pin", "explorer", "file", "studio"]);
    expect(pin(skill("deploy"))).toEqual({
      act: "pin",
      title: "Pin to this workspace",
      available: true,
      reason: "",
    });
    // Another workspace's or the persona's pin is not this workspace's to undo here.
    expect(
      pin(
        skill("deploy", {
          pinned_by: [
            { at: "persona", name: "reviewer" },
            { at: "workspace", name: "web" },
          ],
        }),
      ),
    ).toMatchObject({ title: "Pin to this workspace", available: true });
    expect(
      pin(skill("deploy", { pinned: true, pinned_by: [{ at: "workspace", name: "ops" }] })),
    ).toEqual({
      act: "pin",
      title: "Unpin from this workspace",
      available: true,
      reason: "",
    });
  });

  it("says a pin this workspace's chats read none of is not used, and why (#3432)", () => {
    const refused = (name: string, bad: string | null = null): SkillPinsRefused => ({
      by: { at: "workspace", name },
      why: bad === null ? "the list holds more than 64 pins" : `"${bad}" is not a skill's name`,
      bad,
      file: `workspaces/${name}/skills.pins`,
    });
    const pinRow = (one: StudioSkill, pinsRefused: SkillPinsRefused | null) =>
      menuRowsOf(row(one), { workspace: "ops", explorer: "ops", chat: false, pinsRefused }).find(
        (r) => r.act === "pin",
      );
    const pinned = skill("deploy", { pinned: true, pinned_by: [{ at: "workspace", name: "ops" }] });
    expect(pinRow(pinned, refused("ops"))).toEqual({
      act: "pin",
      title: "Unpin from this workspace",
      available: true,
      reason: "",
      note: "Pinned, not used: the list holds more than 64 pins.",
    });
    // A pin not yet written would be read no more than the rest: the list is put right first.
    expect(pinRow(skill("deploy"), refused("ops", "Not A Name"))).toEqual({
      act: "pin",
      title: "Pin to this workspace",
      available: false,
      reason:
        'Workspace ops\'s pins aren\'t used: "Not A Name" is not a skill\'s name. Remove "Not A Name" in the Skill studio, or edit workspaces/ops/skills.pins.',
    });
    expect(pinRow(skill("deploy"), refused("ops"))).toMatchObject({
      available: false,
      reason:
        "Workspace ops's pins aren't used: the list holds more than 64 pins. Unpin some, or edit workspaces/ops/skills.pins.",
    });
    // Another workspace's refusal says nothing here; nor does none.
    expect(pinRow(pinned, refused("web"))).not.toHaveProperty("note");
    expect(pinRow(pinned, null)).not.toHaveProperty("note");
  });

  it("says why it cannot pin one where it cannot", () => {
    const pin = (one: StudioSkill, workspace: string | undefined = "ops") =>
      menuRowsOf(row(one), { workspace, explorer: workspace, chat: false }).find(
        (r) => r.act === "pin",
      );
    const atTheRoot = menuRowsOf(row(skill("deploy")), {
      workspace: undefined,
      explorer: undefined,
      chat: false,
    });
    expect(atTheRoot.find((r) => r.act === "pin")).toMatchObject({
      title: "Pin to this workspace",
      available: false,
      reason: "A skill is pinned for one workspace's chats. Pick one in the title bar.",
    });
    expect(pin(skill("secrets", { level: "built-in" }))).toMatchObject({
      available: false,
      reason: "It ships with purlis, and is always listed.",
    });
    for (const level of ["installed", "repo"] as const)
      expect(pin(skill("pdf", { level, folder: "" }))).toMatchObject({
        available: false,
        reason: "Its program lists it, not purlis's router.",
      });
    expect(
      pin(skill("out", { on: false, left_out: "Its grant does not reach ops." })),
    ).toMatchObject({
      available: false,
      reason: "This workspace's chats do not get it.",
    });
    // A pin still written on a skill the chats no longer get can still be undone.
    expect(
      pin(skill("out", { on: false, pinned_by: [{ at: "workspace", name: "ops" }] })),
    ).toMatchObject({ title: "Unpin from this workspace", available: true });
    // A persona's own skill is pinned for the persona, which the studio filtered to it does.
    expect(
      menuRowsOf(row(skill("review", { level: "persona" }), { persona: "reviewer" }), {
        workspace: "ops",
        explorer: "ops",
        chat: false,
      }).find((r) => r.act === "pin"),
    ).toMatchObject({
      available: false,
      reason: "reviewer's own skill is pinned for it, in its Skill studio.",
    });
  });

  it("shows a skill's folder in Explorer at the project's root, and says why not elsewhere", () => {
    const at = (where: string | undefined, one = skill("deploy")) =>
      menuRowsOf(row(one), { workspace: where, explorer: where, chat: false }).find(
        (r) => r.act === "explorer",
      );
    expect(at(undefined)).toMatchObject({ available: true });
    expect(at("ops")).toMatchObject({ available: false, reason: expect.stringMatching(/root/) });
    expect(at(undefined, skill("secrets", { level: "built-in", folder: "/app/x" }))).toMatchObject({
      available: false,
      reason: "Its folder is not one of the project's files.",
    });
  });

  it("shows a repo skill in its clone, in the workspace Explorer shows (#3308)", () => {
    const lint = skill("lint-rs", {
      level: "repo",
      folder: "workspaces/ops/api/.claude/skills/lint-rs",
      owner: { kind: "repo", name: "api" },
    });
    expect(explorerPlaceOf(lint, "ops")).toEqual({ repo: "api", folder: ".claude/skills/lint-rs" });
    const at = (where: MenuAt) => menuRowsOf(row(lint), where).find((r) => r.act === "explorer");
    expect(at({ workspace: "ops", explorer: "ops", chat: false })).toMatchObject({
      available: true,
    });
    // Narrowed to a chat in ops while the title bar, and so Explorer, is on web.
    expect(at({ workspace: "ops", chat: true, explorer: "web" })).toMatchObject({
      available: false,
      reason: expect.stringMatching(/Pick it in the title bar/),
    });
    expect(at({ workspace: undefined, explorer: undefined, chat: false })).toMatchObject({
      available: false,
    });
    // A project skill at a workspace is still the root's to show.
    expect(explorerPlaceOf(skill("deploy"), undefined)).toEqual({
      repo: "",
      folder: "skills/deploy",
    });
    expect(explorerPlaceOf(skill("deploy"), "ops")).toMatchObject({ reason: expect.any(String) });
    // A program's own skill is nowhere in the project.
    expect(explorerPlaceOf(skill("pdf", { level: "installed", folder: "" }), "ops")).toMatchObject({
      reason: "Its folder is not one of the project's files.",
    });
  });
});

describe("a repo skill in a branch folder (#4326)", () => {
  const onTheBranch = skill("fmt", {
    level: "repo",
    folder: "workspaces/ops/.worktrees/web/fix/.claude/skills/fmt",
    owner: { kind: "repo", name: "web" },
  });

  it("opens the branch folder's copy of the file", () => {
    expect(filePlaceOf(onTheBranch)).toEqual({
      workspace: "ops",
      repo: "web",
      piece: "fix",
      path: ".claude/skills/fmt/SKILL.md",
    });
  });

  it("shows its folder in the branch folder, in that workspace only", () => {
    expect(explorerPlaceOf(onTheBranch, "ops")).toEqual({
      repo: "web",
      piece: "fix",
      folder: ".claude/skills/fmt",
    });
    expect(explorerPlaceOf(onTheBranch, "other")).toHaveProperty("reason");
    expect(explorerPlaceOf(onTheBranch, undefined)).toHaveProperty("reason");
  });

  it("offers no place for a branch folder's path that stops short of a skill", () => {
    const short = skill("fmt", { level: "repo", folder: "workspaces/ops/.worktrees/web/fix" });
    expect(filePlaceOf(short)).toHaveProperty("reason");
    expect(explorerPlaceOf(short, "ops")).toHaveProperty("reason");
  });
});
