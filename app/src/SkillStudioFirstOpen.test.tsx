// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * What the Skill studio offers when it opens (SK-22, #2785; decision 42): the skills a project
 * with no skill of its own already gets, a skill started from New skill… alone (#3684); the
 * offered skill sources, each one click to an Add source the person confirms, never an install;
 * and the move SK-21 left for here. What to offer is the core's (`skills::first_open`, its seam
 * test); the core is mocked here.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { forgetStudio, renderStudio } from "./SkillStudioTab.testkit";
import { AT_THE_ROOT } from "./skillStudio";
import { PROJECT } from "./skillChanges";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetStudio();
});

const secrets = {
  name: "secrets",
  level: "built-in",
  label: "built-in",
  local: false,
  grant: null,
  granted: null,
  turned_off: false,
  not_granted: null,
  on: true,
  left_out: null,
  description: "Keeps secrets.",
  folder: "/app/plugin/skills/secrets",
  compat: [],
  lint: [],
  hidden: [],
  pinned: false,
  pinned_by: [],
  written: "purlis:secrets",
  named: false,
  unchecked: null,
};

const ANTHROPIC = {
  offering: "well-known",
  name: "Anthropic's skills",
  why: "The collection the open skill format began with.",
  source: {
    remote: {
      url: "https://github.com/anthropics/skills",
      branch: null,
      label: "https://github.com/anthropics/skills",
    },
    folder: "skills",
    vault: null,
    key: null,
    kept: "shared",
    label: "https://github.com/anthropics/skills, skills",
  },
};

const NOTHING_FIRST = { starts: false, unmoved: null };

/** A core whose studio offers `first`, records every other ask, and answers it with
 *  `answers[cmd]`, or a change with no notes. */
function core(
  first: unknown,
  answers: Record<string, (args: Record<string, unknown>) => unknown> = {},
  offered: unknown[] = [],
) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    if (cmd === "skill_studio")
      return {
        on: true,
        workspace: null,
        persona: null,
        workspaces: [],
        levels: [{ level: PROJECT, label: "project" }],
        harnesses: [],
        clashes: [],
        skills: [secrets],
        first,
      };
    asked.push({ cmd, args: given });
    const answer = answers[cmd];
    if (answer !== undefined) return answer(given);
    if (cmd === "skills_deleted" || cmd === "skill_drafts" || cmd === "skill_sources") return [];
    if (cmd === "skill_sources_offered") return offered;
    if (cmd === "skill_updates") return { offers: [], unconfirmed: [], unasked: [] };
    if (cmd === "skill_templates")
      return [{ id: { kind: "blank" }, label: "Blank", about: "Empty." }];
    return { notes: [] };
  });
  return asked;
}

describe("the empty Skill studio", () => {
  it("lists the skills chats already get, and starts a skill only from New skill… (#3684)", async () => {
    core({ ...NOTHING_FIRST, starts: true });
    renderStudio();

    await screen.findByTestId("skill-secrets");
    // The three starts are New skill…'s ways now: the studio draws none of its own.
    expect(screen.queryByTestId("skill-studio-empty")).not.toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "Ways to start a skill" })).toBeNull();
    expect(screen.getByRole("button", { name: "New skill…" })).toBeEnabled();
  });
});

describe("the offered skill sources", () => {
  it("fills in Add source with one click, asks first, and only adds it", async () => {
    const asked = core(NOTHING_FIRST, {}, [ANTHROPIC]);
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");

    const offered = await screen.findByTestId("skill-source-offered");
    expect(offered).toHaveTextContent("Anthropic's skills");
    expect(offered).toHaveTextContent("The collection the open skill format began with.");
    await user.click(within(offered).getByRole("button", { name: "Add Anthropic's skills" }));

    const form = await screen.findByRole("dialog", { name: "Add Anthropic's skills" });
    expect(within(form).getByTestId("skill-source-offered-asks")).toHaveTextContent(
      "Nothing is installed until you review a skill from it.",
    );
    expect(within(form).getByLabelText("Repository")).toHaveValue(
      "https://github.com/anthropics/skills",
    );
    await user.click(within(form).getByRole("button", { name: "More: branch, folder, sign-in" }));
    expect(within(form).getByLabelText("Folder")).toHaveValue("skills");
    expect(asked.some((one) => one.cmd === "skill_source_add")).toBe(false);

    await user.click(within(form).getByRole("button", { name: "Add source" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_source_add")?.args).toMatchObject({
        plane: "p",
        source: {
          remote: { url: "https://github.com/anthropics/skills", branch: null },
          folder: "skills",
          kept: "shared",
        },
      }),
    );
    expect(
      asked.some((one) => one.cmd === "skill_install" || one.cmd === "skill_install_review"),
    ).toBe(false);
  });
});
