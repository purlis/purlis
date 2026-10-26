// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Skills view** (SK-55, #3266; decisions amendment 2026-10-26), against a mocked core: what
 * it lists from the Skill studio's own answer, the keyboard and a screen reader's way through
 * it, each act of a row's menu, its header, its switch to the chat in front, its empty state and
 * the workspace it follows. The sandbox draws no window: these check what each surface says and
 * does, in the DOM a reader reads.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ChatSkill, PersonaSkills, Studio, StudioSkill } from "./bindings";
import { SkillsView, type FrontChat } from "./SkillsView";
import { SkillStudioTab } from "./SkillStudioTab";
import { skill, studio } from "./skillsListing.testkit";
import { inWorkspace, skillStudioView } from "./skillStudio";
import { skillView } from "./skillRuns";
import { SKILL_CHAT, type SkillChatAsk } from "./skillDraftAsks";

afterEach(() => {
  cleanup();
  clearMocks();
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
  more: { personas?: PersonaSkills[]; chat?: ChatSkill[] } = {},
): Asked[] {
  const asked: Asked[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "skill_studio")
      return of(given.workspace as string | null, given.persona as string | null);
    if (cmd === "skill_persona_skills") return more.personas ?? [];
    if (cmd === "chat_skills")
      return { on: true, known: true, skills: more.chat ?? [], budget: null };
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
/** The row of skill `name`, by the words it is drawn with. */
const rowOf = (name: string): HTMLElement => {
  const found = [...document.querySelectorAll<HTMLElement>('[data-testid^="skills-row-"]')].find(
    (one) => one.querySelector('[data-part="label"]')?.textContent === name,
  );
  if (found === undefined) throw new Error(`no row of ${name}`);
  return found;
};
/** The groups the list draws, by their names, in their order. */
const groupNames = (grid: HTMLElement) =>
  [...grid.querySelectorAll('[data-part="header"]')].map((one) => one.textContent);

describe("what the Skills view lists", () => {
  it("is this workspace's skills by level, Needs you on top, each persona's own too", async () => {
    opsCore();
    view();
    const grid = await list();
    await waitFor(() => expect(within(grid).getByText("review")).toBeInTheDocument());

    expect(groupNames(grid)).toEqual(["Needs you", "Built-in", "Project", "Workspace", "Personas"]);
    // One mark at most per row, in words a reader hears.
    expect(rowOf("drafted")).toHaveTextContent("Needs review");
    expect(rowOf("old-deploy")).toHaveTextContent("Off");
    expect(rowOf("deploy")).not.toHaveAttribute("data-mark");
  });

  it("says a row's description and reason from the keyboard, as its description", async () => {
    opsCore();
    view();
    await list();
    await waitFor(() => expect(rowOf("old-deploy")).toBeInTheDocument());

    act(() => rowOf("old-deploy").focus());
    await userEvent.keyboard("{ArrowDown}{ArrowUp}");

    const tip = await screen.findByRole("tooltip");
    expect(tip).toHaveTextContent("What old-deploy does.");
    expect(tip).toHaveTextContent("ops turned it off.");
  });

  it("is walked with the arrows and opens a skill's tab on Enter, or on a click", async () => {
    opsCore();
    const hands = view();
    const grid = await list();
    await waitFor(() => expect(rowOf("deploy")).toBeInTheDocument());

    // One Tab stop: the list's first row, then the arrows.
    act(() => rowOf("drafted").focus());
    await userEvent.keyboard("{ArrowDown}");
    expect(rowOf("secrets")).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}{Enter}");
    expect(hands.onOpenView).toHaveBeenLastCalledWith(skillView("deploy"), "deploy");

    await userEvent.click(within(grid).getByText("ops-runbook"));
    expect(hands.onOpenView).toHaveBeenLastCalledWith(skillView("ops-runbook"), "ops-runbook");
  });

  it("finds a skill by typing its name, and filters a long list from its box", async () => {
    const many = Array.from({ length: 9 }, (_, at) => skill(`skill-${at}`));
    core(() => studio([...many, skill("deploy")]));
    view();
    const grid = await list();
    await waitFor(() => expect(rowOf("deploy")).toBeInTheDocument());

    act(() => rowOf("skill-0").focus());
    await userEvent.keyboard("{Escape}d");
    expect(rowOf("deploy")).toHaveFocus();

    await userEvent.type(screen.getByRole("searchbox", { name: "Filter skills" }), "dep");
    await waitFor(() => expect(within(grid).getAllByRole("row")).toHaveLength(2));
    expect(within(grid).getByText("deploy")).toBeInTheDocument();
  });

  it("re-reads when the title bar's workspace changes, and lists the project's and the personas' at the root", async () => {
    const asked = opsCore();
    const { rerender } = render(
      <SkillsView
        plane="p"
        workspace="ops"
        onOpenView={vi.fn()}
        onUseSkill={vi.fn()}
        onShowInExplorer={vi.fn()}
      />,
    );
    await waitFor(() => expect(rowOf("ops-runbook")).toBeInTheDocument());

    rerender(
      <SkillsView
        plane="p"
        workspace={undefined}
        onOpenView={vi.fn()}
        onUseSkill={vi.fn()}
        onShowInExplorer={vi.fn()}
      />,
    );
    const grid = await list();
    await waitFor(() => expect(within(grid).queryByText("ops-runbook")).toBeNull());
    await waitFor(() => expect(groupNames(grid)).toEqual(["Project", "Personas"]));
    expect(asked).toContainEqual({
      cmd: "skill_studio",
      args: { plane: "p", workspace: null, persona: null },
    });
    // Every persona's own skills in one read, never a studio per persona.
    expect(asked).toContainEqual({
      cmd: "skill_persona_skills",
      args: { plane: "p", workspace: null },
    });
    expect(asked.filter((one) => one.cmd === "skill_studio" && one.args.persona !== null)).toEqual(
      [],
    );
  });
});

describe("a row's menu", () => {
  const CHAT: FrontChat = { session: 7, workspace: "ops", persona: null, harness: "claude" };

  it("uses the skill in the chat in front, through Use skill…", async () => {
    opsCore();
    const hands = view({ chat: CHAT });
    await list();
    await waitFor(() => expect(rowOf("deploy")).toBeInTheDocument());

    fireEvent.contextMenu(rowOf("deploy"));
    const menu = await screen.findByRole("menu", { name: "deploy" });
    const items = within(menu).getAllByRole("menuitem");
    expect(items).toHaveLength(3);
    expect(items[0]).toHaveTextContent(/^Use in this chat$/);
    expect(items[1]).toHaveTextContent(/^Turn off in ops$/);
    // Explorer shows a workspace's repos, so the row says where its folder is shown.
    expect(items[2]).toHaveTextContent(/^Show in the Explorer view/);
    expect(items[2]).toHaveAttribute("aria-disabled", "true");
    expect(items[2]).toHaveTextContent(/at the project's root/);
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Use in this chat" }));
    expect(hands.onUseSkill).toHaveBeenCalledWith(7, "deploy");
  });

  it("offers no Use in this chat while no chat is in front", async () => {
    opsCore();
    view();
    await list();
    await waitFor(() => expect(rowOf("deploy")).toBeInTheDocument());

    fireEvent.contextMenu(rowOf("deploy"));
    const menu = await screen.findByRole("menu", { name: "deploy" });
    expect(within(menu).queryByRole("menuitem", { name: "Use in this chat" })).toBeNull();
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

  it("cannot turn a built-in skill off, and says why", async () => {
    opsCore();
    view();
    await list();
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
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Show in the Explorer view" }));
    expect(hands.onShowInExplorer).toHaveBeenCalledWith("skills/deploy");
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
      "Skill studio · ops",
    );
  });

  it("starts a new skill with the studio's own starts: a chat's draft, or the studio's form", async () => {
    opsCore();
    const asks: SkillChatAsk[] = [];
    const heard = (event: Event) => asks.push((event as CustomEvent<SkillChatAsk>).detail);
    window.addEventListener(SKILL_CHAT, heard);
    const hands = view();
    await list();

    await userEvent.click(screen.getByRole("button", { name: "New skill…" }));
    const starts = await screen.findByRole("menu", { name: "Ways to start a skill" });
    expect(
      within(starts)
        .getAllByRole("menuitem")
        .map((item) => item.textContent),
    ).toEqual([
      "Ask a chat to draft a skillSay what you want in a chat. Its draft comes back here to try and accept.",
      "Browse sourcesInstall a reviewed copy of a skill from a git repository.",
      "New skillWrite one yourself, from a blank template.",
    ]);
    await userEvent.click(within(starts).getByRole("menuitem", { name: /Ask a chat/ }));
    window.removeEventListener(SKILL_CHAT, heard);
    expect(asks).toEqual([
      { plane: "p", kind: "draft", level: { at: "workspace", name: "ops", local: false } },
    ]);

    await userEvent.click(screen.getByRole("button", { name: "New skill…" }));
    const again = await screen.findByRole("menu", { name: "Ways to start a skill" });
    await userEvent.click(within(again).getByRole("menuitem", { name: /blank template/ }));
    expect(hands.onOpenView).toHaveBeenLastCalledWith(
      skillStudioView(inWorkspace("ops")),
      "Skill studio · ops",
    );
    // The studio it opened opens on its New skill form: one copy of the start.
    render(<SkillStudioTab plane="p" scope={inWorkspace("ops")} changed={0} />);
    expect(await screen.findByRole("form", { name: "New skill" })).toBeInTheDocument();
  });
});

describe("Only this chat's", () => {
  const CHAT: FrontChat = { session: 7, workspace: "ops", persona: "reviewer", harness: "claude" };

  it("is there only while a chat tab is in front", async () => {
    opsCore();
    view();
    await list();
    expect(screen.queryByRole("button", { name: "Only this chat's" })).toBeNull();
  });

  it("narrows the list to the chat's own set, marks what it does not get, and is kept", async () => {
    const asked = opsCore([given("deploy"), given("review", "persona")]);
    view({ chat: CHAT });
    const grid = await list();
    await waitFor(() => expect(within(grid).getByText("ops-runbook")).toBeInTheDocument());

    const only = screen.getByRole("button", { name: "Only this chat's" });
    expect(only).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(only);

    // The chat's real set, as Use skill… reads it.
    expect(asked).toContainEqual({ cmd: "chat_skills", args: { plane: "p", session: 7 } });
    await waitFor(() =>
      expect(groupNames(grid)).toEqual(["Needs you", "Project", "Personas", "Not in this chat"]),
    );
    expect(rowOf("ops-runbook")).toHaveTextContent("Left out");
    // Use in this chat waits on a skill the chat does not get, and says why.
    fireEvent.contextMenu(rowOf("ops-runbook"));
    const menu = await screen.findByRole("menu", { name: "ops-runbook" });
    const use = within(menu).getByRole("menuitem", { name: /Use in this chat/ });
    expect(use).toHaveAttribute("aria-disabled", "true");
    expect(use).toHaveTextContent("The chat in front does not get it.");
    await userEvent.keyboard("{Escape}");

    // Kept for the window: drawn again, the switch is still on.
    cleanup();
    view({ chat: CHAT });
    expect(await screen.findByRole("button", { name: "Only this chat's" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await userEvent.click(screen.getByRole("button", { name: "Only this chat's" }));
  });

  it("keeps the chat's own program's skills and each repo's, which its set does not hold", async () => {
    const program = (harness: "claude" | "codex", title: string, skills: StudioSkill[]) => ({
      harness,
      title,
      unsupported: null,
      only_off: null,
      trouble: null,
      plugins: [],
      skills,
      repo_left_out: [],
    });
    const repo = skill("lint-rs", {
      level: "repo",
      folder: "",
      owner: { kind: "repo", name: "svc" },
    });
    core(
      () =>
        studio([skill("deploy")], {
          harnesses: [
            program("claude", "Claude Code", [
              skill("pdf", { level: "installed", folder: "" }),
              repo,
            ]),
            program("codex", "Codex", [skill("plan", { level: "installed", folder: "" }), repo]),
          ],
        }),
      // What the core's chat_skills holds: the plane's and the built-in skills alone.
      { chat: [given("deploy")] },
    );
    view({ chat: { session: 7, workspace: "ops", persona: null, harness: "claude" } });
    const grid = await list();
    await waitFor(() => expect(within(grid).getByText("plan")).toBeInTheDocument());

    await userEvent.click(screen.getByRole("button", { name: "Only this chat's" }));

    await waitFor(() => expect(within(grid).queryByText("plan")).toBeNull());
    expect(groupNames(grid)).toEqual(["Project", "Repo", "Claude Code"]);
    expect(rowOf("pdf")).not.toHaveAttribute("data-mark");
    fireEvent.contextMenu(rowOf("lint-rs"));
    const menu = await screen.findByRole("menu", { name: "lint-rs" });
    expect(within(menu).getByRole("menuitem", { name: "Use in this chat" })).not.toHaveAttribute(
      "aria-disabled",
      "true",
    );
    await userEvent.keyboard("{Escape}");
    await userEvent.click(screen.getByRole("button", { name: "Only this chat's" }));
  });

  it("keeps every group for a chat at the project's root", async () => {
    opsCore([given("secrets", "built-in"), given("deploy")]);
    view({
      workspace: undefined,
      chat: { session: 9, workspace: undefined, persona: null, harness: "claude" },
    });
    const grid = await list();
    await waitFor(() => expect(groupNames(grid)).toEqual(["Project", "Personas"]));

    await userEvent.click(screen.getByRole("button", { name: "Only this chat's" }));

    // Built-in is what the root chat gets: not hidden by the root's own grouping.
    await waitFor(() => expect(groupNames(grid)).toEqual(["Built-in", "Project"]));
    expect(rowOf("secrets")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Only this chat's" }));
  });
});

describe("a project with no skill of its own", () => {
  it("is offered the studio's claim, and its three starts under New skill…", async () => {
    core(() =>
      studio([skill("secrets", { level: "built-in" })], { first: { starts: true, unmoved: null } }),
    );
    view();
    const empty = await screen.findByTestId("skills-view-empty");
    expect(within(empty).getByText("This project has no skills of its own yet")).toBeVisible();
    // Its three starts are New skill…'s, so no start is a button of a name the studio's is too.
    expect(empty).toHaveTextContent("Start one of the project's own from New skill…");
    expect(within(empty).queryByRole("button")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "New skill…" }));
    const starts = await screen.findByRole("menu", { name: "Ways to start a skill" });
    expect(within(starts).getAllByRole("menuitem")).toHaveLength(3);
    // The skills its chats already get are listed under it.
    expect(rowOf("secrets")).toBeInTheDocument();
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
