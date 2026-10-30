// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Skills view** (SK-55, #3266; spec #3679 S7, #3693), against a mocked core: whose skills
 * it shows (the chat in front's, or a new chat's in the title bar's workspace), its three groups
 * of cards and the built-in skills folded among them, where a card goes, each act of a card's
 * menu and its fix, its header, and its empty state. The sandbox draws no window: these check
 * what each surface says and does, in the DOM a reader reads.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ChatSkill, PersonaSkills, Studio, StudioSkill } from "./bindings";
import { SkillsView, type FrontChat } from "./SkillsView";
import { skill, studio } from "./skillsListing.testkit";
import {
  SKILL_STUDIO_TITLE,
  asPersona,
  inWorkspace,
  skillRowId,
  skillStudioAt,
  skillStudioView,
} from "./skillStudio";
import { forgetStudio, renderStudio } from "./SkillStudioTab.testkit";
import { PLANE_ROOT, pieceFileView } from "./pieceViews";
import { DEFAULT_SKILLS_VIEW, SKILLS_VIEW } from "./skillsViewPrefs";

afterEach(() => {
  cleanup();
  clearMocks();
  SKILLS_VIEW.forget();
});

const REVIEWER = {
  level: { at: "persona" as const, name: "reviewer", local: false },
  label: "persona reviewer",
};

/** The skills a chat in `ops` gets, and the reviewer persona's own. */
const OPS: StudioSkill[] = [
  skill("secrets", { level: "built-in", label: "built-in", folder: "/app/skills/secrets" }),
  skill("deploy"),
  skill("old-deploy", { on: false, turned_off: true, left_out: "ops turned it off." }),
  skill("ops-runbook", {
    level: "workspace",
    label: "workspace ops",
    folder: "workspaces/ops/skills/ops-runbook",
  }),
  skill("drafted", {
    on: false,
    accept: "workspaces/ops/skills",
    left_out: "A chat made its folder.",
  }),
];

type Asked = { cmd: string; args: Record<string, unknown> };

/** A core whose studio answers `of(workspace, persona)`, with every persona's own skills
 *  `personas` and the chat in front's set `chat`, recording every ask. */
function core(
  of: (workspace: string | null, persona: string | null) => Studio,
  more: { personas?: PersonaSkills[]; chat?: ChatSkill[]; repos?: string[] } = {},
): Asked[] {
  const asked: Asked[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "skill_studio")
      return of(given.workspace as string | null, given.persona as string | null);
    if (cmd === "skill_persona_skills") return more.personas ?? [];
    if (cmd === "chat_skills")
      return {
        on: true,
        known: true,
        skills: more.chat ?? [],
        budget: null,
        repos: more.repos ?? [],
      };
    if (cmd === "skills_deleted" || cmd === "skill_drafts" || cmd === "skill_sources") return [];
    if (cmd === "skill_sources_offered") return [];
    if (cmd === "skill_updates") return { offers: [], unconfirmed: [], unasked: [] };
    if (cmd === "skill_templates")
      return [{ id: { kind: "blank" }, label: "Blank", about: "Empty." }];
    if (cmd === "skill_deletes_cut_short") return { settled: [], unsettled: [] };
    return { notes: [] };
  });
  return asked;
}

/** The reviewer persona's own skill. */
const REVIEW = skill("review", { level: "persona", folder: "personas/reviewer/skills/review" });

/** A skill of a chat's set, as Use skill… reads it. */
const given = (name: string, level: ChatSkill["level"] = "project"): ChatSkill => ({
  name,
  level,
  label: level,
  description: `What ${name} does.`,
  manual: false,
  takes_arguments: false,
  arguments: [],
  hint: null,
  hidden: null,
});

/** The project as `ops` and the project's root see it, with the reviewer's own skill. */
function opsCore(chat: ChatSkill[] = []) {
  return core(
    (workspace) =>
      studio(workspace === "ops" ? OPS : [skill("deploy")], {
        levels: [{ level: { at: "project", local: false }, label: "project" }, REVIEWER],
      }),
    { personas: [{ persona: "reviewer", skills: [REVIEW] }], chat },
  );
}

function view(
  over: Partial<Parameters<typeof SkillsView>[0]> = {},
): Required<
  Pick<Parameters<typeof SkillsView>[0], "onOpenView" | "onUseSkill" | "onShowInExplorer">
> {
  const hands = {
    onOpenView: vi.fn(),
    onUseSkill: vi.fn(),
    onShowInExplorer: vi.fn(),
  };
  render(<SkillsView plane="p" workspace="ops" {...hands} {...over} />);
  return hands;
}

const list = () => screen.findByRole("grid", { name: "Skills" });
/** The card of skill `name`, by the words it is drawn with. */
const rowOf = (name: string): HTMLElement => {
  const found = [...document.querySelectorAll<HTMLElement>('[data-testid^="skills-row-"]')].find(
    (one) => one.querySelector('[data-part="label"]')?.textContent === name,
  );
  if (found === undefined) throw new Error(`no card of ${name}`);
  return found;
};
/** Whether a card of `name` is drawn. */
const hasRow = (name: string) => {
  try {
    rowOf(name);
    return true;
  } catch {
    return false;
  }
};
/** A row of the list that folds (a group's name, the built-in skills' quiet line), by its words. */
const foldOf = (name: string | RegExp) =>
  within(screen.getByRole("grid", { name: "Skills" })).getByRole("row", { name });
/** Whether a row that folds is open, as its cell says it. */
const openOf = (row: HTMLElement) =>
  within(row).getByRole("gridcell").getAttribute("aria-expanded");

const STEWARD: FrontChat = {
  session: 5,
  name: "steward 5",
  workspace: "ops",
  persona: "reviewer",
  harness: "claude",
};

describe("whose skills the Skills view shows", () => {
  it("is a new chat's in the title bar's workspace while no chat is in front, said in its header", async () => {
    const asked = opsCore();
    view();
    await list();
    await waitFor(() => expect(hasRow("deploy")).toBe(true));
    expect(screen.getByTestId("skills-view-whose")).toHaveTextContent(
      "A new chat in ops · 4 skills",
    );
    // A new chat runs as no persona: the studio is read for none, and no persona's own is listed.
    expect(asked.filter((one) => one.cmd === "skill_studio").map((one) => one.args)).toEqual([
      { plane: "p", workspace: "ops", persona: null },
    ]);
    expect(asked.some((one) => one.cmd === "chat_skills")).toBe(false);
    expect(hasRow("review")).toBe(false);
  });

  it("is a new chat's at the project's root there", async () => {
    opsCore();
    view({ workspace: undefined });
    await list();
    await waitFor(() =>
      expect(screen.getByTestId("skills-view-whose")).toHaveTextContent(
        "A new chat at the project's root · 1 skill",
      ),
    );
  });

  it("is the chat in front's set, read for its workspace and persona, under its name", async () => {
    const asked = opsCore([
      given("secrets", "built-in"),
      given("deploy"),
      given("review", "persona"),
    ]);
    view({ chat: STEWARD, workspace: undefined });
    await list();
    await waitFor(() => expect(hasRow("review")).toBe(true));
    expect(screen.getByTestId("skills-view-whose")).toHaveTextContent("steward 5 · 4 skills");
    expect(asked).toContainEqual({
      cmd: "skill_studio",
      args: { plane: "p", workspace: "ops", persona: "reviewer" },
    });
    expect(asked).toContainEqual({ cmd: "chat_skills", args: { plane: "p", session: 5 } });
    // What the chat started without is left out, with why, once Left out is unfolded.
    expect(hasRow("ops-runbook")).toBe(false);
    await userEvent.click(foldOf(/^Left out/));
    await waitFor(() => expect(hasRow("ops-runbook")).toBe(true));
    expect(rowOf("ops-runbook")).toHaveTextContent("The chat in front started without it.");
  });

  it("follows the chat in front as it changes, and has no Only this chat's", async () => {
    const asked = opsCore([given("deploy")]);
    const hands = {
      onOpenView: vi.fn(),
      onUseSkill: vi.fn(),
      onShowInExplorer: vi.fn(),
    };
    const { rerender } = render(<SkillsView plane="p" workspace="ops" {...hands} />);
    await list();
    expect(screen.queryByRole("button", { name: "Only this chat's" })).toBeNull();
    expect(screen.queryByRole("switch")).toBeNull();
    rerender(<SkillsView plane="p" workspace="ops" chat={STEWARD} {...hands} />);
    await waitFor(() =>
      expect(screen.getByTestId("skills-view-whose")).toHaveTextContent("steward 5"),
    );
    rerender(<SkillsView plane="p" workspace="ops" {...hands} />);
    await waitFor(() =>
      expect(screen.getByTestId("skills-view-whose")).toHaveTextContent("A new chat in ops"),
    );
    expect(asked.filter((one) => one.cmd === "chat_skills")).toHaveLength(1);
  });

  it("heads its middle group as the header says whose set it is (#3778)", async () => {
    opsCore([given("deploy")]);
    const hands = {
      onOpenView: vi.fn(),
      onUseSkill: vi.fn(),
      onShowInExplorer: vi.fn(),
    };
    const { rerender } = render(<SkillsView plane="p" workspace="ops" {...hands} />);
    await list();
    await waitFor(() => expect(foldOf(/^A new chat gets/)).toBeInTheDocument());
    expect(screen.queryByRole("row", { name: /^In this chat/ })).toBeNull();
    // Folded under one heading, the group stays folded under the other: it is one group.
    await userEvent.click(foldOf(/^A new chat gets/));
    await waitFor(() => expect(openOf(foldOf(/^A new chat gets/))).toBe("false"));
    rerender(<SkillsView plane="p" workspace="ops" chat={STEWARD} {...hands} />);
    await waitFor(() => expect(foldOf(/^In this chat/)).toBeInTheDocument());
    expect(openOf(foldOf(/^In this chat/))).toBe("false");
    expect(screen.queryByRole("row", { name: /^A new chat gets/ })).toBeNull();
    rerender(<SkillsView plane="p" workspace="ops" {...hands} />);
    await waitFor(() => expect(foldOf(/^A new chat gets/)).toBeInTheDocument());
    expect(openOf(foldOf(/^A new chat gets/))).toBe("false");
  });
});

describe("the Skills view's cards", () => {
  it("are grouped Needs you, A new chat gets with its built-in skills folded, then Left out folded", async () => {
    opsCore();
    view();
    await list();
    await waitFor(() => expect(hasRow("deploy")).toBe(true));
    const names = within(screen.getByRole("grid", { name: "Skills" }))
      .getAllByRole("row")
      .map((one) => one.querySelector('[data-part="label"], [data-part="title"]')?.textContent);
    expect(names).toEqual([
      "Needs you",
      "drafted",
      // No chat is in front: the middle group is what a new chat would get (#3778).
      "A new chat gets",
      "deploy",
      "ops-runbook",
      "1 built-in skill",
      "Left out",
    ]);
    expect(openOf(foldOf("Needs you, 1"))).toBe("true");
    expect(openOf(foldOf("A new chat gets, 3"))).toBe("true");
    expect(openOf(foldOf("Left out, 1"))).toBe("false");
    // Each card says what it does, or why it is as it is, in its two lines.
    expect(rowOf("deploy")).toHaveTextContent("What deploy does.");
    expect(rowOf("drafted")).toHaveTextContent("A chat made its folder.");
    expect(rowOf("deploy")).toHaveTextContent("project");
  });

  it("unfold the built-in skills in place, with Make my own copy, which opens the studio's own dialog", async () => {
    opsCore();
    const hands = view();
    await list();
    await userEvent.click(await waitFor(() => foldOf("1 built-in skill")));
    await waitFor(() => expect(hasRow("secrets")).toBe(true));
    expect(
      screen.getByText("They ship with purlis: they can't be edited, turned off or replaced."),
    ).toBeInTheDocument();
    expect(SKILLS_VIEW.value().folded).toEqual(["left-out"]);
    await userEvent.click(screen.getByRole("button", { name: "Make my own copy…" }));
    expect(hands.onOpenView).toHaveBeenLastCalledWith(
      skillStudioView(inWorkspace("ops")),
      SKILL_STUDIO_TITLE,
    );
    // The studio it opened opens on its Make my own copy: one copy of the dialog.
    renderStudio(inWorkspace("ops"));
    expect(
      await screen.findByRole("dialog", { name: "Make my own copy of a built-in skill" }),
    ).toBeInTheDocument();
    forgetStudio();
  });

  it("keep the folds as the person leaves them, and open every group while a filter is typed", async () => {
    core(() => studio(Array.from({ length: 12 }, (_, at) => skill(`s${at}`))));
    view();
    await list();
    await userEvent.click(await waitFor(() => foldOf(/^A new chat gets/)));
    await waitFor(() => expect(hasRow("s0")).toBe(false));
    expect(SKILLS_VIEW.value().folded).toEqual([...DEFAULT_SKILLS_VIEW.folded, "in-chat"]);
    await userEvent.type(screen.getByRole("searchbox", { name: "Filter skills" }), "s1");
    await waitFor(() => expect(hasRow("s1")).toBe(true));
    expect(hasRow("s2")).toBe(false);
  });

  it("open the skill in the studio at Library › it, for the view's scope, the chat keeping its pane", async () => {
    opsCore([given("deploy"), given("review", "persona")]);
    const hands = view({ chat: STEWARD });
    await list();
    await waitFor(() => expect(hasRow("review")).toBe(true));
    await userEvent.click(rowOf("deploy"));
    expect(hands.onOpenView).toHaveBeenLastCalledWith(
      skillStudioAt(inWorkspace("ops"), "library", { skill: skillRowId(skill("deploy")) }),
      SKILL_STUDIO_TITLE,
    );
    // A persona's own skill opens on that persona's chats.
    await userEvent.click(rowOf("review"));
    expect(hands.onOpenView).toHaveBeenLastCalledWith(
      skillStudioAt(asPersona("reviewer"), "library", { skill: skillRowId(REVIEW) }),
      SKILL_STUDIO_TITLE,
    );
    expect(hands.onUseSkill).not.toHaveBeenCalled();
  });

  it("type the skill into the chat in front with Use, not sent", async () => {
    opsCore([given("deploy")]);
    const hands = view({ chat: { ...STEWARD, persona: null } });
    await list();
    await waitFor(() => expect(hasRow("deploy")).toBe(true));
    await userEvent.click(
      within(rowOf("deploy")).getByRole("button", { name: "Use deploy in this chat" }),
    );
    expect(hands.onUseSkill).toHaveBeenCalledWith(5, "deploy");
    expect(hands.onOpenView).not.toHaveBeenCalled();
  });

  it("offer one fix: a skill the workspace turned off is turned on there", async () => {
    const asked = opsCore();
    view();
    await list();
    await userEvent.click(await waitFor(() => foldOf(/^Left out/)));
    await waitFor(() => expect(hasRow("old-deploy")).toBe(true));
    expect(rowOf("old-deploy")).toHaveTextContent("ops turned it off.");
    await userEvent.click(
      within(rowOf("old-deploy")).getByRole("button", { name: "Turn on in ops" }),
    );
    await waitFor(() =>
      expect(asked).toContainEqual({
        cmd: "skill_turn",
        args: { plane: "p", workspace: "ops", name: "old-deploy", on: true },
      }),
    );
  });

  it("offer a change waiting for review as a fix that opens it in the studio", async () => {
    opsCore();
    const hands = view();
    await list();
    await waitFor(() => expect(hasRow("drafted")).toBe(true));
    await userEvent.click(
      within(rowOf("drafted")).getByRole("button", { name: "Review the change" }),
    );
    expect(hands.onOpenView).toHaveBeenLastCalledWith(
      skillStudioAt(inWorkspace("ops"), "library", { skill: skillRowId(OPS[4]) }),
      SKILL_STUDIO_TITLE,
    );
  });
});

describe("the Skills view's review fixes", () => {
  it("counts the whole set in its header, whatever the filter leaves", async () => {
    core(() => studio(Array.from({ length: 12 }, (_, at) => skill(`s${at}`))));
    view();
    await list();
    await waitFor(() =>
      expect(screen.getByTestId("skills-view-whose")).toHaveTextContent("· 12 skills"),
    );
    await userEvent.type(screen.getByRole("searchbox", { name: "Filter skills" }), "s1");
    await waitFor(() => expect(hasRow("s2")).toBe(false));
    expect(screen.getByTestId("skills-view-whose")).toHaveTextContent("· 12 skills");
  });

  it("draws a built-in skill a skill of the project has the name of outside the fold", async () => {
    core(() =>
      studio([
        skill("secrets", { level: "built-in", label: "built-in", folder: "/app/skills/secrets" }),
        skill("browser", { level: "built-in", label: "built-in", folder: "/app/skills/browser" }),
        skill("secrets", { on: false, left_out: "a built-in skill has its name" }),
      ]),
    );
    view();
    await list();
    await waitFor(() => expect(foldOf("1 built-in skill")).toBeInTheDocument());
    expect(
      [...document.querySelectorAll('[data-testid^="skills-row-"] [data-part="label"]')].map(
        (one) => one.textContent,
      ),
    ).toContain("secrets");
    expect(hasRow("browser")).toBe(false);
  });

  it("opens a program's own skill on the studio's Harness skills page", async () => {
    core(
      () =>
        studio([skill("deploy")], {
          harnesses: [
            {
              harness: "claude",
              title: "Claude Code",
              skills: [skill("pdf", { level: "installed", label: "Claude Code", folder: "" })],
              repo_left_out: [],
            },
          ],
        }),
      { chat: [given("deploy")] },
    );
    const hands = view({
      chat: { session: 3, name: "web 1", workspace: "ops", persona: null, harness: "claude" },
    });
    await list();
    await waitFor(() => expect(hasRow("pdf")).toBe(true));
    await userEvent.click(rowOf("pdf"));
    expect(hands.onOpenView).toHaveBeenLastCalledWith(
      skillStudioAt(inWorkspace("ops"), "harness"),
      SKILL_STUDIO_TITLE,
    );
  });
});

describe("a row's menu", () => {
  const CHAT: FrontChat = {
    session: 7,
    name: "steward 5",
    workspace: "ops",
    persona: null,
    harness: "claude",
  };

  it("uses the skill in the chat in front, through Use skill…", async () => {
    opsCore([given("deploy"), given("ops-runbook", "workspace")]);
    const hands = view({ chat: CHAT });
    await list();
    await waitFor(() => expect(rowOf("deploy")).toBeInTheDocument());

    fireEvent.contextMenu(rowOf("deploy"));
    const menu = await screen.findByRole("menu", { name: "deploy" });
    const items = within(menu).getAllByRole("menuitem");
    expect(items).toHaveLength(6);
    expect(items[4]).toHaveTextContent(/^Open the file$/);
    expect(items[5]).toHaveTextContent(/^Open in Skill studio$/);
    expect(items[0]).toHaveTextContent(/^Use in this chat$/);
    expect(items[1]).toHaveTextContent(/^Turn off in ops$/);
    expect(items[2]).toHaveTextContent(/^Pin to this workspace$/);
    // Explorer shows a workspace's repos, so the row says where its folder is shown.
    expect(items[3]).toHaveTextContent(/^Show in the Explorer view/);
    expect(items[3]).toHaveAttribute("aria-disabled", "true");
    expect(items[3]).toHaveTextContent(/at the project's root/);
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Use in this chat" }));
    expect(hands.onUseSkill).toHaveBeenCalledWith(7, "deploy");
  });

  it("says Use in this chat waits for a chat while none is in front, and draws no Use", async () => {
    opsCore();
    view();
    await list();
    await waitFor(() => expect(rowOf("deploy")).toBeInTheDocument());
    expect(within(rowOf("deploy")).queryByRole("button", { name: /^Use / })).toBeNull();

    fireEvent.contextMenu(rowOf("deploy"));
    const menu = await screen.findByRole("menu", { name: "deploy" });
    const use = within(menu).getByRole("menuitem", { name: /Use in this chat/ });
    expect(use).toHaveAttribute("aria-disabled", "true");
    expect(use).toHaveTextContent("No chat is in front to use it in.");
  });

  it("opens a skill's file in a tab, and the skill in the studio", async () => {
    opsCore();
    const hands = view({ workspace: undefined });
    await list();
    await waitFor(() => expect(rowOf("deploy")).toBeInTheDocument());

    fireEvent.contextMenu(rowOf("deploy"));
    let menu = await screen.findByRole("menu", { name: "deploy" });
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Open the file" }));
    expect(hands.onOpenView).toHaveBeenLastCalledWith(
      pieceFileView(PLANE_ROOT, "skills/deploy/SKILL.md"),
      "SKILL.md · deploy",
    );
    fireEvent.contextMenu(rowOf("deploy"));
    menu = await screen.findByRole("menu", { name: "deploy" });
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Open in Skill studio" }));
    expect(hands.onOpenView).toHaveBeenLastCalledWith(
      skillStudioAt({ at: "root", name: "" }, "library", { skill: skillRowId(skill("deploy")) }),
      SKILL_STUDIO_TITLE,
    );
  });

  it("turns a project skill off in the workspace with the studio's own switch", async () => {
    const asked = opsCore();
    view();
    await list();
    await waitFor(() => expect(rowOf("deploy")).toBeInTheDocument());
    const reads = () => asked.filter((one) => one.cmd === "skill_studio").length;
    const before = reads();

    fireEvent.contextMenu(rowOf("deploy"));
    const menu = await screen.findByRole("menu", { name: "deploy" });
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Turn off in ops" }));

    await waitFor(() =>
      expect(asked).toContainEqual({
        cmd: "skill_turn",
        args: { plane: "p", workspace: "ops", name: "deploy", on: false },
      }),
    );
    // It is read again as the disk says the project's skills changed, not a second time here.
    expect(reads()).toBe(before);
  });

  it("pins a skill to the workspace with the studio's own command, one change", async () => {
    const asked = opsCore();
    view();
    await list();
    await waitFor(() => expect(rowOf("deploy")).toBeInTheDocument());

    fireEvent.contextMenu(rowOf("deploy"));
    const menu = await screen.findByRole("menu", { name: "deploy" });
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Pin to this workspace" }));

    await waitFor(() =>
      expect(asked.filter((one) => one.cmd === "skill_pin")).toEqual([
        {
          cmd: "skill_pin",
          args: { plane: "p", by: { at: "workspace", name: "ops" }, name: "deploy", on: true },
        },
      ]),
    );
  });

  it("unpins a skill the workspace pins", async () => {
    const asked = core(() =>
      studio([skill("deploy", { pinned: true, pinned_by: [{ at: "workspace", name: "ops" }] })]),
    );
    view();
    await list();
    await waitFor(() => expect(rowOf("deploy")).toBeInTheDocument());

    fireEvent.contextMenu(rowOf("deploy"));
    const menu = await screen.findByRole("menu", { name: "deploy" });
    expect(within(menu).queryByRole("menuitem", { name: "Pin to this workspace" })).toBeNull();
    await userEvent.click(
      within(menu).getByRole("menuitem", { name: "Unpin from this workspace" }),
    );

    await waitFor(() =>
      expect(asked).toContainEqual({
        cmd: "skill_pin",
        args: { plane: "p", by: { at: "workspace", name: "ops" }, name: "deploy", on: false },
      }),
    );
  });

  it("says a pin this workspace's chats read none of is not used, and the core's why (#3432)", async () => {
    core(() =>
      studio([skill("deploy", { pinned: true, pinned_by: [{ at: "workspace", name: "ops" }] })], {
        pins_refused: {
          by: { at: "workspace", name: "ops" },
          why: "the list holds more than 64 pins",
          bad: null,
          file: "workspaces/ops/skills.pins",
        },
      }),
    );
    view();
    await list();
    await waitFor(() => expect(rowOf("deploy")).toBeInTheDocument());

    fireEvent.contextMenu(rowOf("deploy"));
    const menu = await screen.findByRole("menu", { name: "deploy" });
    const unpin = within(menu).getByRole("menuitem", { name: "Unpin from this workspace" });
    // Said under the row, and as its description, not only on hover.
    expect(unpin).toHaveTextContent("Pinned, not used: the list holds more than 64 pins.");
    expect(unpin).toHaveAccessibleDescription(
      "Pinned, not used: the list holds more than 64 pins.",
    );
    expect(unpin).not.toHaveAttribute("aria-disabled", "true");
  });

  it("says the core's refusal where a pin is refused", async () => {
    const asked: Asked[] = [];
    mockIPC((cmd, args) => {
      asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
      if (cmd === "skill_studio") return studio([skill("deploy")]);
      if (cmd === "skill_persona_skills") return [];
      if (cmd === "skill_pin") throw "The list holds more than 64 pins.";
      return [];
    });
    view();
    await list();
    await waitFor(() => expect(rowOf("deploy")).toBeInTheDocument());

    fireEvent.contextMenu(rowOf("deploy"));
    const menu = await screen.findByRole("menu", { name: "deploy" });
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Pin to this workspace" }));

    const said = await screen.findByText("The list holds more than 64 pins.");
    expect(said.closest("[aria-label]")).toHaveAttribute("aria-label", "The skill was not pinned");
  });

  it("cannot turn a built-in skill off, and says why", async () => {
    opsCore();
    view();
    await list();
    await userEvent.click(await waitFor(() => foldOf("1 built-in skill")));
    await waitFor(() => expect(rowOf("secrets")).toBeInTheDocument());

    fireEvent.contextMenu(rowOf("secrets"));
    const menu = await screen.findByRole("menu", { name: "secrets" });
    expect(within(menu).getByRole("menuitem", { name: /Turn off/ })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
  });

  it("shows a project skill's folder in Explorer at the project's root", async () => {
    opsCore();
    const hands = view({ workspace: undefined });
    await list();
    await waitFor(() => expect(rowOf("deploy")).toBeInTheDocument());

    fireEvent.contextMenu(rowOf("deploy"));
    const menu = await screen.findByRole("menu", { name: "deploy" });
    await userEvent.click(
      within(menu).getByRole("menuitem", { name: "Show in the Explorer view" }),
    );
    expect(hands.onShowInExplorer).toHaveBeenCalledWith({ repo: "", folder: "skills/deploy" });
  });

  it("shows a repo skill of the chat's clone in its clone, in the workspace (#3308)", async () => {
    core(
      () =>
        studio([skill("deploy")], {
          harnesses: [
            {
              harness: "claude",
              title: "Claude Code",
              skills: [
                skill("lint-rs", {
                  level: "repo",
                  folder: "workspaces/ops/svc/.claude/skills/lint-rs",
                  owner: { kind: "repo", name: "svc" },
                }),
              ],
              repo_left_out: [],
            },
          ],
        }),
      { chat: [given("deploy")], repos: ["svc"] },
    );
    const hands = view({
      chat: { session: 9, name: "web 1", workspace: "ops", persona: null, harness: "claude" },
    });
    await list();
    await waitFor(() => expect(rowOf("lint-rs")).toBeInTheDocument());

    fireEvent.contextMenu(rowOf("lint-rs"));
    const menu = await screen.findByRole("menu", { name: "lint-rs" });
    await userEvent.click(
      within(menu).getByRole("menuitem", { name: "Show in the Explorer view" }),
    );
    expect(hands.onShowInExplorer).toHaveBeenCalledWith({
      repo: "svc",
      folder: ".claude/skills/lint-rs",
    });
  });
});

describe("the view's header", () => {
  it("opens the Skill studio on the workspace's chats", async () => {
    opsCore();
    const hands = view();
    await list();

    await userEvent.click(screen.getByRole("button", { name: "Open Skill studio" }));
    expect(hands.onOpenView).toHaveBeenCalledWith(
      skillStudioView(inWorkspace("ops")),
      SKILL_STUDIO_TITLE,
    );
    // ⌘-click: a studio tab of its own, on the same Library (#3681).
    const user = userEvent.setup();
    await user.keyboard("{Meta>}");
    await user.click(screen.getByRole("button", { name: "Open Skill studio" }));
    await user.keyboard("{/Meta}");
    expect(hands.onOpenView).toHaveBeenLastCalledWith(
      skillStudioAt(inWorkspace("ops"), "library", { newTab: true }),
      SKILL_STUDIO_TITLE,
    );
  });

  it("opens the studio's own New skill… dialog, which asks how (#3684)", async () => {
    opsCore();
    const hands = view();
    await list();

    await userEvent.click(screen.getByRole("button", { name: "New skill…" }));
    // No menu of its own: the studio opens on its Library, and its dialog asks how there.
    expect(screen.queryByRole("menu")).toBeNull();
    expect(hands.onOpenView).toHaveBeenLastCalledWith(
      skillStudioView(inWorkspace("ops")),
      SKILL_STUDIO_TITLE,
    );
    renderStudio(inWorkspace("ops"));
    expect(await screen.findByRole("dialog", { name: "New skill" })).toBeInTheDocument();
    forgetStudio();
  });
});

describe("a project with no skill of its own", () => {
  it("is offered the studio's claim, and New skill…", async () => {
    core(() =>
      studio([skill("secrets", { level: "built-in" })], { first: { starts: true, unmoved: null } }),
    );
    view();
    const empty = await screen.findByTestId("skills-view-empty");
    expect(within(empty).getByText("This project has no skills of its own yet")).toBeVisible();
    // A skill is started from New skill…, which asks how (#3684): the claim has no button.
    expect(empty).toHaveTextContent("Start one of the project's own from New skill…");
    expect(within(empty).queryByRole("button")).toBeNull();
    // The skills its chats already get are listed under it: its built-in skills, folded.
    await waitFor(() => expect(foldOf("1 built-in skill")).toBeInTheDocument());
  });

  it("says the core's sentence where its skills cannot be read", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_studio") throw "The project's skills folder is a link.";
      return null;
    });
    view();
    expect(await screen.findByTestId("skills-view-refused")).toHaveTextContent(
      "The project's skills folder is a link.",
    );
  });
});
