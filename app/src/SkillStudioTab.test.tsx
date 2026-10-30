// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { forgetStudio, openSkill, renderStudio } from "./SkillStudioTab.testkit";
import {
  AT_THE_ROOT,
  SKILL_STUDIO_TITLE,
  asPersona,
  inWorkspace,
  skillStudioAs,
  skillStudioAt,
  skillStudioTitle,
  skillStudioView,
  studioOpening,
} from "./skillStudio";
import { catalogue, SKILL_STUDIO_SHOW, type Now } from "./actions";
import { noTabs } from "./tabs";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetStudio();
});

function answering(studio: unknown) {
  mockIPC((cmd) => {
    if (cmd === "skill_studio") return studio;
    if (cmd === "skills_deleted") return [];
    throw new Error(`unexpected ${cmd}`);
  });
}

describe("the Skill studio", () => {
  it("lists each skill with its level, and why one is left out", async () => {
    answering({
      on: true,
      workspace: null,
      first: { starts: false, unmoved: null },
      workspaces: [],
      levels: [],
      harnesses: [],
      clashes: [],
      skills: [
        {
          name: "deploy",
          level: "project",
          label: "project",
          local: false,
          grant: "root",
          granted: { to: "root" },
          turned_off: false,
          on: true,
          left_out: null,
          description: "Deploys the site.",
          not_granted: null,
          written: "deploy",
          named: false,
          unchecked: null,
          folder: "skills/deploy",
          compat: [],
          lint: [],
          hidden: [],
          pinned: false,
          pinned_by: [],
        },
        {
          name: "secrets",
          level: "built-in",
          label: "built-in",
          local: false,
          grant: null,
          granted: null,
          turned_off: false,
          on: true,
          left_out: null,
          description: "Keeps secrets.",
          not_granted: null,
          written: "secrets",
          named: false,
          unchecked: null,
          folder: "/app/plugin/skills/secrets",
          compat: [],
          lint: [],
          hidden: [],
          pinned: false,
          pinned_by: [],
        },
        {
          name: "review",
          level: "project",
          label: "project",
          local: false,
          grant: "root",
          granted: { to: "root" },
          turned_off: false,
          on: false,
          left_out: "a built-in skill has this name, and a built-in skill is never replaced",
          description: "",
          not_granted: null,
          written: "review",
          named: false,
          unchecked: null,
          folder: "skills/review",
          compat: [],
          lint: [],
          hidden: [],
          pinned: false,
          pinned_by: [],
        },
      ],
    });

    renderStudio();

    const deploy = await screen.findByTestId("skill-deploy");
    expect(within(deploy).getByText("project")).toBeInTheDocument();
    // One line of what it does under its name, and no mark for a skill a chat simply gets.
    expect(within(deploy).getByText("Deploys the site.")).toBeInTheDocument();
    expect(within(deploy).queryByText("Left out")).toBeNull();
    expect(within(screen.getByTestId("skill-review")).getByText("Left out")).toBeInTheDocument();
    expect(
      within(await openSkill("skill-deploy")).getByTestId("skill-in-a-chat"),
    ).toHaveTextContent("On: a chat at the project's root gets it.");
    expect(within(screen.getByTestId("skill-secrets")).getByText("built-in")).toBeInTheDocument();
    expect(
      within(screen.getByTestId("skill-review")).getByText(
        "Left out: a built-in skill has this name, and a built-in skill is never replaced",
      ),
    ).toBeInTheDocument();
  });

  it("says why the router hid a skill, and which are pinned (SK-15)", async () => {
    const row = (name: string, more: object) => ({
      name,
      level: "project",
      label: "project",
      local: false,
      grant: "root",
      granted: { to: "root" },
      turned_off: false,
      on: true,
      left_out: null,
      description: `Does ${name}.`,
      not_granted: null,
      written: name,
      named: false,
      unchecked: null,
      folder: `skills/${name}`,
      compat: [],
      lint: [],
      hidden: [],
      pinned: false,
      pinned_by: [],
      ...more,
    });
    answering({
      on: true,
      workspace: null,
      first: { starts: false, unmoved: null },
      workspaces: [],
      levels: [],
      harnesses: [],
      clashes: [],
      skills: [
        row("deploy", {
          hidden: [
            {
              harness: "codex",
              title: "Codex",
              hidden: {
                router: "built-in",
                why: "over-budget",
                said: "hidden by the built-in router: over this chat's skill budget, so it is in the index skill",
              },
            },
          ],
        }),
        row("review", { pinned: true }),
      ],
    });

    renderStudio();

    const user = userEvent.setup();
    // The row's one mark says the router hid it; its page says how, on which program.
    expect(
      within(await screen.findByTestId("skill-deploy")).getByText("Hidden by the router"),
    ).toBeInTheDocument();
    expect(within(screen.getByTestId("skill-review")).getByText("Pinned")).toBeInTheDocument();
    const deploy = await openSkill("skill-deploy", user);
    expect(within(deploy).getByTestId("skill-in-a-chat")).toHaveTextContent(
      "On, hidden by the built-in router: over this chat's skill budget, so it is in the index skill (on Codex)",
    );
    // Its banner says so too, and the chat can still load it.
    expect(within(deploy).getByTestId("skill-banner-hidden")).toHaveTextContent(
      "Hidden by the router on Codex",
    );
    expect(
      within(await openSkill("skill-review", user)).getByTestId("skill-in-a-chat"),
    ).toHaveTextContent(/^On, pinned/);
  });

  it("shows a workspace's chats, its own copy over the project's, and both copies", async () => {
    const asked: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "skills_deleted") return [];
      if (cmd !== "skill_studio") throw new Error(`unexpected ${cmd}`);
      asked.push(args);
      return {
        on: true,
        workspace: "alpha",
        first: { starts: false, unmoved: null },
        workspaces: ["alpha", "beta"],
        levels: [],
        harnesses: [],
        clashes: [],
        skills: [
          {
            name: "lint",
            level: "workspace",
            label: "workspace",
            local: false,
            grant: null,
            granted: null,
            turned_off: false,
            on: true,
            left_out: null,
            description: "Lints here.",
            not_granted: null,
            written: "lint",
            named: false,
            unchecked: null,
            folder: "workspaces/alpha/skills/lint",
            compat: [],
            lint: [],
            hidden: [],
            pinned: false,
            pinned_by: [],
          },
          {
            name: "lint",
            level: "project",
            label: "project",
            local: false,
            grant: "all",
            granted: { to: "all" },
            turned_off: false,
            on: false,
            left_out: "hidden by the workspace skill of this name",
            description: "",
            not_granted: null,
            written: "lint",
            named: false,
            unchecked: null,
            folder: "skills/lint",
            compat: [],
            lint: [],
            hidden: [],
            pinned: false,
            pinned_by: [],
          },
        ],
      };
    });

    renderStudio(inWorkspace("alpha"));

    const mine = await screen.findByTestId("skill-lint");
    expect(within(mine).getByText("workspace")).toBeInTheDocument();
    expect(
      within(screen.getByTestId("skill-lint-hidden-1")).getByText(
        "Left out: hidden by the workspace skill of this name",
      ),
    ).toBeInTheDocument();
    const hidden = await openSkill("skill-lint-hidden-1");
    expect(within(hidden).getByText("Every workspace")).toBeInTheDocument();
    expect(
      within(hidden).getByText("Left out: hidden by the workspace skill of this name"),
    ).toBeInTheDocument();
    expect(screen.getByText(/Every skill a chat in alpha can get/)).toBeInTheDocument();
    expect(asked[0]).toMatchObject({ plane: "p", workspace: "alpha" });
  });

  it("shares a local skill and reads the project again", async () => {
    let shared = false;
    const calls: string[] = [];
    mockIPC((cmd, args) => {
      calls.push(cmd);
      if (cmd === "skill_share") {
        expect(args).toMatchObject({
          plane: "p",
          whose: { at: "project" },
          name: "scratch",
        });
        shared = true;
        return null;
      }
      if (cmd === "skills_deleted") return [];
      if (cmd !== "skill_studio") throw new Error(`unexpected ${cmd}`);
      return {
        on: true,
        workspace: null,
        first: { starts: false, unmoved: null },
        workspaces: [],
        levels: [],
        harnesses: [],
        clashes: [],
        skills: [
          {
            name: "scratch",
            level: "project",
            label: shared ? "project" : "local project",
            local: !shared,
            grant: "root",
            granted: { to: "root" },
            turned_off: false,
            on: true,
            left_out: null,
            description: "Tries it.",
            not_granted: null,
            written: "scratch",
            named: false,
            unchecked: null,
            folder: shared ? "skills/scratch" : "skills.local/scratch",
            compat: [],
            lint: [],
            hidden: [],
            pinned: false,
            pinned_by: [],
          },
        ],
      };
    });

    renderStudio();
    const row = await screen.findByTestId("skill-scratch");
    expect(within(row).getByText(/local project/)).toBeInTheDocument();
    within(await openSkill("skill-scratch"))
      .getByRole("button", { name: "Share" })
      .click();

    await waitFor(() =>
      expect(within(screen.getByTestId("skill-scratch")).getByText("project")).toBeInTheDocument(),
    );
    expect(calls.filter((cmd) => cmd === "skill_studio")).toHaveLength(2);
  });

  /** A studio with no skills, whose core answers the deletes cut short with `ended`, and
   *  records every command asked. */
  function cutShort(ended: { settled: string[]; unsettled: string[] }) {
    const calls: string[] = [];
    mockIPC((cmd) => {
      calls.push(cmd);
      if (cmd === "skill_studio")
        return {
          on: true,
          workspace: null,
          persona: null,
          first: { starts: false, unmoved: null },
          workspaces: [],
          levels: [],
          skills: [],
          harnesses: [],
          clashes: [],
        };
      if (cmd === "skill_deletes_cut_short") return ended;
      return [];
    });
    return calls;
  }

  it("says a skill delete a crash cut short, finished as it opens, and reads its skills again", async () => {
    // SK-35 (#3013): the core finishes or undoes it, and the studio says which in one line.
    const line = "release's delete was cut short; it is finished now, and Restore brings it back";
    const calls = cutShort({ settled: [line], unsettled: [] });
    renderStudio();

    const notice = await screen.findByRole("status", { name: /Skill delete cut short/ });
    expect(notice).toHaveTextContent(line);
    // The notice family's box says its tone (#2912).
    expect(notice.closest('[data-scope="notice"][data-part="root"]')).toHaveAttribute(
      "data-variant",
      "news",
    );
    await waitFor(() =>
      expect(calls.filter((cmd) => cmd === "skill_studio").length).toBeGreaterThanOrEqual(2),
    );
    expect(calls.filter((cmd) => cmd === "skill_deletes_cut_short")).toHaveLength(1);
  });

  it("shows a skill delete it could neither finish nor put back as trouble", async () => {
    const failed =
      "release's delete was cut short and could not be finished or put back: the project's " +
      "last commit still has it, and purlis tries again next time";
    const finished =
      "deploy's delete was cut short; it is finished now, and Restore brings it back";
    cutShort({ settled: [finished], unsettled: [failed] });
    renderStudio();

    const notice = await screen.findByRole("status", {
      name: /A skill delete could not be finished/,
    });
    expect(notice.closest('[data-scope="notice"][data-part="root"]')).toHaveAttribute(
      "data-variant",
      "trouble",
    );
    expect(notice).toHaveTextContent(`${failed} ${finished}`);
  });

  it("says it is not on while its flag is off", async () => {
    answering({
      on: false,
      workspace: null,
      first: { starts: false, unmoved: null },
      workspaces: [],
      skills: [],
      harnesses: [],
      clashes: [],
    });
    renderStudio();
    expect(await screen.findByTestId("skill-studio-off")).toBeInTheDocument();
  });

  it("shows a persona's chats, with the persona's own skill and one it names", async () => {
    const asked: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd !== "skill_studio") throw new Error(`unexpected ${cmd}`);
      asked.push(args);
      return {
        on: true,
        workspace: null,
        persona: "ops",
        first: { starts: false, unmoved: null },
        workspaces: ["alpha"],
        levels: [],
        harnesses: [],
        clashes: [],
        skills: [
          {
            name: "triage",
            level: "persona",
            label: "persona",
            local: false,
            grant: null,
            on: true,
            left_out: null,
            description: "Triages.",
            granted: null,
            turned_off: false,
            not_granted: null,
            folder: "personas/ops/skills/triage",
            compat: [],
            lint: [],
            hidden: [],
            pinned: false,
            pinned_by: [],
            written: "triage",
            named: false,
            unchecked: null,
          },
          {
            name: "deploy",
            level: "project",
            label: "project",
            local: false,
            grant: "root",
            on: true,
            left_out: null,
            description: "Deploys.",
            granted: null,
            turned_off: false,
            not_granted: null,
            folder: "skills/deploy",
            compat: [],
            lint: [],
            hidden: [],
            pinned: false,
            pinned_by: [],
            written: "deploy",
            named: true,
            unchecked: null,
          },
          {
            name: "superpowers:tdd",
            level: "installed",
            label: "installed",
            local: false,
            grant: null,
            on: true,
            left_out: null,
            description: "",
            granted: null,
            turned_off: false,
            not_granted: null,
            folder: "",
            compat: [],
            lint: [],
            hidden: [],
            pinned: false,
            pinned_by: [],
            written: "superpowers:tdd",
            named: true,
            unchecked: "purlis can't check whether this machine has it",
          },
        ],
      };
    });

    renderStudio(asPersona("ops"));

    const user = userEvent.setup();
    expect(
      within(await screen.findByTestId("skill-triage")).getByText("persona"),
    ).toBeInTheDocument();
    const triage = await openSkill("skill-triage", user);
    expect(within(triage).queryByRole("button", { name: /Take .* out of/ })).toBeNull();
    expect(
      within(await openSkill("skill-deploy", user)).getByRole("button", {
        name: "Take deploy out of ops",
      }),
    ).toBeInTheDocument();
    expect(
      within(await openSkill("skill-superpowers:tdd", user)).getByText(/purlis can't check/),
    ).toBeInTheDocument();
    expect(screen.getByRole("form", { name: "Add a skill to ops" })).toBeInTheDocument();
    expect(screen.getByText(/Every skill a chat as ops can get/)).toBeInTheDocument();
    expect(asked[0]).toMatchObject({ plane: "p", workspace: null, persona: "ops" });
  });

  it("opens one tab per project, at the page and scope each way in names (#3681)", () => {
    const free = () => false;
    for (const scope of [AT_THE_ROOT, inWorkspace("ops"), asPersona("ops")])
      expect(studioOpening(skillStudioView(scope), free)).toEqual({
        tab: { from: null, view: "skill-studio", key: "" },
        place: { page: "library", scope },
      });
    expect(studioOpening(skillStudioAt(inWorkspace("ops"), "sources"), free).place).toEqual({
      page: "sources",
      scope: inWorkspace("ops"),
    });
    // A tab put back from an older launch, keyed by its scope, is the one tab on its Library.
    expect(
      studioOpening({ from: null, view: "skill-studio", key: "persona/ops" }, () => true),
    ).toEqual({
      tab: { from: null, view: "skill-studio", key: "" },
      place: { page: "library", scope: asPersona("ops") },
    });
    // ⌘-click: a tab of its own, beside the ones open.
    const open = new Set(["", "#2"]);
    expect(
      studioOpening(
        skillStudioAt(AT_THE_ROOT, "library", { newTab: true }),
        (key) => !open.has(key),
      ).tab.key,
    ).toBe("#3");
    // A studio tab itself opens as itself.
    expect(studioOpening({ from: null, view: "skill-studio", key: "#2" }, free)).toEqual({
      tab: { from: null, view: "skill-studio", key: "#2" },
    });
    expect(skillStudioTitle(asPersona("ops"))).toBe("Skill studio · ops");
  });

  it("offers each persona's studio in the palette only while its flag is on", () => {
    const now = (skillStudio: boolean): Now => ({
      tabs: noTabs(),
      workspaces: [],
      needsYou: [],
      nameOf: (session) => String(session),
      plane: "p",
      personas: ["ops"],
      skillStudio,
    });
    const row = (on: boolean) =>
      catalogue(now(on)).find((offer) => offer.id === skillStudioAs("ops"));
    expect(row(true)?.does).toEqual({
      verb: "openView",
      view: skillStudioView(asPersona("ops")),
      title: SKILL_STUDIO_TITLE,
    });
    expect(row(false)).toBeUndefined();
  });

  it("is offered in the palette only while its flag is on", () => {
    const now = (skillStudio: boolean): Now => ({
      tabs: noTabs(),
      workspaces: [],
      needsYou: [],
      nameOf: (session) => String(session),
      plane: "p",
      skillStudio,
    });
    const ids = (on: boolean) => catalogue(now(on)).map((offer) => offer.id);
    expect(ids(true)).toContain(SKILL_STUDIO_SHOW);
    expect(ids(false)).not.toContain(SKILL_STUDIO_SHOW);
  });
});

const ROW = {
  local: false,
  grant: null,
  granted: null,
  turned_off: false,
  not_granted: null,
  on: true,
  left_out: null,
  owner: null,
  switch: null,
};

/** A studio with one project skill, `review`, and Claude Code's own skills and plugins. */
function withClaudeCode(offLangfuse: boolean) {
  return {
    on: true,
    workspace: null,
    first: { starts: false, unmoved: null },
    workspaces: [],
    levels: [],
    skills: [
      {
        ...ROW,
        name: "review",
        level: "project",
        label: "project",
        grant: "root",
        description: "Reviews.",
        folder: "skills/review",
        compat: [],
        lint: [],
        hidden: [],
        pinned: false,
        pinned_by: [],
      },
      {
        ...ROW,
        name: "secrets",
        level: "built-in",
        label: "built-in",
        description: "Keeps secrets.",
        folder: "/app/plugin/skills/secrets",
        compat: [],
        lint: [],
        hidden: [],
        pinned: false,
        pinned_by: [],
      },
    ],
    clashes: [
      {
        name: "review",
        level: "project",
        local: false,
        harness: "Claude Code",
        outside: "repo",
        owner: { kind: "repo", name: "api" },
        outcome: "both-load",
        said: "the repo api has a skill of this name too, and a chat there gets both",
      },
    ],
    harnesses: [
      {
        harness: "claude",
        title: "Claude Code",
        repo_left_out: [],
        skills: [
          {
            ...ROW,
            name: "langfuse",
            level: "installed",
            label: "installed",
            on: !offLangfuse,
            left_out: offLangfuse ? "turned off for this project's chats" : null,
            description: "Reads traces.",
            folder: "/home/.claude/skills/langfuse",
            compat: [],
            lint: [],
            hidden: [],
            pinned: false,
            pinned_by: [],
            owner: { kind: "you" },
            switch: { kind: "alone" },
          },
          {
            ...ROW,
            name: "figma:frame",
            level: "installed",
            label: "installed",
            description: "Frames.",
            folder: "/cache/figma/skills/frame",
            compat: [],
            lint: [],
            hidden: [],
            pinned: false,
            pinned_by: [],
            owner: { kind: "plugin", name: "figma" },
            switch: {
              kind: "not",
              why: "it comes with the plugin figma, which Claude Code turns on or off itself, so its skills are shown read-only",
            },
          },
          {
            ...ROW,
            name: "review",
            level: "repo",
            label: "repo",
            description: "The repo's review.",
            folder: "workspaces/alpha/api/.claude/skills/review",
            compat: [],
            lint: [],
            hidden: [],
            pinned: false,
            pinned_by: [],
            owner: { kind: "repo", name: "api" },
            switch: { kind: "alone" },
          },
        ],
      },
      {
        harness: "opencode",
        title: "opencode",
        repo_left_out: [],
        skills: [],
      },
    ],
  };
}

describe("each program's own skills in the Skill studio (SK-6)", () => {
  it("marks a project skill a repo also has, and says a built-in skill is always on", async () => {
    answering(withClaudeCode(false));
    renderStudio();

    const user = userEvent.setup();
    await openSkill("skill-review", user);
    expect(await screen.findByTestId("skill-banner-clash")).toHaveTextContent(
      "the repo api has a skill of this name too, and a chat there gets both",
    );
    expect(
      within(await openSkill("skill-secrets", user)).getByTestId("skill-built-in"),
    ).toHaveTextContent(/can't be edited, moved or turned off/);
  });

  it("marks no clash on a copy left out, which no chat gets (SK-46)", async () => {
    const studio = withClaudeCode(false);
    answering({
      ...studio,
      skills: [
        {
          ...studio.skills[0],
          on: false,
          left_out: "hidden by another project skill of this name",
        },
        ...studio.skills.slice(1),
      ],
    });
    renderStudio();

    await openSkill("skill-review");
    expect(screen.queryByTestId("skill-banner-clash")).toBeNull();
  });

  it("marks a clash in the core's words, as `purlis skill list` says it (SK-46)", async () => {
    const studio = withClaudeCode(false);
    studio.clashes = [
      {
        ...studio.clashes[0],
        harness: "opencode",
        outcome: "repo-wins",
        said: "the repo api has a skill of this name too, and opencode chats there get the repo's copy alone",
      },
    ];
    answering(studio);
    renderStudio();

    await openSkill("skill-review");
    expect(await screen.findByTestId("skill-banner-clash")).toHaveTextContent(
      "the repo api has a skill of this name too, and opencode chats there get the repo's copy alone",
    );
  });

  it("renames the project's copy from the clash mark, through the studio's own Rename", async () => {
    const asked: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "skill_rename") {
        asked.push(args);
        return { notes: [] };
      }
      if (cmd === "skill_studio") return withClaudeCode(false);
      if (cmd === "skills_deleted") return [];
      throw new Error(`unexpected ${cmd}`);
    });
    renderStudio();

    await openSkill("skill-review");
    await screen.findByTestId("skill-banner-clash");
    screen.getByRole("button", { name: "Rename the project's copy" }).click();
    const dialog = await screen.findByRole("dialog", { name: "Rename review" });
    expect(within(dialog).getByLabelText("New name")).toBeInTheDocument();
  });
});
