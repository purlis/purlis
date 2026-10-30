// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * The Skill studio's changes (SK-7, #2770): New skill at the level the person came from; Rename,
 * Move and Delete as dialogs that say what changes and who is affected, ending in a notice with
 * Undo or Bring it back (#3685); the grant control, and a workspace's turn-on refused with the grant that would
 * reach it. Each is done from the skill chosen in the Library, beside the list (#3681). The core
 * is mocked; what each command does is the core's seam test.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { chooseMore, forgetStudio, openSkill, renderStudio } from "./SkillStudioTab.testkit";
import { need } from "./skillsListing.testkit";
import { AT_THE_ROOT, asPersona, inWorkspace } from "./skillStudio";
import { pickIn } from "./ui/select/select.testkit";
import { levelFrom, PROJECT } from "./skillChanges";
import { DELETED_FOR, HISTORY_RESTARTS } from "./SkillChangeDialogs";
import { NAME_RESERVED } from "./SkillCopy";

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

  it("renames a skill in a dialog that checks the name and says what changes with it", async () => {
    const triage = { ...deploy, name: "triage", folder: "skills/triage" };
    const secrets = { ...deploy, name: "secrets", level: "built-in", label: "built-in" };
    const asked = core([deploy, triage, secrets], null, {
      skill_users: () => ({
        root: true,
        workspaces: ["ops"],
        persona: null,
        personas: ["steward"],
        skills: ["release"],
        pinned_by: [{ at: "workspace", name: "ops" }],
      }),
    });
    const user = userEvent.setup();
    renderStudio();

    await openSkill("skill-deploy", user);
    await chooseMore("deploy", /^Rename/, user);
    const dialog = await screen.findByRole("dialog", { name: "Rename deploy" });
    const reach = await within(dialog).findByRole("region", { name: "Changed with it" });
    await waitFor(() => expect(reach).toHaveTextContent("1 persona that names it: steward"));
    expect(reach).toHaveTextContent("1 skill that names it: release");
    expect(reach).toHaveTextContent("Its pins, by workspace ops");
    expect(dialog).toHaveTextContent(HISTORY_RESTARTS);

    const box = within(dialog).getByLabelText("New name");
    const rename = within(dialog).getByRole("button", { name: "Rename" });
    for (const [typed, says] of [
      ["Ship", /lowercase letters, digits and hyphens/],
      ["secrets", NAME_RESERVED],
      ["triage", "There is already a skill named triage here."],
    ] as const) {
      await user.clear(box);
      await user.type(box, typed);
      expect(within(dialog).getByTestId("skill-rename-problem")).toHaveTextContent(says);
      expect(rename).toBeDisabled();
    }
    await user.clear(box);
    await user.type(box, "ship");
    await user.click(rename);

    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_rename")?.args).toMatchObject({
        level: { at: "project", local: false },
        name: "deploy",
        to: "ship",
      }),
    );
    const notice = await screen.findByRole("status", { name: /Skill renamed/ });
    expect(notice).toHaveTextContent("deploy is ship now.");
    expect(screen.queryByRole("dialog", { name: "Rename deploy" })).toBeNull();
    await user.click(within(notice).getByRole("button", { name: "Undo" }));
    await waitFor(() =>
      expect(asked.filter((one) => one.cmd === "skill_rename")[1]?.args).toMatchObject({
        level: { at: "project", local: false },
        name: "ship",
        to: "deploy",
      }),
    );
    expect(await screen.findByRole("status", { name: /Rename undone/ })).toHaveTextContent(
      "ship is deploy again.",
    );
  });

  it("says the core's refusal of a rename in its dialog, which stays open", async () => {
    core([deploy], null, {
      skill_users: () => ({
        root: true,
        workspaces: [],
        persona: null,
        personas: [],
        skills: [],
        pinned_by: [],
      }),
      skill_rename: () => {
        throw "the project is opened read-only";
      },
    });
    const user = userEvent.setup();
    renderStudio();

    await openSkill("skill-deploy", user);
    await chooseMore("deploy", /^Rename/, user);
    const dialog = await screen.findByRole("dialog", { name: "Rename deploy" });
    expect(
      await within(dialog).findByText("No persona, skill or pin names it."),
    ).toBeInTheDocument();
    const box = within(dialog).getByLabelText("New name");
    await user.clear(box);
    await user.type(box, "ship");
    await user.click(within(dialog).getByRole("button", { name: "Rename" }));

    expect(await within(dialog).findByTestId("skill-rename-refused")).toHaveTextContent(
      "the project is opened read-only",
    );
    // A new name typed takes the refusal of the last one away.
    await user.type(box, "s");
    expect(within(dialog).queryByTestId("skill-rename-refused")).toBeNull();
  });

  it("moves a skill in a dialog of every level and who gets it, asking where it reaches in the project", async () => {
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
      skill_move_needs_grant: (args) =>
        (args.to as { at: string }).at === "project" &&
        (args.from as { at: string }).at !== "project",
      skill_users: () => ({
        root: false,
        workspaces: ["ops"],
        persona: null,
        personas: [],
        skills: [],
        pinned_by: [],
      }),
    });
    const user = userEvent.setup();
    renderStudio(inWorkspace("ops"));

    await openSkill("skill-triage", user);
    await chooseMore("triage", /^Move to another level/, user);
    const dialog = await screen.findByRole("dialog", { name: "Move triage to another level" });
    expect(dialog).toHaveTextContent("It is a workspace ops skill now. Only chats in ops get it.");
    // Each level says who gets it there; the one it is at is not offered.
    const web = within(dialog).getByRole("radio", { name: /^workspace web/ });
    expect(within(dialog).queryByRole("radio", { name: /^workspace ops/ })).toBeNull();
    expect(dialog).toHaveTextContent("Only chats in web get it.");
    expect(within(dialog).getByRole("button", { name: "Move" })).toBeDisabled();

    await user.click(web);
    const changes = await within(dialog).findByRole("region", { name: "What changes" });
    await waitFor(() => expect(changes).toHaveTextContent("Chats in ops stop getting it."));
    expect(changes).toHaveTextContent("Chats in web start getting it.");

    await user.click(within(dialog).getByRole("radio", { name: /^project/ }));
    await pickIn(
      await within(dialog).findByLabelText("Where triage will reach"),
      "Every workspace",
      user,
    );
    await waitFor(() =>
      expect(within(dialog).getByRole("region", { name: "What changes" })).toHaveTextContent(
        "Chats at the project's root, chats in web start getting it.",
      ),
    );
    expect(asked.some((one) => one.cmd === "skill_move")).toBe(false);
    await user.click(within(dialog).getByRole("button", { name: "Move" }));

    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_move")?.args).toMatchObject({
        from: { at: "workspace", name: "ops", local: false },
        name: "triage",
        to: { at: "project", local: false },
        grant: { to: "all" },
      }),
    );
    const notice = await screen.findByRole("status", { name: /Skill moved/ });
    expect(notice).toHaveTextContent("triage is a project skill now.");
    await user.click(within(notice).getByRole("button", { name: "Undo" }));
    await waitFor(() =>
      expect(asked.filter((one) => one.cmd === "skill_move")[1]?.args).toMatchObject({
        from: { at: "project", local: false },
        name: "triage",
        to: { at: "workspace", name: "ops", local: false },
        grant: null,
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
        pinned_by: [{ at: "persona", name: "steward" }],
      }),
      skills_deleted: () =>
        asked.some((one) => one.cmd === "skill_delete")
          ? [
              {
                id: "abc:skills/deploy",
                name: "deploy",
                level: { at: "project", local: false },
                label: "project",
                days_left: 30,
                in_history: false,
                reserved: false,
              },
            ]
          : [],
    });
    const user = userEvent.setup();
    renderStudio();

    await openSkill("skill-deploy", user);
    await chooseMore("deploy", /^Delete/, user);
    const dialog = await screen.findByRole("dialog", { name: "Delete deploy?" });
    expect(dialog).toHaveTextContent(DELETED_FOR);
    const users = await within(dialog).findByTestId("skill-users");
    await waitFor(() =>
      expect(users).toHaveTextContent("Chats at the project's root, chats in ops."),
    );
    expect(users).toHaveTextContent("1 persona names it: steward.");
    expect(users).toHaveTextContent("Pinned by persona steward.");
    await user.click(within(dialog).getByRole("button", { name: "Delete" }));

    const notice = await screen.findByRole("status", { name: /Skill deleted/ });
    expect(notice).toHaveTextContent("deploy is deleted. It stays in Deleted for 30 days.");
    // The Deleted page says how long the deleted skill can be brought back (SK-33, S9).
    await user.click(await screen.findByRole("button", { name: /^Deleted/ }));
    expect(await screen.findByTestId("deleted-deploy")).toHaveTextContent(
      "30 days left to bring it back",
    );
    await user.click(within(notice).getByRole("button", { name: "Bring it back" }));
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
    await user.click(screen.getAllByRole("button", { name: "Turn on in ops" })[0]);

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
      {
        pins_refused: refused,
        // The core's item of Needs you for it (#3689).
        needs: [
          need("pins:workspace ops", {
            kind: "pins",
            title: "workspace ops's pins",
            tag: refused.file,
            reason: `A chat reads none of workspace ops's pins: ${refused.why}, so a router may hide what they pin.`,
            fix: { kind: "unpin", by: { at: "workspace", name: "ops" }, name: "Not A Name" },
            fix_label: "Remove that pin",
          }),
        ],
      },
    );
    const user = userEvent.setup();
    renderStudio(inWorkspace("ops"), "needs");

    expect(await screen.findByTestId("skill-need")).toHaveTextContent(
      "A chat reads none of workspace ops's pins",
    );
    await user.click(screen.getByRole("button", { name: "Library" }));
    const item = await openSkill("skill-deploy", user);
    const pin = within(item).getByRole("switch", { name: "Pin deploy for workspace ops" });
    expect(pin).toBeChecked();
    expect(within(item).getByText("Pinned, not used")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^Needs you/ }));
    await user.click(await screen.findByRole("button", { name: "Remove that pin" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_pin")?.args).toMatchObject({
        by: { at: "workspace", name: "ops" },
        name: "Not A Name",
        on: false,
      }),
    );
  });

  it("sends a persona whose pins are too many to be used to the Library to unpin some (SK-15d)", async () => {
    core(
      [deploy],
      null,
      {},
      {
        needs: [
          need("pins:persona reviewer", {
            kind: "pins",
            title: "persona reviewer's pins",
            fix: { kind: "library" },
            fix_label: "Unpin some in the Library",
          }),
        ],
      },
    );
    const user = userEvent.setup();
    renderStudio(asPersona("reviewer"), "needs");

    await user.click(await screen.findByRole("button", { name: "Unpin some in the Library" }));
    expect(await screen.findByTestId("skill-deploy")).toBeInTheDocument();
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
    await user.click(screen.getByRole("button", { name: "Take lint out of ops" }));
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
        pinned_by: [],
      }),
    });
    const user = userEvent.setup();
    renderStudio(asPersona("reviewer"));

    const row = await openSkill("skill-lint", user);
    // Its own skill needs no name in its skills.
    expect(within(row).queryByRole("button", { name: "Add lint to reviewer" })).toBeNull();
    // It moves to the project's levels and this persona's, never another persona's.
    await chooseMore("lint", /^Move to another level/, user);
    let dialog = await screen.findByRole("dialog", { name: "Move lint to another level" });
    expect(within(dialog).getByRole("radio", { name: /^project/ })).toBeInTheDocument();
    expect(within(dialog).queryByRole("radio", { name: /steward/ })).toBeNull();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));

    await chooseMore("lint", /^Rename/, user);
    dialog = await screen.findByRole("dialog", { name: "Rename lint" });
    const box = within(dialog).getByLabelText("New name");
    await user.clear(box);
    await user.type(box, "check");
    await user.click(within(dialog).getByRole("button", { name: "Rename" }));
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
    dialog = await screen.findByRole("dialog", { name: "Move lint to another level" });
    await user.click(within(dialog).getByRole("radio", { name: /^project/ }));
    expect(await within(dialog).findByTestId("persona-keeps")).toHaveTextContent(
      "reviewer keeps it in every workspace",
    );
    // Its persona keeps it, so its chats are no change: the project's root starts getting it.
    await waitFor(() =>
      expect(within(dialog).getByRole("region", { name: "What changes" })).toHaveTextContent(
        "Chats at the project's root start getting it.",
      ),
    );
    await user.click(within(dialog).getByRole("button", { name: "Move" }));
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
    await waitFor(async () =>
      expect(await screen.findByTestId("skill-users")).toHaveTextContent(
        "Chats as reviewer, in every workspace.",
      ),
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
