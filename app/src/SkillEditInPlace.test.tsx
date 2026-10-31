// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Editing a skill in place** (#3683, spec #3679, the amendment's S4): Edit turns the Text tab
 * into the editor, checked as the person types, with Save and Discard and an "Unsaved changes"
 * mark; leaving with unsaved changes (a row, a page, Stop editing) asks Keep editing, Discard
 * and leave, or Save and leave, and the unsaved text is the tab's unsaved work in the window's
 * registry, which every close asks through (#3929). Drawn at the window seam, the core faked at its
 * command boundary.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { PLANE, STUDIO_HISTORY, forgetStudio, renderStudioAt } from "./SkillStudioTab.testkit";
import { SkillStudioTab } from "./SkillStudioTab";
import { AT_THE_ROOT, SKILL_STUDIO_VIEW, skillRowId, type StudioPlace } from "./skillStudio";
import { skill, studio } from "./skillsListing.testkit";
import { forgetSkillEdit, keepSkillEdit, skillEditIn } from "./skillEdit";
import { typeIn } from "./editor/typing.testkit";
import { handedOver, takeHanded, unsavedIn, unsavedNow } from "./unsavedWork";
import type { SkillLintFinding, StudioSkill } from "./bindings";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetSkillEdit(STUDIO_HISTORY);
  forgetStudio();
});

const DEPLOY = skill("deploy", { description: "Deploys the site." });
const REVIEW = skill("review", { description: "Reviews a pull request." });
const TEXT = "---\nname: deploy\ndescription: Deploys the site.\n---\n\nShip it.\n";

/** A core answering the studio with deploy and review, `lint` for every lint asked, `more` by
 *  command, and recording each ask. */
function core(
  lint: () => SkillLintFinding[] = () => [],
  more: Record<string, (args: Record<string, unknown>) => unknown> = {},
) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd in more) return more[cmd](given);
    switch (cmd) {
      case "skill_studio":
        return studio([DEPLOY, REVIEW]);
      case "skill_text":
        return TEXT;
      case "skill_lint":
        return lint();
      case "skill_save":
        return { notes: [] };
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

function at(one: StudioSkill, more: Partial<StudioPlace> = {}): StudioPlace {
  return { page: "library", scope: AT_THE_ROOT, skill: skillRowId(one), ...more };
}

/** The editor's text, once the editor is open. */
const editorText = () => screen.findByRole("textbox", { name: "deploy's instructions" });

/** Opens deploy's page, presses Edit, and answers the editor's text. */
async function startEditing(user: ReturnType<typeof userEvent.setup>) {
  renderStudioAt(at(DEPLOY));
  await user.click(await screen.findByRole("button", { name: "Edit deploy" }));
  return editorText();
}

/** The question a leave asks, once it is up. */
const question = () => screen.findByRole("alertdialog", { name: "Save your changes to deploy?" });

describe("Edit, in place", () => {
  it("turns the Text tab into the editor, and Stop editing back", async () => {
    core();
    const user = userEvent.setup();
    await startEditing(user);

    expect(screen.getByRole("tab", { name: /Text/ })).toHaveAttribute("aria-selected", "true");
    expect(await screen.findByText("It is a whole skill.")).toBeInTheDocument();
    // Nothing typed: nothing to save or discard, and nothing marked.
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Discard" })).toBeDisabled();
    expect(screen.queryByTestId("skill-unsaved")).toBeNull();

    // Stop editing with nothing changed asks nothing.
    await user.click(screen.getByRole("button", { name: "Stop editing deploy" }));
    expect(await screen.findByTestId("skill-text")).toBeInTheDocument();
    expect(screen.queryByTestId("skill-editor")).toBeNull();
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(screen.getByRole("button", { name: "Edit deploy" })).toBeInTheDocument();
  });

  it("marks unsaved changes, and Discard drops them", async () => {
    core();
    const user = userEvent.setup();
    typeIn(await startEditing(user), "More.\n");

    expect(screen.getByTestId("skill-unsaved")).toHaveTextContent("Unsaved changes");
    await user.click(screen.getByRole("button", { name: "Discard" }));
    expect(screen.queryByTestId("skill-unsaved")).toBeNull();
    expect(await screen.findByTestId("skill-text")).toBeInTheDocument();
    expect(skillEditIn(STUDIO_HISTORY)).toBeUndefined();
  });

  it("lints as the person types, and Save waits while a finding blocks", async () => {
    let lint: SkillLintFinding[] = [];
    core(() => lint);
    const user = userEvent.setup();
    const text = await startEditing(user);

    lint = [{ level: "blocks", says: "its frontmatter names no description:" }];
    typeIn(text, "x");
    expect(await screen.findByText("its frontmatter names no description:")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();

    lint = [{ level: "warns", says: "its ! command calls jq, which this machine does not have" }];
    typeIn(text, "y");
    expect(
      await screen.findByText(
        "Worth knowing: its ! command calls jq, which this machine does not have",
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
  });

  it("saves what was typed and says chats started from now get it", async () => {
    const asked = core();
    const user = userEvent.setup();
    typeIn(await startEditing(user), "More.\n");

    await waitFor(() => expect(screen.getByRole("button", { name: "Save" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_save")?.args).toMatchObject({
        name: "deploy",
        was: TEXT,
        now: `${TEXT}More.\n`,
      }),
    );
    expect(
      await screen.findByText("Saved. Chats started from now get this version."),
    ).toBeInTheDocument();
    expect(screen.queryByTestId("skill-editor")).toBeNull();
  });

  it("keeps the editor and says why when the save is refused", async () => {
    core(undefined, {
      skill_save: () => {
        throw "deploy changed on disk since it was opened";
      },
    });
    const user = userEvent.setup();
    typeIn(await startEditing(user), "More.\n");

    await waitFor(() => expect(screen.getByRole("button", { name: "Save" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByText(/changed on disk/)).toBeInTheDocument();
    expect(screen.getByTestId("skill-unsaved")).toBeInTheDocument();
  });

  it("keeps what was typed while its tab is away from the front", async () => {
    core();
    const user = userEvent.setup();
    typeIn(await startEditing(user), "More.\n");

    // The tab goes behind another, and comes back: the studio is drawn afresh.
    cleanup();
    renderStudioAt(at(DEPLOY, { tab: "text" }));
    expect((await editorText()).textContent).toContain("More.");
    expect(screen.getByTestId("skill-unsaved")).toBeInTheDocument();
    // Never saved on the word of a lint not yet asked of the text drawn afresh.
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    await waitFor(() => expect(screen.getByRole("button", { name: "Save" })).toBeEnabled());
  });
});

describe("leaving with unsaved changes", () => {
  it("asks before another skill is chosen: Keep editing stays", async () => {
    core();
    const user = userEvent.setup();
    typeIn(await startEditing(user), "More.\n");

    await user.click(screen.getByTestId("skill-review"));
    const asked = await question();
    expect(asked).toHaveTextContent("If you leave now, the changes are lost.");
    await user.click(within(asked).getByRole("button", { name: "Keep editing" }));

    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(screen.getByTestId("skill-studio-skill")).toHaveAttribute(
      "data-row",
      skillRowId(DEPLOY),
    );
    expect(screen.getByTestId("skill-unsaved")).toBeInTheDocument();
  });

  it("Discard and leave goes on, and saves nothing", async () => {
    const asked = core();
    const user = userEvent.setup();
    typeIn(await startEditing(user), "More.\n");

    await user.click(screen.getByTestId("skill-review"));
    await user.click(within(await question()).getByRole("button", { name: "Discard and leave" }));

    await waitFor(() =>
      expect(screen.getByTestId("skill-studio-skill")).toHaveAttribute(
        "data-row",
        skillRowId(REVIEW),
      ),
    );
    expect(asked.some((one) => one.cmd === "skill_save")).toBe(false);
    expect(skillEditIn(STUDIO_HISTORY)).toBeUndefined();
  });

  it("Save and leave saves, then goes on", async () => {
    const asked = core();
    const user = userEvent.setup();
    typeIn(await startEditing(user), "More.\n");

    await user.click(screen.getByRole("button", { name: "Sources" }));
    const saveAndLeave = within(await question()).getByRole("button", { name: "Save and leave" });
    await waitFor(() => expect(saveAndLeave).toBeEnabled());
    await user.click(saveAndLeave);

    await waitFor(() => expect(asked.some((one) => one.cmd === "skill_save")).toBe(true));
    await waitFor(() => expect(screen.queryByTestId("skill-studio-skill")).toBeNull());
    expect(
      await screen.findByText("Saved. Chats started from now get this version."),
    ).toBeInTheDocument();
  });

  it("asks on Stop editing", async () => {
    core();
    const user = userEvent.setup();
    typeIn(await startEditing(user), "More.\n");

    await user.click(screen.getByRole("button", { name: "Stop editing deploy" }));
    await user.click(within(await question()).getByRole("button", { name: "Discard and leave" }));
    expect(await screen.findByTestId("skill-text")).toBeInTheDocument();
  });

  it("offers no Save and leave while a finding blocks the save", async () => {
    let lint: SkillLintFinding[] = [];
    core(() => lint);
    const user = userEvent.setup();
    const text = await startEditing(user);
    lint = [{ level: "blocks", says: "its frontmatter names no description:" }];
    typeIn(text, "x");
    await screen.findByText("its frontmatter names no description:");

    await user.click(screen.getByTestId("skill-review"));
    const asked = await question();
    expect(asked).toHaveTextContent("it can't be saved as it is");
    expect(within(asked).getByRole("button", { name: "Save and leave" })).toBeDisabled();
  });
});

describe("the editor's unsaved text, as the window's unsaved work (#3929)", () => {
  it("is registered while it is unsaved, by the studio's view, and goes once it is not", async () => {
    core();
    const user = userEvent.setup();
    const text = await startEditing(user);
    // Opened, nothing typed: nothing a close would lose.
    expect(unsavedNow()).toEqual([]);

    typeIn(text, "More.\n");
    expect(unsavedIn(PLANE)).toEqual([
      expect.objectContaining({
        key: STUDIO_HISTORY,
        plane: PLANE,
        view: SKILL_STUDIO_VIEW,
        name: "deploy",
        workspace: undefined,
        persona: undefined,
      }),
    ]);

    await user.click(screen.getByRole("button", { name: "Discard" }));
    expect(unsavedNow()).toEqual([]);
  });

  it("saves through the registry as the studio's Save does, and the editor closes", async () => {
    const asked = core();
    const user = userEvent.setup();
    typeIn(await startEditing(user), "More.\n");

    const [work] = unsavedIn(PLANE);
    expect(await work.save()).toBeUndefined();
    expect(asked.filter((one) => one.cmd === "skill_save").map((one) => one.args)).toEqual([
      expect.objectContaining({ plane: PLANE, name: "deploy", was: TEXT, now: `${TEXT}More.\n` }),
    ]);
    expect(skillEditIn(STUDIO_HISTORY)).toBeUndefined();
    expect(unsavedNow()).toEqual([]);
  });

  it("says why a save through the registry was refused, and keeps the text", async () => {
    core(undefined, {
      skill_save: () => {
        throw "nothing was saved: its frontmatter names no description:";
      },
    });
    const user = userEvent.setup();
    typeIn(await startEditing(user), "More.\n");

    const [work] = unsavedIn(PLANE);
    expect(await work.save()).toBe("nothing was saved: its frontmatter names no description:");
    expect(skillEditIn(STUDIO_HISTORY)?.now).toBe(`${TEXT}More.\n`);
    expect(unsavedIn(PLANE)).toHaveLength(1);
  });

  it("is a workspace's where the skill is saved into one, so its removal asks too", () => {
    keepSkillEdit(STUDIO_HISTORY, {
      plane: PLANE,
      view: SKILL_STUDIO_VIEW,
      at: at(DEPLOY, { tab: "text" }),
      skill: DEPLOY,
      level: { at: "workspace", name: "ops", local: false },
      was: TEXT,
      now: `${TEXT}More.\n`,
    });
    expect(unsavedIn(PLANE, { workspace: "ops" }).map((one) => one.name)).toEqual(["deploy"]);
    unsavedIn(PLANE)[0].discard();
    expect(skillEditIn(STUDIO_HISTORY)).toBeUndefined();
  });

  it("is carried to another window as it is, whose studio opens on it (#4028)", async () => {
    core();
    const user = userEvent.setup();
    typeIn(await startEditing(user), "More.\n");
    const edit = skillEditIn(STUDIO_HISTORY);
    // Carried out of this window, and let go of here with its tab, as a move does.
    const carried = handedOver(PLANE);
    cleanup();
    forgetSkillEdit(STUDIO_HISTORY);
    forgetStudio();
    expect(unsavedNow()).toEqual([]);

    // Sent between windows as JSON, to a window whose studio tab has no history yet.
    takeHanded(JSON.parse(JSON.stringify(carried)) as typeof carried);
    expect(skillEditIn(STUDIO_HISTORY)).toEqual(edit);
    expect(unsavedIn(PLANE).map((one) => one.key)).toEqual([STUDIO_HISTORY]);
    render(<SkillStudioTab plane={PLANE} changed={0} />);

    // On the text, not on the library away from it.
    expect((await editorText()).textContent).toContain("More.");
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("is a persona's where the skill is saved into one, so its removal asks too (#4029)", () => {
    keepSkillEdit(STUDIO_HISTORY, {
      plane: PLANE,
      view: SKILL_STUDIO_VIEW,
      at: at(DEPLOY, { tab: "text" }),
      skill: DEPLOY,
      level: { at: "persona", name: "steward", local: false },
      was: TEXT,
      now: `${TEXT}More.\n`,
    });
    expect(unsavedIn(PLANE, { persona: "steward" }).map((one) => one.name)).toEqual(["deploy"]);
    expect(unsavedIn(PLANE, { workspace: "steward" })).toEqual([]);
    unsavedIn(PLANE)[0].discard();
    expect(skillEditIn(STUDIO_HISTORY)).toBeUndefined();
  });
});
