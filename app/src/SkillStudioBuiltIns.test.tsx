// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Built-in skills in the Library, and Make my own copy** (#3686, spec #3679 S12): built-in
 * skills fold into one quiet line at the end of the Library, a Built-in chip lists them alone,
 * and one stands out of the fold when it needs the person, clashes or is searched for. Make my
 * own copy, from the folded line or a built-in's page, asks for a name that is not a built-in's
 * and a skill level, then opens the copy's page. Drawn at the window seam, the core faked at its
 * command boundary.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { forgetStudio, openSkill, renderStudio } from "./SkillStudioTab.testkit";
import { skill, studio } from "./skillsListing.testkit";
import { pickIn, selectNamed } from "./ui/select/select.testkit";
import type { StudioSkill } from "./bindings";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetStudio();
});

/** A built-in skill, as the studio answers one. */
const builtIn = (name: string, over: Partial<StudioSkill> = {}) =>
  skill(name, {
    level: "built-in",
    label: "built-in",
    grant: null,
    granted: null,
    folder: `/app/plugin/skills/${name}`,
    ...over,
  });

const OWN = [skill("deploy"), skill("review")];
const BUILT_IN = [builtIn("browser"), builtIn("handoff"), builtIn("secrets")];

const LEVELS = [
  { level: { at: "project" as const, local: false }, label: "project" },
  { level: { at: "workspace" as const, name: "ops", local: false }, label: "workspace ops" },
];

/** A core whose studio holds `skills` (the built-ins first, as the core lists them), and which
 *  makes a copy as `copied` says. */
function core(
  skills: StudioSkill[] = [...BUILT_IN, ...OWN],
  copied: (args: Record<string, unknown>) => unknown = () => ({ notes: [] }),
) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  let made: StudioSkill | undefined;
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "skill_studio")
      return studio(made === undefined ? skills : [...skills, made], { levels: LEVELS });
    if (cmd === "skill_copy") {
      const said = copied(given);
      if (typeof said === "string") throw said;
      made = skill(String(given.to), {
        level: "workspace",
        label: "workspace ops",
        folder: `workspaces/ops/skills/${String(given.to)}`,
      });
      return said;
    }
    if (cmd === "skill_updates") return { offers: [], unconfirmed: [], unasked: [] };
    return [];
  });
  return asked;
}

/** The Library's rows, by what a reader hears first. */
function rows(): string[] {
  const list = screen.getByRole("grid", { name: "Skills" });
  return within(list)
    .getAllByRole("row")
    .map((one) => one.textContent ?? "");
}

const quietLine = (count: number) =>
  within(screen.getByRole("grid", { name: "Skills" })).getByRole("row", {
    name: `${count} built-in skills`,
  });

describe("the Library's built-in skills", () => {
  it("fold into one quiet line after the project's own skills", async () => {
    core();
    renderStudio();
    await screen.findByTestId("skill-deploy");
    expect(screen.queryByTestId("skill-browser")).toBeNull();
    expect(screen.queryByTestId("skill-secrets")).toBeNull();
    const all = rows();
    expect(all).toHaveLength(3);
    expect(all[0]).toContain("deploy");
    expect(all[1]).toContain("review");
    expect(all[2]).toContain("3 built-in skills");
  });

  it("unfold in place from the quiet line, with what they are and Make my own copy", async () => {
    core();
    const user = userEvent.setup();
    renderStudio();
    await screen.findByTestId("skill-deploy");
    await user.click(quietLine(3));
    await screen.findByTestId("skill-browser");
    expect(screen.getByTestId("skill-handoff")).toBeInTheDocument();
    expect(
      screen.getByText("They ship with purlis: they can't be edited, turned off or replaced."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Make my own copy…" })).toBeInTheDocument();
    // The line folds them again, and choosing it chose no skill.
    expect(screen.getByTestId("skill-studio-unpicked")).toBeInTheDocument();
    await user.click(quietLine(3));
    await waitFor(() => expect(screen.queryByTestId("skill-browser")).toBeNull());
  });

  it("are listed alone, unfolded, under the Built-in chip", async () => {
    core();
    const user = userEvent.setup();
    renderStudio();
    await screen.findByTestId("skill-deploy");
    await user.click(screen.getByRole("checkbox", { name: "Built-in" }));
    await screen.findByTestId("skill-browser");
    expect(screen.getByTestId("skill-secrets")).toBeInTheDocument();
    expect(screen.queryByTestId("skill-deploy")).toBeNull();
    expect(rows().some((one) => one.includes("built-in skills"))).toBe(false);
  });

  it("stand out of the fold where one matches the search", async () => {
    core();
    const user = userEvent.setup();
    renderStudio();
    await screen.findByTestId("skill-deploy");
    await user.type(screen.getByRole("searchbox", { name: "Search skills" }), "secr");
    await screen.findByTestId("skill-secrets");
    expect(screen.queryByTestId("skill-browser")).toBeNull();
    expect(rows().some((one) => one.includes("built-in skills"))).toBe(false);
  });

  it("stand out of the fold where one needs the person or a skill of the project clashes", async () => {
    core([
      builtIn("browser", { lint: ["Its description is over 1024 characters."] }),
      builtIn("handoff"),
      builtIn("secrets"),
      builtIn("theme"),
      skill("secrets", {
        on: false,
        left_out: "a built-in skill has its name",
        folder: "skills/secrets",
      }),
      ...OWN,
    ]);
    renderStudio();
    await screen.findByTestId("skill-deploy");
    expect(screen.getByTestId("skill-browser")).toBeInTheDocument();
    expect(screen.getByTestId("skill-secrets")).toBeInTheDocument();
    expect(screen.queryByTestId("skill-handoff")).toBeNull();
    expect(quietLine(2)).toBeInTheDocument();
  });

  it("keep the one chosen in sight, beside its page", async () => {
    core();
    const user = userEvent.setup();
    renderStudio();
    await screen.findByTestId("skill-deploy");
    await user.click(quietLine(3));
    const page = await openSkill("skill-handoff", user);
    expect(within(page).getByRole("button", { name: "Make my own copy…" })).toBeInTheDocument();
    await user.click(quietLine(2));
    expect(screen.getByTestId("skill-handoff")).toBeInTheDocument();
    expect(screen.queryByTestId("skill-browser")).toBeNull();
  });
});

describe("Make my own copy", () => {
  it("copies the built-in on its page under the new name and level, then opens the copy", async () => {
    const asked = core();
    const user = userEvent.setup();
    renderStudio();
    await screen.findByTestId("skill-deploy");
    await user.click(quietLine(3));
    const page = await openSkill("skill-secrets", user);
    await user.click(within(page).getByRole("button", { name: "Make my own copy…" }));
    const dialog = await screen.findByRole("dialog", { name: "Make my own copy of secrets" });
    expect(dialog).toHaveTextContent(
      "secrets stays as it is, and chats keep getting it beside your copy.",
    );
    const name = within(dialog).getByRole("textbox", { name: "New name" });
    expect(name).toHaveValue("my-secrets");
    await user.clear(name);
    await user.type(name, "vault-steps");
    // The copy's own description starts as the built-in's, noted as the person's copy (#3777).
    const described = within(dialog).getByRole("textbox", {
      name: "What it does and when to use it",
    });
    expect(described).toHaveValue("What secrets does. (your copy of secrets)");
    await user.clear(described);
    await user.type(described, "Our vault steps, with the ops checklist.");
    await pickIn(selectNamed("Lands in", dialog), "1", user);
    await user.click(within(dialog).getByRole("button", { name: "Make the copy" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(asked.find((one) => one.cmd === "skill_copy")?.args).toMatchObject({
      name: "secrets",
      level: { at: "workspace", name: "ops", local: false },
      to: "vault-steps",
      description: "Our vault steps, with the ops checklist.",
    });
    const copy = await screen.findByTestId("skill-studio-skill");
    await waitFor(() =>
      expect(copy).toHaveAttribute("data-row", expect.stringContaining("#vault-steps")),
    );
    expect(screen.getByRole("status", { name: /Copy made/ })).toHaveTextContent(
      "vault-steps is your own copy of secrets. Chats still get secrets.",
    );
  });

  it("refuses a built-in skill's name as it is typed, in the core's words", async () => {
    core();
    const user = userEvent.setup();
    renderStudio();
    await screen.findByTestId("skill-deploy");
    await user.click(quietLine(3));
    await user.click(screen.getByRole("button", { name: "Make my own copy…" }));
    const dialog = await screen.findByRole("dialog", {
      name: "Make my own copy of a built-in skill",
    });
    const name = within(dialog).getByRole("textbox", { name: "New name" });
    await user.clear(name);
    await user.type(name, "handoff");
    expect(within(dialog).getByRole("alert")).toHaveTextContent(
      "That name belongs to a built-in skill; pick another, or make a copy.",
    );
    expect(within(dialog).getByRole("button", { name: "Make the copy" })).toBeDisabled();
  });

  it("refuses the built-in's description word for word, or none, as it is typed (#3777)", async () => {
    const asked = core();
    const user = userEvent.setup();
    renderStudio();
    await screen.findByTestId("skill-deploy");
    await user.click(quietLine(3));
    await user.click(screen.getByRole("button", { name: "Make my own copy…" }));
    const dialog = await screen.findByRole("dialog", {
      name: "Make my own copy of a built-in skill",
    });
    const described = within(dialog).getByRole("textbox", {
      name: "What it does and when to use it",
    });
    const make = within(dialog).getByRole("button", { name: "Make the copy" });
    await user.clear(described);
    await user.type(described, "What browser does.");
    expect(within(dialog).getByRole("alert")).toHaveTextContent(
      "the copy's description is browser's word for word, so a chat could not tell them apart",
    );
    expect(make).toBeDisabled();
    await user.clear(described);
    expect(within(dialog).queryByRole("alert")).toBeNull();
    expect(make).toBeDisabled();
    // Typed, it is kept as the person wrote it when another built-in is picked.
    await user.type(described, "Mine.");
    await pickIn(selectNamed("Copy of", dialog), "handoff", user);
    expect(described).toHaveValue("Mine.");
    expect(make).toBeEnabled();
    expect(asked.some((one) => one.cmd === "skill_copy")).toBe(false);
  });

  it("from the folded line, picks the built-in to copy, and says the core's refusal in place", async () => {
    const asked = core(undefined, () => "the project skills already have one named my-browser");
    const user = userEvent.setup();
    renderStudio();
    await screen.findByTestId("skill-deploy");
    await user.click(quietLine(3));
    await user.click(screen.getByRole("button", { name: "Make my own copy…" }));
    const dialog = await screen.findByRole("dialog", {
      name: "Make my own copy of a built-in skill",
    });
    await pickIn(selectNamed("Copy of", dialog), "browser", user);
    expect(within(dialog).getByRole("textbox", { name: "New name" })).toHaveValue("my-browser");
    // The proposed description follows the built-in picked, until the person types their own.
    expect(
      within(dialog).getByRole("textbox", { name: "What it does and when to use it" }),
    ).toHaveValue("What browser does. (your copy of browser)");
    await user.click(within(dialog).getByRole("button", { name: "Make the copy" }));
    await waitFor(() =>
      expect(within(dialog).getByRole("alert")).toHaveTextContent(
        "the project skills already have one named my-browser",
      ),
    );
    expect(asked.find((one) => one.cmd === "skill_copy")?.args).toMatchObject({
      name: "browser",
      level: { at: "project", local: false },
      to: "my-browser",
      description: "What browser does. (your copy of browser)",
    });
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });
});
