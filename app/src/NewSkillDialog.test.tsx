// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **New skill… asks how, then opens the new skill** (#3684, spec #3679 S4): the studio's top bar
 * opens a dialog with three ways; Write it yourself checks the name as it is typed, makes the
 * skill at the level picked, and opens its page with the Text tab in Edit; Ask a chat to draft it
 * asks the window for a chat with the prompt typed and not sent; Install one from a source goes
 * to Sources. Drawn at the window seam, the core faked at its command boundary.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { StudioSkill } from "./bindings";
import { NAME_RESERVED } from "./SkillCopy";
import { forgetSkillEdit } from "./skillEdit";
import { STUDIO_HISTORY, forgetStudio, renderStudio } from "./SkillStudioTab.testkit";
import { SKILL_CHAT, type SkillChatAsk } from "./skillDraftAsks";
import { AT_THE_ROOT, inWorkspace } from "./skillStudio";
import { skill, studio } from "./skillsListing.testkit";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetSkillEdit(STUDIO_HISTORY);
  forgetStudio();
});

const DEPLOY = skill("deploy");
const SECRETS = skill("secrets", { level: "built-in", label: "built-in", folder: "/app/secrets" });
const TRIAGE_TEXT = "---\nname: triage\ndescription: Sorts issues.\n---\n\n# triage\n";

/** The levels of a project with the workspace `ops`. */
const LEVELS = [
  { level: { at: "project" as const, local: false }, label: "project" },
  { level: { at: "workspace" as const, name: "ops", local: false }, label: "workspace ops" },
];

/** A core whose studio lists `skills` and, once `skill_new` was asked, the skill it made too;
 *  `more` answers by command; every ask is recorded. */
function core(
  skills: StudioSkill[],
  more: Record<string, (args: Record<string, unknown>) => unknown> = {},
) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  let made: StudioSkill | undefined;
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd in more) return more[cmd](given);
    switch (cmd) {
      case "skill_studio":
        return studio(made === undefined ? skills : [made, ...skills], {
          workspace: (given.workspace as string | null) ?? null,
          levels: LEVELS,
        });
      case "skill_new": {
        const level = given.level as { at: "project" | "workspace" | "persona" };
        made = skill(given.name as string, {
          level: level.at,
          label: level.at,
          description: given.description as string,
        });
        return { notes: [] };
      }
      case "skill_templates":
        return [
          { id: { kind: "blank" }, label: "Blank", about: "Empty." },
          { id: { kind: "steps" }, label: "Steps to follow", about: "Steps." },
        ];
      case "skill_text":
        return TRIAGE_TEXT;
      case "skill_lint":
        return [];
      case "skill_stats":
        return { by_harness: [], by_persona: [], by_workspace: [] };
      case "skill_details":
        return { copies: [], uses: [], used_by: [], uses_cut: false, pinned_by: [] };
      case "skill_updates":
        return { offers: [], unconfirmed: [], unasked: [] };
      default:
        return [];
    }
  });
  return asked;
}

/** New skill…, pressed in the top bar: the dialog asking how. */
async function newSkill(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole("button", { name: "New skill…" }));
  return screen.findByRole("dialog", { name: "New skill" });
}

describe("New skill…", () => {
  it("asks how first, with three ways", async () => {
    core([DEPLOY]);
    const user = userEvent.setup();
    renderStudio();

    const dialog = await newSkill(user);
    const ways = within(dialog).getByRole("list", { name: "Ways to make a skill" });
    expect(
      within(ways)
        .getAllByRole("button")
        .map((one) => one.textContent),
    ).toEqual(["Write it yourself", "Ask a chat to draft it", "Install one from a source"]);
  });

  it("checks the name as it is typed: its shape, a built-in's, one its level holds", async () => {
    core([DEPLOY, SECRETS]);
    const user = userEvent.setup();
    renderStudio();

    await user.click(
      within(await newSkill(user)).getByRole("button", { name: "Write it yourself" }),
    );
    const form = await screen.findByRole("dialog", { name: "Write a new skill" });
    const name = within(form).getByLabelText("Name");
    const make = within(form).getByRole("button", { name: "Make it and open its page" });
    await user.type(
      within(form).getByLabelText("What it does, and when a chat should use it"),
      "Sorts issues.",
    );

    await user.type(name, "Triage");
    expect(within(form).getByTestId("skill-new-problem")).toHaveTextContent(
      "Use lowercase letters, digits and hyphens",
    );
    expect(make).toBeDisabled();
    await user.clear(name);
    await user.type(name, "secrets");
    expect(within(form).getByTestId("skill-new-problem")).toHaveTextContent(NAME_RESERVED);
    await user.clear(name);
    await user.type(name, "deploy");
    expect(within(form).getByTestId("skill-new-problem")).toHaveTextContent(
      "There is already a skill named deploy here.",
    );
    // At another level the name is free.
    await user.click(within(form).getByRole("radio", { name: /^workspace ops/ }));
    expect(within(form).queryByTestId("skill-new-problem")).toBeNull();
    expect(make).toBeEnabled();
  });

  it("makes it at the level picked, and opens its page with the Text tab in Edit", async () => {
    const asked = core([DEPLOY]);
    const user = userEvent.setup();
    renderStudio();

    await user.click(
      within(await newSkill(user)).getByRole("button", { name: "Write it yourself" }),
    );
    const form = await screen.findByRole("dialog", { name: "Write a new skill" });
    // It starts where the person came from: the project's skills at the root.
    expect(within(form).getByRole("radio", { name: /^project/ })).toBeChecked();
    await user.click(within(form).getByRole("radio", { name: /^workspace ops/ }));
    await user.type(within(form).getByLabelText("Name"), "triage");
    await user.type(
      within(form).getByLabelText("What it does, and when a chat should use it"),
      "Sorts issues.",
    );
    await user.click(within(form).getByRole("button", { name: "Make it and open its page" }));

    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_new")?.args).toMatchObject({
        level: { at: "workspace", name: "ops", local: false },
        name: "triage",
        description: "Sorts issues.",
        template: { kind: "blank" },
      }),
    );
    expect(
      await screen.findByRole("textbox", { name: "triage's instructions" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Text/ })).toHaveAttribute("aria-selected", "true");
    expect(screen.queryByRole("dialog", { name: "Write a new skill" })).toBeNull();
    // It went to the workspace's Library, where its skills are listed.
    expect(screen.getByLabelText("Whose chats")).toHaveTextContent("A chat in ops");
    expect(
      screen.getByText("triage is made in the workspace ops skills. Its text is open to edit."),
    ).toBeInTheDocument();
  });

  it("says the core's refusal in the dialog, which stays open", async () => {
    core([DEPLOY], {
      skill_new: () => {
        throw "the project's settings are not writable";
      },
    });
    const user = userEvent.setup();
    renderStudio();

    await user.click(
      within(await newSkill(user)).getByRole("button", { name: "Write it yourself" }),
    );
    const form = await screen.findByRole("dialog", { name: "Write a new skill" });
    await user.type(within(form).getByLabelText("Name"), "triage");
    await user.type(
      within(form).getByLabelText("What it does, and when a chat should use it"),
      "Sorts issues.",
    );
    await user.click(within(form).getByRole("button", { name: "Make it and open its page" }));
    expect(await within(form).findByTestId("skill-new-refused")).toHaveTextContent(
      "the project's settings are not writable",
    );
  });

  it("asks the window for a chat to draft it, with what it should do typed and not sent", async () => {
    core([DEPLOY]);
    const asks: SkillChatAsk[] = [];
    const heard = (event: Event) => asks.push((event as CustomEvent<SkillChatAsk>).detail);
    window.addEventListener(SKILL_CHAT, heard);
    const user = userEvent.setup();
    renderStudio(inWorkspace("ops"));

    const dialog = await newSkill(user);
    await user.click(within(dialog).getByRole("button", { name: "Ask a chat to draft it" }));
    const form = await screen.findByRole("dialog", { name: "Ask a chat to draft a skill" });
    const open = within(form).getByRole("button", { name: "Open the chat" });
    expect(open).toBeDisabled();
    await user.type(within(form).getByLabelText("What should it do?"), "Sorts issues.");
    await user.click(open);
    window.removeEventListener(SKILL_CHAT, heard);

    expect(asks).toEqual([
      {
        plane: "p",
        kind: "draft",
        level: { at: "workspace", name: "ops", local: false },
        what: "Sorts issues.",
      },
    ]);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("goes to Sources for Install one from a source", async () => {
    core([DEPLOY], {
      skill_updates: () => ({ offers: [], unconfirmed: [], unasked: [] }),
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT);

    const dialog = await newSkill(user);
    await user.click(within(dialog).getByRole("button", { name: "Install one from a source" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Sources" })).toHaveAttribute(
        "aria-current",
        "page",
      ),
    );
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
