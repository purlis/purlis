// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * The Skill studio's changes (SK-7, #2770): New skill at the level the person came from, Rename,
 * a move into the project that asks for a grant first, Delete that says who uses the skill and
 * offers Restore, the grant control, and a workspace's turn-on refused with the grant that would
 * reach it. Each is done from the skill chosen in the Library, beside the list (#3681). The core
 * is mocked; what each command does is the core's seam test.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { chooseMore, forgetStudio, openSkill, renderStudio } from "./SkillStudioTab.testkit";
import { AT_THE_ROOT, asPersona, inWorkspace } from "./skillStudio";
import { offeredIn, pickIn } from "./ui/select/select.testkit";
import { levelFrom, PROJECT } from "./skillChanges";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetStudio();
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
  hidden: [],
  pinned: false,
  pinned_by: [],
  written: "deploy",
  named: false,
  unchecked: null,
};

/** The levels the core lists for a project with workspaces `ops` and `web` and personas
 *  `reviewer` and `steward`. */
const LEVELS = [
  ...[false, true].map((local) => ({
    level: { at: "project" as const, local },
    label: `${local ? "local " : ""}project`,
  })),
  ...(["workspace", "persona"] as const).flatMap((at) =>
    (at === "workspace" ? ["ops", "web"] : ["reviewer", "steward"]).flatMap((name) =>
      [false, true].map((local) => ({
        level: { at, name, local },
        label: `${local ? "local " : ""}${at} ${name}`,
      })),
    ),
  ),
];

/** A core that answers the studio with `skills` in `workspace`, records every other ask, and
 *  answers it with `answers[cmd]` (a function of its arguments), or a change with no notes. */
function core(
  skills: unknown[],
  workspace: string | null,
  answers: Record<string, (args: Record<string, unknown>) => unknown> = {},
  studio: Record<string, unknown> = {},
) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    if (cmd === "skill_studio")
      return {
        on: true,
        workspace,
        persona: (given.persona as string | null) ?? null,
        first: { starts: false, unmoved: null },
        workspaces: ["ops", "web"],
        levels: LEVELS,
        harnesses: [],
        clashes: [],
        skills,
        pins_refused: null,
        ...studio,
      };
    asked.push({ cmd, args: given });
    const answer = answers[cmd];
    if (answer !== undefined) return answer(given);
    if (cmd === "skills_deleted") return [];
    if (cmd === "skill_templates")
      return [
        { id: { kind: "blank" }, label: "Blank", about: "Empty." },
        { id: { kind: "steps" }, label: "Steps to follow", about: "Steps." },
      ];
    return { notes: [] };
  });
  return asked;
}

describe("the Skill studio's changes", () => {
  it("makes a new skill at the level the person came from", async () => {
    const asked = core([deploy], "ops");
    const user = userEvent.setup();
    renderStudio(inWorkspace("ops"));

    await user.click(await screen.findByRole("button", { name: "New skill…" }));
    expect(screen.getByTestId("skill-new-level")).toHaveTextContent(
      "It lands in the workspace ops skills.",
    );
    await user.type(screen.getByLabelText("Name"), "triage");
    await user.type(screen.getByLabelText("What it does and when to use it"), "Sorts issues.");
    await user.click(screen.getByRole("button", { name: "Make it" }));

    await waitFor(() => expect(asked.some((one) => one.cmd === "skill_new")).toBe(true));
    expect(asked.find((one) => one.cmd === "skill_new")?.args).toMatchObject({
      plane: "p",
      level: { at: "workspace", name: "ops", local: false },
      name: "triage",
      description: "Sorts issues.",
      template: { kind: "blank" },
    });
    expect(levelFrom(AT_THE_ROOT, LEVELS)).toEqual(PROJECT);
    expect(levelFrom(asPersona("reviewer"), LEVELS)).toEqual({
      at: "persona",
      name: "reviewer",
      local: false,
    });
    // A persona made as one file keeps no skills: the project's is where a new one lands.
    expect(levelFrom(asPersona("flat"), LEVELS)).toEqual(PROJECT);
  });

  it("renames a skill", async () => {
    const asked = core([deploy], null);
    const user = userEvent.setup();
    renderStudio();

    await openSkill("skill-deploy", user);
    await chooseMore("deploy", /^Rename/, user);
    const box = screen.getByLabelText("New name");
    await user.clear(box);
    await user.type(box, "ship");
    await user.click(screen.getByRole("button", { name: "Rename" }));

    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_rename")?.args).toMatchObject({
        level: { at: "project", local: false },
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
      hidden: [],
      pinned: false,
    };
    const asked = core([triage], "ops", {
      skill_move_needs_grant: (args) => (args.to as { at: string }).at === "project",
    });
    const user = userEvent.setup();
    renderStudio(inWorkspace("ops"));

    await openSkill("skill-triage", user);
    await chooseMore("triage", /^Move to another level/, user);
    await pickIn(await screen.findByLabelText("Move triage to"), /^project$/, user);
    const prompt = await screen.findByRole("region", {
      name: "Move triage to the project skills",
    });
    expect(asked.some((one) => one.cmd === "skill_move")).toBe(false);
    await pickIn(within(prompt).getByLabelText("Where triage will reach"), "Every workspace", user);
    await user.click(within(prompt).getByRole("button", { name: "Move" }));

    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_move")?.args).toMatchObject({
        from: { at: "workspace", name: "ops", local: false },
        name: "triage",
        to: { at: "project", local: false },
        grant: { to: "all" },
      }),
    );
  });

  it("says who uses a skill before deleting it, and offers it back", async () => {
    const asked = core([deploy], null, {
      skill_users: () => ({
        root: true,
        workspaces: ["ops"],
        persona: null,
        personas: ["steward"],
        skills: [],
      }),
      skills_deleted: () =>
        asked.some((one) => one.cmd === "skill_delete")
          ? [
              {
                id: "abc:skills/deploy",
                name: "deploy",
                level: { at: "project", local: false },
                label: "project",
                stays: "kept on this machine for 30 more days",
              },
            ]
          : [],
    });
    const user = userEvent.setup();
    renderStudio();

    await openSkill("skill-deploy", user);
    await chooseMore("deploy", /^Delete/, user);
    const users = await screen.findByTestId("skill-users");
    expect(users).toHaveTextContent("Used by chats at the project's root, chats in ops.");
    expect(users).toHaveTextContent("Named by 1 persona: steward.");
    await user.click(
      within(screen.getByRole("region", { name: "Delete deploy" })).getByRole("button", {
        name: "Delete",
      }),
    );

    const notice = await screen.findByRole("status", { name: /Skill deleted/ });
    // The restore list, on the Deleted page, says how long the deleted skill stays (SK-33).
    await user.click(await screen.findByRole("button", { name: /^Deleted/ }));
    expect(await screen.findByTestId("deleted-deploy")).toHaveTextContent(
      "kept on this machine for 30 more days",
    );
    await user.click(within(notice).getByRole("button", { name: "Restore" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_restore")?.args).toMatchObject({
        id: "abc:skills/deploy",
      }),
    );
  });

  it("changes a project skill's grant beside the list", async () => {
    const asked = core([deploy], null);
    const user = userEvent.setup();
    renderStudio();

    await openSkill("skill-deploy", user);
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
    renderStudio(inWorkspace("ops"));

    await openSkill("skill-deploy", user);
    await user.click(screen.getByRole("button", { name: "Turn on in ops" }));

    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_turn")?.args).toMatchObject({
        workspace: "ops",
        name: "deploy",
        on: true,
      }),
    );
  });

  it("pins a skill for a workspace's chats with its Pin switch, one change (SK-15d)", async () => {
    const asked = core([deploy], "ops");
    const user = userEvent.setup();
    renderStudio(inWorkspace("ops"));

    await openSkill("skill-deploy", user);
    const pin = await screen.findByRole("switch", { name: "Pin deploy for workspace ops" });
    expect(pin).not.toBeChecked();
    await user.click(pin);

    await waitFor(() =>
      expect(asked.filter((one) => one.cmd === "skill_pin").map((one) => one.args)).toEqual([
        { plane: "p", by: { at: "workspace", name: "ops" }, name: "deploy", on: true },
      ]),
    );
  });

  it("unpins a skill a persona pins, and offers no pin at the project's root (SK-15d)", async () => {
    const pinned = { ...deploy, pinned: true, pinned_by: [{ at: "persona", name: "reviewer" }] };
    const asked = core([pinned], null);
    const user = userEvent.setup();
    renderStudio(asPersona("reviewer"));

    await openSkill("skill-deploy", user);
    const pin = await screen.findByRole("switch", { name: "Pin deploy for persona reviewer" });
    expect(pin).toBeChecked();
    await user.click(pin);

    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_pin")?.args).toMatchObject({
        by: { at: "persona", name: "reviewer" },
        name: "deploy",
        on: false,
      }),
    );
    cleanup();
    forgetStudio();
    core([pinned], null);
    renderStudio();
    await openSkill("skill-deploy");
    expect(screen.queryByRole("switch", { name: /^Pin deploy/ })).toBeNull();
  });

  it("offers no pin on a built-in skill or on a hidden copy (SK-15d)", async () => {
    core(
      [
        {
          ...deploy,
          level: "built-in",
          label: "built-in",
          granted: null,
          folder: "x/handoff",
          name: "handoff",
        },
        {
          ...deploy,
          level: "workspace",
          label: "workspace",
          granted: null,
          folder: "workspaces/ops/skills/deploy",
        },
        { ...deploy, on: false, left_out: "hidden by the workspace's copy" },
      ],
      "ops",
    );
    renderStudio(inWorkspace("ops"));

    const user = userEvent.setup();
    await openSkill("skill-handoff", user);
    expect(screen.queryByRole("switch", { name: "Pin handoff for workspace ops" })).toBeNull();
    const shown = await openSkill("skill-deploy", user);
    expect(
      within(shown).getByRole("switch", { name: "Pin deploy for workspace ops" }),
    ).toBeInTheDocument();
    await openSkill("skill-deploy-hidden-1", user);
    expect(screen.queryByRole("switch", { name: /^Pin deploy/ })).toBeNull();
  });

  it("gives the Pin switch to the copy a chat gets, not the first row of the name (SK-15d)", async () => {
    core(
      [
        { ...deploy, on: false, left_out: "turned off in this workspace" },
        {
          ...deploy,
          level: "workspace",
          label: "workspace",
          granted: null,
          folder: "workspaces/ops/skills/deploy",
        },
      ],
      "ops",
    );
    renderStudio(inWorkspace("ops"));

    const user = userEvent.setup();
    const shown = await openSkill("skill-deploy-hidden-1", user);
    expect(
      within(shown).getByRole("switch", { name: "Pin deploy for workspace ops" }),
    ).toBeInTheDocument();
    const first = await openSkill("skill-deploy", user);
    expect(within(first).queryByRole("switch", { name: /^Pin deploy/ })).toBeNull();
  });

  it("says a workspace's pins aren't used and removes the entry that is no skill's name (SK-15d)", async () => {
    const refused = {
      by: { at: "workspace", name: "ops" },
      why: '"Not A Name" is not a skill\'s name (it holds a capital)',
      bad: "Not A Name",
      file: "workspaces/ops/workspace.json",
    };
    const asked = core(
      [{ ...deploy, pinned: false, pinned_by: [{ at: "workspace", name: "ops" }] }],
      "ops",
      {},
      { pins_refused: refused },
    );
    const user = userEvent.setup();
    // On Needs you, which is in the navigation while a pin is refused (#3681).
    renderStudio(inWorkspace("ops"), "needs");

    expect(await screen.findByTestId("skill-pins-refused")).toHaveTextContent(
      `workspace ops's pins aren't used: "Not A Name" is not a skill's name (it holds a capital).`,
    );
    expect(screen.getByRole("button", { name: /^Needs you/ })).toHaveAttribute(
      "aria-current",
      "page",
    );
    await user.click(screen.getByRole("button", { name: "Library" }));
    const item = await openSkill("skill-deploy", user);
    const pin = within(item).getByRole("switch", { name: "Pin deploy for workspace ops" });
    expect(pin).toBeChecked();
    expect(within(item).getByText("Pinned, not used")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^Needs you/ }));
    await user.click(
      screen.getByRole("button", { name: 'Remove it, the pin "Not A Name" of workspace ops' }),
    );
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_pin")?.args).toMatchObject({
        by: { at: "workspace", name: "ops" },
        name: "Not A Name",
        on: false,
      }),
    );
  });

  it("points to the file where a persona's pins are too many to be used (SK-15d)", async () => {
    core(
      [deploy],
      null,
      {},
      {
        pins_refused: {
          by: { at: "persona", name: "reviewer" },
          why: "the list holds more than 64 pins",
          bad: null,
          file: "personas/reviewer/persona.md",
        },
      },
    );
    renderStudio(asPersona("reviewer"), "needs");

    expect(await screen.findByTestId("skill-pins-refused")).toHaveTextContent(
      "persona reviewer's pins aren't used: the list holds more than 64 pins. Unpin some, or edit personas/reviewer/persona.md.",
    );
  });

  it("says which grant would reach a skill the workspace was not granted, and offers no on", async () => {
    const refusal =
      "deploy is not granted to ops, so it cannot be turned on there: a workspace only turns off what the project granted it. Its grant is root: a grant to ops, or to all, would reach it";
    core(
      [{ ...deploy, on: false, not_granted: refusal, left_out: "not granted to this workspace" }],
      "ops",
    );
    renderStudio(inWorkspace("ops"));

    await openSkill("skill-deploy");
    expect(await screen.findByTestId("not-granted-deploy")).toHaveTextContent(refusal);
    expect(screen.queryByRole("button", { name: "Turn on in ops" })).toBeNull();
  });

  it("adds a listed skill to a persona's skills from its item, and removes a named one", async () => {
    const asked = core(
      [
        { ...deploy, name: "lint", written: "lint", named: true },
        { ...deploy, name: "triage", written: "triage", named: false },
      ],
      null,
      { persona_skill: () => true },
    );
    const user = userEvent.setup();
    renderStudio(asPersona("ops"));

    await openSkill("skill-lint", user);
    await user.click(screen.getByRole("button", { name: "Remove lint from ops" }));
    await waitFor(() => expect(asked.some((one) => one.cmd === "persona_skill")).toBe(true));
    await openSkill("skill-triage", user);
    await user.click(screen.getByRole("button", { name: "Add triage to ops" }));

    await waitFor(() =>
      expect(asked.filter((one) => one.cmd === "persona_skill").map((one) => one.args)).toEqual([
        { plane: "p", persona: "ops", written: "lint", change: "remove" },
        { plane: "p", persona: "ops", written: "triage", change: "add" },
      ]),
    );
  });

  it("changes a persona's own skill from its studio, and offers its levels alone", async () => {
    // SK-32 (#3003): a persona's own row is at the persona the studio shows.
    const lint = {
      ...deploy,
      name: "lint",
      level: "persona",
      label: "persona",
      grant: null,
      granted: null,
      folder: "personas/reviewer/skills/lint",
      written: "lint",
      named: false,
    };
    const asked = core([lint], null, {
      skill_move_needs_grant: (args) => (args.to as { at: string }).at === "project",
      skill_users: () => ({
        root: false,
        workspaces: [],
        persona: "reviewer",
        personas: [],
        skills: [],
      }),
    });
    const user = userEvent.setup();
    renderStudio(asPersona("reviewer"));

    const row = await openSkill("skill-lint", user);
    // Its own skill needs no name in its skills.
    expect(within(row).queryByRole("button", { name: "Add lint to reviewer" })).toBeNull();
    // It moves to the project's levels and this persona's, never another persona's.
    await chooseMore("lint", /^Move to another level/, user);
    const offered = await offeredIn(await screen.findByLabelText("Move lint to"), user);
    expect(offered).toContain("project");
    expect(offered.some((one) => one.includes("steward"))).toBe(false);
    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    await chooseMore("lint", /^Rename/, user);
    const box = screen.getByLabelText("New name");
    await user.clear(box);
    await user.type(box, "check");
    await user.click(screen.getByRole("button", { name: "Rename" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_rename")?.args).toMatchObject({
        level: { at: "persona", name: "reviewer", local: false },
        name: "lint",
        to: "check",
      }),
    );

    // The faked core keeps its old name, which a real rename would not: chosen again.
    await openSkill("skill-lint", user);
    await chooseMore("lint", /^Move to another level/, user);
    await pickIn(await screen.findByLabelText("Move lint to"), /^project$/, user);
    const prompt = await screen.findByRole("region", { name: "Move lint to the project skills" });
    expect(within(prompt).getByTestId("persona-keeps")).toHaveTextContent(
      "reviewer keeps it in every workspace",
    );
    await user.click(within(prompt).getByRole("button", { name: "Move" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_move")?.args).toMatchObject({
        from: { at: "persona", name: "reviewer", local: false },
        name: "lint",
        to: { at: "project", local: false },
        grant: { to: "root" },
      }),
    );

    await openSkill("skill-lint", user);
    await chooseMore("lint", /^Delete/, user);
    expect(await screen.findByTestId("skill-users")).toHaveTextContent(
      "Used by chats as reviewer, in every workspace.",
    );
  });

  it("makes a new skill at the persona the studio shows", async () => {
    const asked = core([deploy], null);
    const user = userEvent.setup();
    renderStudio(asPersona("reviewer"));

    await user.click(await screen.findByRole("button", { name: "New skill…" }));
    await user.type(screen.getByLabelText("Name"), "triage");
    await user.type(screen.getByLabelText("What it does and when to use it"), "Sorts issues.");
    await user.click(screen.getByRole("button", { name: "Make it" }));

    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_new")?.args).toMatchObject({
        level: { at: "persona", name: "reviewer", local: false },
        name: "triage",
      }),
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
    renderStudio();

    const item = await openSkill("skill-secrets");
    expect(within(item).queryByRole("button", { name: "Edit secrets" })).toBeNull();
    expect(within(item).queryByLabelText("Move secrets to")).toBeNull();
  });
});
