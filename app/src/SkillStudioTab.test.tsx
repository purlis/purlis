// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { StudioRouterLine } from "./SkillStudioTab";
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
  it("asks once for the project where it cannot confirm which folders were accepted", async () => {
    const said =
      "purlis can't confirm which of this project's folders you accepted before: accept them again in the Skill studio";
    const asked: string[] = [];
    mockIPC((cmd) => {
      if (cmd === "skill_studio")
        return {
          on: true,
          workspace: null,
          first: { starts: false, unmoved: null },
          workspaces: [],
          levels: [],
          harnesses: [],
          clashes: [],
          skills: [],
          pins_refused: null,
          not_supported: [],
          levels_to_confirm: said,
        };
      if (cmd === "skills_deleted") return [];
      if (cmd === "skill_levels_confirm") {
        asked.push(cmd);
        return {};
      }
      return null;
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "needs");
    const item = await screen.findByTestId("skill-studio-levels-to-confirm");
    expect(item).toHaveTextContent(said);
    await user.click(screen.getByRole("button", { name: "Accept the folders" }));
    await waitFor(() => expect(asked).toEqual(["skill_levels_confirm"]));
  });

  it("says in one line why a declared harness's chats get no skills (SK-30)", async () => {
    const said =
      "Skills are not supported on Goose: it has no skill format purlis can hand skills by, and its declaration does not say it takes MCP servers, so its chats get no skills.";
    answering({
      on: true,
      workspace: null,
      first: { starts: false, unmoved: null },
      workspaces: [],
      levels: [],
      harnesses: [],
      clashes: [],
      skills: [],
      pins_refused: null,
      not_supported: [{ name: "goose", title: "Goose", said }],
    });
    renderStudio(AT_THE_ROOT, "harness");
    const line = await screen.findByTestId("skill-studio-not-supported-goose");
    expect(line).toHaveTextContent("Goose");
    expect(line).toHaveTextContent(said);
  });

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
    expect(within(await openSkill("skill-deploy")).getByText("On")).toBeInTheDocument();
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
    expect(
      within(deploy).getByText(
        "On, hidden by the built-in router: over this chat's skill budget, so it is in the index skill (on Codex)",
      ),
    ).toBeInTheDocument();
    expect(
      within(await openSkill("skill-review", user)).getByText("On, pinned"),
    ).toBeInTheDocument();
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
    expect(within(triage).queryByRole("button", { name: /Remove/ })).toBeNull();
    expect(
      within(await openSkill("skill-deploy", user)).getByRole("button", {
        name: "Remove deploy from ops",
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
  it("lists them with whose each is, and shows a plugin's skills read-only, naming the plugin", async () => {
    answering(withClaudeCode(false));
    renderStudio(AT_THE_ROOT, "harness");

    const claude = await screen.findByTestId("skill-studio-claude");
    expect(within(claude).getByRole("heading", { name: "Claude Code" })).toBeInTheDocument();
    const own = within(claude).getByTestId("found-claude-langfuse");
    expect(within(own).getByText("You")).toBeInTheDocument();
    expect(within(own).getByRole("button", { name: "Turn off" })).toBeInTheDocument();
    const plugin = within(claude).getByTestId("found-claude-figma:frame");
    expect(within(plugin).queryByRole("button")).toBeNull();
    expect(within(plugin).getByText("Plugin figma")).toBeInTheDocument();
    expect(
      within(plugin).getByText(/it comes with the plugin figma, which Claude Code turns on or off/),
    ).toBeInTheDocument();
    expect(
      within(within(claude).getByTestId("found-claude-review")).getByText("Repo api"),
    ).toBeInTheDocument();
    // #3467: no plugin of a program's is listed to switch, purlis's own included.
    expect(within(claude).queryByRole("table", { name: "Claude Code plugins" })).toBeNull();
  });

  it("marks a project skill a repo also has, and says a built-in skill is always on", async () => {
    answering(withClaudeCode(false));
    renderStudio();

    const user = userEvent.setup();
    await openSkill("skill-review", user);
    expect(await screen.findByTestId("clash-review")).toHaveTextContent(
      "the repo api has a skill of this name too, and a chat there gets both",
    );
    expect(
      within(await openSkill("skill-secrets", user)).getByText(/cannot be turned off/),
    ).toBeInTheDocument();
  });

  it("turns one off through the core and reads the project again", async () => {
    let off = false;
    const asked: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "skill_studio_switch") {
        asked.push(args);
        off = true;
        return null;
      }
      if (cmd === "skill_studio") return withClaudeCode(off);
      throw new Error(`unexpected ${cmd}`);
    });
    renderStudio(AT_THE_ROOT, "harness");

    const row = await screen.findByTestId("found-claude-langfuse");
    within(row).getByRole("button", { name: "Turn off" }).click();

    await waitFor(() =>
      expect(
        within(screen.getByTestId("found-claude-langfuse")).getByText(
          "Left out: turned off for this project's chats",
        ),
      ).toBeInTheDocument(),
    );
    expect(asked).toEqual([
      { plane: "p", workspace: null, harness: "claude", id: "langfuse", on: false },
    ]);
  });

  it("says what the core refused, and changes nothing", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_studio_switch") throw "Claude Code turns it off only with its plugin";
      if (cmd === "skill_studio") return withClaudeCode(false);
      throw new Error(`unexpected ${cmd}`);
    });
    renderStudio(AT_THE_ROOT, "harness");
    const own = await screen.findByTestId("found-claude-langfuse");
    within(own).getByRole("button", { name: "Turn off" }).click();
    expect(
      await screen.findByText("Claude Code turns it off only with its plugin"),
    ).toBeInTheDocument();
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
    expect(screen.queryByTestId("clash-review")).toBeNull();
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
    expect(await screen.findByTestId("clash-review")).toHaveTextContent(
      "the repo api has a skill of this name too, and opencode chats there get the repo's copy alone",
    );
  });

  it("lists under a program each project skill a repo leaves out of its chats, with why (SK-53)", async () => {
    const studio = withClaudeCode(false);
    const opencode = {
      ...studio.harnesses[1],
      repo_left_out: [
        {
          ...ROW,
          name: "ship",
          level: "workspace" as const,
          label: "workspace",
          on: false,
          left_out:
            "purlis could not read the skills folder of web whole, so a copy of this skill may be there, and a chat in that repo would get that one",
          folder: "workspaces/alpha/skills/ship",
        },
      ],
    };
    answering({ ...studio, harnesses: [studio.harnesses[0], opencode] });
    renderStudio(AT_THE_ROOT, "harness");

    const section = await screen.findByTestId("skill-studio-opencode");
    expect(
      within(section).getByText(
        "opencode chats in a repo here start without 1 skill of this project's:",
      ),
    ).toBeInTheDocument();
    const row = within(section).getByTestId("repo-left-out-opencode-ship");
    expect(row).toHaveTextContent("workspace");
    expect(row).toHaveTextContent("Left out: purlis could not read the skills folder of web whole");
    expect(
      within(screen.getByTestId("skill-studio-claude")).queryByText(/start without/),
    ).toBeNull();
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
    const clash = await screen.findByTestId("clash-review");
    within(clash).getByRole("button", { name: "Rename the project's copy" }).click();
    const form = await screen.findByRole("form", { name: "Rename review" });
    expect(within(form).getByLabelText("New name")).toBeInTheDocument();
  });
});

describe("the router a studio's chats are routed by (SK-15b)", () => {
  afterEach(cleanup);

  it("says nothing of the built-in router, and names an extension's with each chat's timing", () => {
    const { container } = render(
      <StudioRouterLine
        router={{ said: "built-in", problem: false, extension: null, chats: [], grade: null }}
      />,
    );
    expect(container.textContent).toBe("");
    cleanup();
    render(
      <StudioRouterLine
        router={{
          said: "Chats here are routed by the router relevance.",
          problem: false,
          extension: "relevance",
          grade: null,
          chats: [
            {
              session: 3,
              routing: {
                router: "relevance",
                ms: 34,
                fell_back: false,
                said: "Routed by the router relevance in 34 ms.",
              },
            },
            {
              session: 4,
              routing: {
                router: "relevance",
                ms: 100,
                fell_back: true,
                said: "The router relevance did not answer within 100 ms, so this chat lists what the built-in router shows.",
              },
            },
          ],
        }}
      />,
    );
    const line = screen.getByTestId("skill-studio-router");
    expect(line.textContent).toMatch(/routed by the router relevance/);
    const chats = within(line).getAllByRole("listitem");
    expect(chats.map((one) => one.textContent)).toEqual([
      "Chat 3: Routed by the router relevance in 34 ms.",
      "Chat 4: The router relevance did not answer within 100 ms, so this chat lists what the built-in router shows.",
    ]);
    expect(chats[1].getAttribute("data-fell-back")).toBe("true");
    expect(screen.getByTestId("skill-studio-router-grade").textContent).toMatch(
      /has not been graded on this library yet/,
    );
    // No Grade where the line is drawn with nothing to grade from.
    expect(screen.queryByTestId("router-grade")).toBeNull();
  });

  it("says the router's newest grade beside the library's commit (SK-15e)", () => {
    render(
      <StudioRouterLine
        router={{
          said: "Chats here are routed by the router relevance.",
          problem: false,
          extension: "relevance",
          chats: [],
          grade:
            "Graded on the library at aaaaaaaaaaaa, 2026-10-02: 1 of 2 trigger tests pass with the router relevance, and 2 of 2 with the built-in router.",
        }}
        grading={{ plane: "p", workspace: "ops", persona: null, onGraded: () => undefined }}
      />,
    );
    expect(screen.getByTestId("skill-studio-router-grade").textContent).toBe(
      "Graded on the library at aaaaaaaaaaaa, 2026-10-02: 1 of 2 trigger tests pass with the router relevance, and 2 of 2 with the built-in router.",
    );
    expect(screen.getByRole("button", { name: "Grade the router…" })).toBeInTheDocument();
  });

  it("says why a choice that names no router is the built-in one", () => {
    render(
      <StudioRouterLine
        router={{
          said: 'the workspace ops chooses "x/y" as its router, which is neither built-in nor an extension\'s id, so its chats are routed by the built-in router.',
          problem: true,
          extension: null,
          chats: [],
          grade: null,
        }}
      />,
    );
    const line = screen.getByTestId("skill-studio-router");
    expect(line.className).toContain("use-skill-over");
    expect(line.textContent).toMatch(/routed by the built-in router/);
  });
});
