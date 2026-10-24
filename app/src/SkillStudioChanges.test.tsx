// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * The Skill studio's changes (SK-7, #2770): New skill at the level the person came from, Rename,
 * a move into the project that asks for a grant first, Delete that says who uses the skill and
 * offers Restore, the grant control, and a workspace's turn-on refused with the grant that would
 * reach it. The core is mocked; what each command does is the core's seam test.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillStudioTab } from "./SkillStudioTab";
import { AT_THE_ROOT, asPersona, inWorkspace } from "./skillStudio";
import { pickIn } from "./primitives/select.testkit";
import { levelFrom, PROJECT } from "./skillChanges";

afterEach(() => {
  cleanup();
  clearMocks();
});

const deploy = {
  name: "deploy",
  level: "project",
  label: "project",
  local: false,
  grant: "root",
  granted: { to: "root" },
  turned_off: false,
  not_granted: null,
  on: true,
  left_out: null,
  description: "Ships it.",
  folder: "skills/deploy",
  compat: [],
  lint: [],
  written: "deploy",
  named: false,
  unchecked: null,
};

/** The levels the core lists for a project with workspaces `ops` and `web`. */
const LEVELS = [null, "ops", "web"].flatMap((workspace) =>
  [false, true].map((local) => ({
    level: { workspace, local },
    label: `${local ? "local " : ""}${workspace === null ? "project" : `workspace ${workspace}`}`,
  })),
);

/** A core that answers the studio with `skills` in `workspace`, records every other ask, and
 *  answers it with `answers[cmd]` (a function of its arguments), or a change with no notes. */
function core(
  skills: unknown[],
  workspace: string | null,
  answers: Record<string, (args: Record<string, unknown>) => unknown> = {},
) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    if (cmd === "skill_studio")
      return {
        on: true,
        workspace,
        persona: (given.persona as string | null) ?? null,
        first: { starts: false, sources: [], unmoved: null },
        workspaces: ["ops", "web"],
        levels: LEVELS,
        harnesses: [],
        clashes: [],
        skills,
      };
    asked.push({ cmd, args: given });
    const answer = answers[cmd];
    if (answer !== undefined) return answer(given);
    if (cmd === "skills_deleted") return [];
    if (cmd === "skill_templates")
      return [
        { id: "blank", label: "Blank", about: "Empty." },
        { id: "steps", label: "Steps to follow", about: "Steps." },
      ];
    return { notes: [] };
  });
  return asked;
}

describe("the Skill studio's changes", () => {
  it("makes a new skill at the level the person came from", async () => {
    const asked = core([deploy], "ops");
    const user = userEvent.setup();
    render(<SkillStudioTab plane="p" scope={inWorkspace("ops")} changed={0} />);

    await user.click(await screen.findByRole("button", { name: "New skill" }));
    expect(screen.getByTestId("skill-new-level")).toHaveTextContent(
      "It lands in the workspace ops skills.",
    );
    await user.type(screen.getByLabelText("Name"), "triage");
    await user.type(screen.getByLabelText("What it does and when to use it"), "Sorts issues.");
    await user.click(screen.getByRole("button", { name: "Make it" }));

    await waitFor(() => expect(asked.some((one) => one.cmd === "skill_new")).toBe(true));
    expect(asked.find((one) => one.cmd === "skill_new")?.args).toMatchObject({
      plane: "p",
      level: { workspace: "ops", local: false },
      name: "triage",
      description: "Sorts issues.",
      template: "blank",
    });
    expect(levelFrom(AT_THE_ROOT)).toEqual(PROJECT);
  });

  it("renames a skill", async () => {
    const asked = core([deploy], null);
    const user = userEvent.setup();
    render(<SkillStudioTab plane="p" scope={AT_THE_ROOT} changed={0} />);

    await user.click(await screen.findByRole("button", { name: "Rename deploy" }));
    const box = screen.getByLabelText("New name");
    await user.clear(box);
    await user.type(box, "ship");
    await user.click(screen.getByRole("button", { name: "Rename" }));

    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_rename")?.args).toMatchObject({
        level: { workspace: null, local: false },
        name: "deploy",
        to: "ship",
      }),
    );
  });

  it("asks where a skill reaches before it becomes a project skill", async () => {
    const triage = {
      ...deploy,
      name: "triage",
      level: "workspace",
      label: "workspace",
      grant: null,
      granted: null,
      folder: "workspaces/ops/skills/triage",
      compat: [],
      lint: [],
    };
    const asked = core([triage], "ops", {
      skill_move_needs_grant: (args) =>
        (args.to as { workspace: string | null }).workspace === null,
    });
    const user = userEvent.setup();
    render(<SkillStudioTab plane="p" scope={inWorkspace("ops")} changed={0} />);

    await pickIn(await screen.findByLabelText("Move triage to"), /^project$/, user);
    const prompt = await screen.findByRole("region", {
      name: "Move triage to the project skills",
    });
    expect(asked.some((one) => one.cmd === "skill_move")).toBe(false);
    await pickIn(within(prompt).getByLabelText("Where triage will reach"), "Every workspace", user);
    await user.click(within(prompt).getByRole("button", { name: "Move" }));

    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_move")?.args).toMatchObject({
        from: { workspace: "ops", local: false },
        name: "triage",
        to: { workspace: null, local: false },
        grant: { to: "all" },
      }),
    );
  });

  it("says who uses a skill before deleting it, and offers it back", async () => {
    const asked = core([deploy], null, {
      skill_users: () => ({ root: true, workspaces: ["ops"], personas: ["steward"], skills: [] }),
      skills_deleted: () =>
        asked.some((one) => one.cmd === "skill_delete")
          ? [
              {
                id: "abc:skills/deploy",
                name: "deploy",
                level: { workspace: null, local: false },
                label: "project",
              },
            ]
          : [],
    });
    const user = userEvent.setup();
    render(<SkillStudioTab plane="p" scope={AT_THE_ROOT} changed={0} />);

    await user.click(await screen.findByRole("button", { name: "Delete deploy" }));
    const users = await screen.findByTestId("skill-users");
    expect(users).toHaveTextContent("Used by chats at the project's root, chats in ops.");
    expect(users).toHaveTextContent("Named by 1 persona: steward.");
    await user.click(
      within(screen.getByRole("region", { name: "Delete deploy" })).getByRole("button", {
        name: "Delete",
      }),
    );

    const notice = await screen.findByRole("status", { name: /Skill deleted/ });
    await user.click(within(notice).getByRole("button", { name: "Restore" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_restore")?.args).toMatchObject({
        id: "abc:skills/deploy",
      }),
    );
  });

  it("changes a project skill's grant on its row", async () => {
    const asked = core([deploy], null);
    const user = userEvent.setup();
    render(<SkillStudioTab plane="p" scope={AT_THE_ROOT} changed={0} />);

    await pickIn(await screen.findByLabelText("Where deploy reaches"), "The root and ops", user);

    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_grant")?.args).toMatchObject({
        name: "deploy",
        grant: { to: "only", workspaces: ["ops"] },
      }),
    );
  });

  it("turns a granted skill back on in a workspace", async () => {
    const asked = core(
      [{ ...deploy, on: false, turned_off: true, left_out: "turned off in this workspace" }],
      "ops",
    );
    const user = userEvent.setup();
    render(<SkillStudioTab plane="p" scope={inWorkspace("ops")} changed={0} />);

    await user.click(await screen.findByRole("button", { name: "Turn on in ops" }));

    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_turn")?.args).toMatchObject({
        workspace: "ops",
        name: "deploy",
        on: true,
      }),
    );
  });

  it("says which grant would reach a skill the workspace was not granted, and offers no on", async () => {
    const refusal =
      "deploy is not granted to ops, so it cannot be turned on there: a workspace only turns off what the project granted it. Its grant is root: a grant to ops, or to all, would reach it";
    core(
      [{ ...deploy, on: false, not_granted: refusal, left_out: "not granted to this workspace" }],
      "ops",
    );
    render(<SkillStudioTab plane="p" scope={inWorkspace("ops")} changed={0} />);

    expect(await screen.findByTestId("not-granted-deploy")).toHaveTextContent(refusal);
    expect(screen.queryByRole("button", { name: "Turn on in ops" })).toBeNull();
  });

  it("adds a listed skill to a persona's skills from its row, and removes a named one", async () => {
    const asked = core(
      [
        { ...deploy, name: "lint", written: "lint", named: true },
        { ...deploy, name: "triage", written: "triage", named: false },
      ],
      null,
      { persona_skill: () => true },
    );
    const user = userEvent.setup();
    render(<SkillStudioTab plane="p" scope={asPersona("ops")} changed={0} />);

    await user.click(await screen.findByRole("button", { name: "Remove lint from ops" }));
    await user.click(await screen.findByRole("button", { name: "Add triage to ops" }));

    await waitFor(() =>
      expect(asked.filter((one) => one.cmd === "persona_skill").map((one) => one.args)).toEqual([
        { plane: "p", persona: "ops", written: "lint", change: "remove" },
        { plane: "p", persona: "ops", written: "triage", change: "add" },
      ]),
    );
  });

  it("never offers to change a built-in skill", async () => {
    core(
      [
        {
          ...deploy,
          name: "secrets",
          level: "built-in",
          label: "built-in",
          grant: null,
          granted: null,
        },
      ],
      null,
    );
    render(<SkillStudioTab plane="p" scope={AT_THE_ROOT} changed={0} />);

    const row = await screen.findByTestId("skill-secrets");
    expect(within(row).queryByRole("button", { name: "Edit secrets" })).toBeNull();
    expect(row).not.toHaveAttribute("draggable", "true");
  });
});
