// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Needs you** (#3689, spec #3679, the amendment's S9): what needs a decision, grouped by kind,
 * the item chosen beside the list, each fix doing what it says. Drawn at the window seam, the
 * core faked at its command boundary.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { forgetStudio, renderStudio } from "./SkillStudioTab.testkit";
import { AT_THE_ROOT, inWorkspace } from "./skillStudio";
import { need, skill, studio } from "./skillsListing.testkit";
import type { NeedsYou, Studio } from "./bindings";
import { OPEN_PROPOSAL_ASK } from "./proposedChangeOpen";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetStudio();
});

const DEPLOY = skill("deploy", { description: "Deploys." });

const NEEDS: NeedsYou[] = [
  need("grilling"),
  need("clash:Project:deploy", {
    kind: "clash",
    title: "deploy",
    reason: "The repo svc has a skill of this name too, and a chat there gets both.",
    fix: { kind: "rename", name: "deploy", level: "project", local: false },
    fix_label: "Rename it…",
    fix_does: "Opens its page with Rename, so only one copy keeps the name.",
  }),
  need("tidy-up:unmoved", {
    kind: "tidy-up",
    title: "2 skills only some programs read",
    tag: null,
    reason: "This project keeps 2 skills where only some chat programs read them.",
    fix: { kind: "move-skills", fix: "project-skills" },
    fix_label: "Move them",
    fix_does: "One commit, which can be undone.",
  }),
  need("tidy-up:old-plugin-keys", {
    kind: "tidy-up",
    title: "Old plugin keys",
    tag: "charter.toml",
    reason: "purlis no longer turns other harnesses' plugins on or off per project.",
    fix: { kind: "remove-old-keys" },
    fix_label: "Remove them…",
    fix_does: "Proposes each file without them, for you to apply.",
  }),
];

/** A core whose studio answers `answer` and each other command from `more`, recording asks. */
function core(
  answer: Studio,
  more: Record<string, (args: Record<string, unknown>) => unknown> = {},
) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "skill_studio") return answer;
    const one = more[cmd];
    if (one !== undefined) return one(given);
    if (cmd === "skill_updates") return { offers: [], unconfirmed: [], unasked: [] };
    if (cmd === "skill_drafts" || cmd === "skills_deleted") return [];
    return { notes: [] };
  });
  return asked;
}

describe("Needs you", () => {
  it("groups what needs a decision by kind, each with its count, the first item beside it", async () => {
    core(studio([DEPLOY], { needs: NEEDS }));
    renderStudio(AT_THE_ROOT, "needs");

    const list = await screen.findByRole("grid", { name: "What needs you" });
    const groups = within(list)
      .getAllByRole("rowgroup")
      .map((one) => one.getAttribute("aria-label"));
    expect(groups).toEqual(["Changes to review", "Clashes", "Tidy up"]);
    expect(screen.getByRole("button", { name: /^Needs you/ })).toHaveTextContent("4");
    const item = screen.getByTestId("skill-need");
    expect(item).toHaveAttribute("data-need", "grilling");
    // Nothing is broken: a decision, in the attention tone, said to a reader as one.
    expect(within(item).getByText("Needs a decision:")).toBeInTheDocument();
    expect(item.querySelector('[data-cause="skill-need:grilling"]')).not.toBeNull();
  });

  it("opens the item chosen beside the list", async () => {
    core(studio([DEPLOY], { needs: NEEDS }));
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "needs");

    await user.click(await screen.findByTestId("need-tidy-up:unmoved"));

    const item = await screen.findByTestId("skill-need");
    await waitFor(() => expect(item).toHaveAttribute("data-need", "tidy-up:unmoved"));
    expect(item).toHaveTextContent("One commit, which can be undone.");
  });

  it("accepts a change to review, as one commit", async () => {
    const asked = core(studio([DEPLOY], { needs: [need("grilling")] }));
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "needs");

    await user.click(await screen.findByRole("button", { name: "Accept it" }));

    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_level_accept")?.args).toEqual({
        plane: "p",
        folder: "skills/grilling",
      }),
    );
  });

  it("accepts the project's folders it lists, as one item", async () => {
    const asked = core(
      studio([DEPLOY], {
        needs: [
          need("review:project-folders", {
            title: "The project's folders",
            tag: null,
            reason:
              "Purlis can't confirm which of this project's folders you accepted before. It accepts workspaces/ops, personas/steward.",
            fix: { kind: "accept-folders", folders: ["workspaces/ops", "personas/steward"] },
            fix_label: "Accept the folders",
            fix_does: "Their skills reach chats from then on.",
          }),
        ],
      }),
    );
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "needs");

    await user.click(await screen.findByRole("button", { name: "Accept the folders" }));

    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_levels_confirm")?.args).toEqual({
        plane: "p",
        folders: ["workspaces/ops", "personas/steward"],
      }),
    );
  });

  it("renames a clash's copy from its page, with Rename open", async () => {
    core(studio([DEPLOY], { needs: [NEEDS[1]] }));
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "needs");

    await user.click(await screen.findByRole("button", { name: "Rename it…" }));

    expect(await screen.findByTestId("skill-studio-doing")).toBeInTheDocument();
    const path = within(screen.getByRole("list", { name: "Path" })).getAllByRole("listitem");
    expect(path[0]).toHaveTextContent("Library");
    // It lands on the skill's own page (#3682), with Rename open under its header.
    const page = screen.getByTestId("skill-studio-skill");
    expect(within(page).getByRole("tablist", { name: "deploy's sections" })).toBeInTheDocument();
    expect(within(page).getByRole("form", { name: "Rename deploy" })).toBeInTheDocument();
  });

  it("moves the skills only some programs read, and says what the move said", async () => {
    const asked = core(studio([DEPLOY], { needs: [NEEDS[2]] }), {
      plane_doctor_fix: () => ({
        fix: "project-skills",
        refused: null,
        said: ["✓ moved deploy and notify."],
        complete: true,
      }),
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "needs");

    await user.click(await screen.findByRole("button", { name: "Move them" }));

    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "plane_doctor_fix")?.args).toEqual({
        plane: "p",
        fix: "project-skills",
      }),
    );
    expect(await screen.findByText("✓ moved deploy and notify.")).toBeInTheDocument();
  });

  it("says why the move was refused", async () => {
    core(studio([DEPLOY], { needs: [NEEDS[2]] }), {
      plane_doctor_fix: () => ({
        fix: "project-skills",
        refused: "the project has changes that are not committed",
        said: [],
        complete: false,
      }),
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "needs");

    await user.click(await screen.findByRole("button", { name: "Move them" }));

    expect(
      await screen.findByText("the project has changes that are not committed"),
    ).toBeInTheDocument();
  });

  it("proposes the files without the old plugin keys, and opens the first change", async () => {
    core(studio([DEPLOY], { needs: [NEEDS[3]] }), {
      remove_old_harness_plugins: () => [{ id: 7, file: "charter.toml" }],
    });
    const opened: unknown[] = [];
    const heard = (event: Event) => opened.push((event as CustomEvent).detail);
    window.addEventListener(OPEN_PROPOSAL_ASK, heard);
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "needs");

    await user.click(await screen.findByRole("button", { name: "Remove them…" }));

    await waitFor(() => expect(opened).toHaveLength(1));
    window.removeEventListener(OPEN_PROPOSAL_ASK, heard);
    expect(opened[0]).toMatchObject({ plane: "p", id: 7, path: "charter.toml" });
  });

  it("leaves the old plugin keys as they are, said once", async () => {
    const asked = core(studio([DEPLOY], { needs: [NEEDS[3]] }), {
      dismiss_old_harness_plugins: () => null,
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "needs");

    await user.click(await screen.findByRole("button", { name: "Leave them" }));

    await waitFor(() =>
      expect(asked.some((one) => one.cmd === "dismiss_old_harness_plugins")).toBe(true),
    );
  });

  it("takes out the pin no chat reads", async () => {
    const by = { at: "workspace" as const, name: "ops" };
    const asked = core(
      studio([DEPLOY], {
        workspace: "ops",
        needs: [
          need("pins:workspace ops", {
            kind: "pins",
            title: "workspace ops's pins",
            fix: { kind: "unpin", by, name: "Not A Name" },
            fix_label: "Remove that pin",
          }),
        ],
      }),
    );
    const user = userEvent.setup();
    renderStudio(inWorkspace("ops"), "needs");

    await user.click(await screen.findByRole("button", { name: "Remove that pin" }));

    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_pin")?.args).toEqual({
        plane: "p",
        by,
        name: "Not A Name",
        on: false,
      }),
    );
  });

  it("is page-and-back on a narrow tab", async () => {
    core(studio([DEPLOY], { needs: NEEDS }));
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "needs", { narrow: true });

    await user.click(await screen.findByTestId("need-grilling"));
    expect(await screen.findByTestId("skill-need")).toHaveAttribute("data-need", "grilling");
    expect(screen.queryByRole("grid", { name: "What needs you" })).toBeNull();
  });

  it("leaves the navigation, and lands on the Library, once nothing needs you", async () => {
    core(studio([DEPLOY]));
    renderStudio(AT_THE_ROOT, "needs");

    await screen.findByTestId("skill-deploy");
    expect(screen.queryByRole("button", { name: /^Needs you/ })).toBeNull();
  });
});
