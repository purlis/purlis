// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillStudioTab } from "./SkillStudioTab";
import { PLANE, STUDIO_HISTORY, forgetStudio, renderStudio } from "./SkillStudioTab.testkit";
import { need, skill, studio } from "./skillsListing.testkit";
import {
  AT_THE_ROOT,
  asPersona,
  harnessSkillNamed,
  inWorkspace,
  type StudioPage,
  type StudioScope,
} from "./skillStudio";
import { openViewAt } from "./ui/panel/viewHistory";
import type { Studio, StudioHarness, StudioRouter } from "./bindings";

/**
 * **The Harness skills and Routing pages** (#3692, spec #3679's S9, variant A of
 * `prototype/studio-secondary-pages`), at the window seam: the core faked at its commands.
 */

afterEach(() => {
  cleanup();
  clearMocks();
  forgetStudio();
});

/** Claude Code's own skills: the person's, a plugin's (read-only) and a repo's; opencode none. */
function harnesses(offLangfuse = false): StudioHarness[] {
  return [
    {
      harness: "claude",
      title: "Claude Code",
      repo_left_out: [],
      own: null,
      skills: [
        skill("langfuse", {
          level: "installed",
          label: "installed",
          grant: null,
          granted: null,
          on: !offLangfuse,
          left_out: offLangfuse ? "turned off for this project's chats" : null,
          description: "Reads traces.",
          folder: "/home/.claude/skills/langfuse",
          owner: { kind: "you" },
          switch: { kind: "alone" },
        }),
        skill("figma:frame", {
          level: "installed",
          label: "installed",
          grant: null,
          granted: null,
          description: "Frames.",
          folder: "/cache/figma/skills/frame",
          owner: { kind: "plugin", name: "figma" },
          switch: {
            kind: "not",
            why: "it comes with the plugin figma, which Claude Code turns on or off itself, so its skills are shown read-only",
          },
        }),
        skill("review", {
          level: "repo",
          label: "repo",
          grant: null,
          granted: null,
          description: "The repo's review.",
          folder: "workspaces/alpha/api/.claude/skills/review",
          owner: { kind: "repo", name: "api" },
          switch: { kind: "alone" },
        }),
      ],
    },
    { harness: "opencode", title: "opencode", repo_left_out: [], own: null, skills: [] },
  ];
}

/** The built-in router, as the core says it for a scope that chooses none. */
const BUILT_IN: StudioRouter = {
  said: "Chats here are routed by the built-in router.",
  problem: false,
  extension: null,
  chats: [],
  grade: null,
  graded: null,
  written: null,
  budget: { skills: 30, chars: 6000 },
  preview:
    "The rows' hidden marks are what the built-in router hides for a chat at the workspace's root. A chat started in a clone counts that clone's repo skills too, and its own Skills view says what it hides.",
};

/** A core answering `answer` for the studio, `more` for each other command, and keeping every
 *  ask. */
function core(answer: () => Studio, more: Record<string, (args: unknown) => unknown> = {}) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "skill_studio") return answer();
    if (cmd in more) return more[cmd](args);
    if (cmd === "skills_deleted" || cmd === "skill_drafts") return [];
    if (cmd === "skill_deletes_cut_short") return { settled: [], unsettled: [] };
    if (cmd.startsWith("plugin:event")) return 0;
    throw new Error(`unexpected ${cmd}`);
  });
  return asked;
}

/** The studio drawn at `page` with `skill` chosen. */
function renderAt(page: StudioPage, scope: StudioScope, skill: string) {
  openViewAt(STUDIO_HISTORY, { page, scope, skill });
  return render(<SkillStudioTab plane={PLANE} changed={0} />);
}

describe("the Harness skills page (#3692)", () => {
  it("groups each program's own skills with whose each is, a plugin's read-only, and says when a program has none", async () => {
    core(() => studio([], { harnesses: harnesses() }));
    renderStudio(AT_THE_ROOT, "harness");

    const claude = await screen.findByRole("row", { name: /^Claude Code/ });
    expect(claude).toBeInTheDocument();
    const own = screen.getByTestId("found-claude-langfuse");
    expect(within(own).getByText("You")).toBeInTheDocument();
    expect(within(own).getByText("Reads traces.")).toBeInTheDocument();
    const plugin = screen.getByTestId("found-claude-figma:frame");
    expect(within(plugin).getByText("Plugin figma")).toBeInTheDocument();
    expect(within(plugin).getByText("Read-only")).toBeInTheDocument();
    expect(
      within(plugin).getByText(/It comes with the plugin figma, which Claude Code turns on or off/),
    ).toBeInTheDocument();
    expect(
      within(screen.getByTestId("found-claude-review")).getByText("Repo api"),
    ).toBeInTheDocument();
    expect(screen.getByTestId("harness-none-opencode")).toHaveTextContent(
      "opencode has no skills of its own here.",
    );
    // The old tables are gone: a list, with the chosen row's page beside it.
    expect(screen.queryByRole("table")).toBeNull();
    expect(screen.getByTestId("harness-unpicked")).toBeInTheDocument();
  });

  it("opens the row chosen beside the list, and its switch turns it off through the core", async () => {
    let off = false;
    const asked = core(() => studio([], { harnesses: harnesses(off) }), {
      skill_studio_switch: () => {
        off = true;
        return null;
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "harness");

    await user.click(await screen.findByTestId("found-claude-langfuse"));
    const item = await screen.findByTestId("harness-skill-item");
    expect(within(item).getByRole("heading", { name: /langfuse/ })).toBeInTheDocument();
    expect(
      within(item).getByText("On for this project's chats", { selector: "dd" }),
    ).toBeInTheDocument();
    const toggle = within(item).getByRole("switch", { name: "On for this project's chats" });
    expect(toggle).toBeChecked();

    await user.click(toggle);
    await waitFor(() =>
      expect(
        within(screen.getByTestId("found-claude-langfuse")).getByText("Off here"),
      ).toBeInTheDocument(),
    );
    expect(asked.filter((one) => one.cmd === "skill_studio_switch").map((one) => one.args)).toEqual(
      [{ plane: "p", workspace: null, harness: "claude", id: "langfuse", on: false }],
    );
    expect(
      within(screen.getByTestId("harness-skill-item")).getByText(
        "Left out: turned off for this project's chats",
      ),
    ).toBeInTheDocument();
  });

  it("says what the core refused beside the switch, and changes nothing", async () => {
    core(() => studio([], { harnesses: harnesses() }), {
      skill_studio_switch: () => {
        throw "Claude Code turns it off only with its plugin";
      },
    });
    const user = userEvent.setup();
    renderStudio(inWorkspace("ops"), "harness");

    await user.click(await screen.findByTestId("found-claude-review"));
    const item = await screen.findByTestId("harness-skill-item");
    await user.click(within(item).getByRole("switch", { name: "On for ops's chats" }));
    expect(
      await within(item).findByText("Claude Code turns it off only with its plugin"),
    ).toBeInTheDocument();
    expect(within(item).getByRole("switch", { name: "On for ops's chats" })).toBeChecked();
  });

  it("offers no switch for a plugin's skill, and says why", async () => {
    core(() => studio([], { harnesses: harnesses() }));
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "harness");

    await user.click(await screen.findByTestId("found-claude-figma:frame"));
    const item = await screen.findByTestId("harness-skill-item");
    expect(within(item).queryByRole("switch")).toBeNull();
    expect(
      within(item).getByText(/^It comes with the plugin figma, which Claude Code turns/),
    ).toBeInTheDocument();
  });

  it("opens at a program's skill named only by its name, as a Needs you fix lands there", async () => {
    core(() => studio([], { harnesses: harnesses() }));
    renderAt("harness", AT_THE_ROOT, harnessSkillNamed("claude", "review"));

    const item = await screen.findByTestId("harness-skill-item");
    expect(within(item).getByRole("heading", { name: /review/ })).toBeInTheDocument();
    expect(within(item).getByText("Repo api", { selector: "dd" })).toBeInTheDocument();
    expect(screen.getByTestId("found-claude-review")).toHaveAttribute("aria-selected", "true");
  });

  it("lands on the program's skill a Needs you item's Show in Harness skills is about", async () => {
    core(() =>
      studio([], {
        harnesses: harnesses(),
        needs: [
          need("problem:langfuse", {
            kind: "problem",
            title: "langfuse",
            tag: "installed",
            reason: "Step 3 names a script the skill doesn't have.",
            fix: { kind: "harness" },
            fix_label: "Show in Harness skills",
            fix_does: "Opens Harness skills, where langfuse has its switch for this project.",
          }),
        ],
      }),
    );
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "needs");

    await user.click(await screen.findByRole("button", { name: "Show in Harness skills" }));
    const item = await screen.findByTestId("harness-skill-item");
    expect(within(item).getByRole("heading", { name: /langfuse/ })).toBeInTheDocument();
    expect(within(item).getByRole("switch")).toBeInTheDocument();
  });

  it("lists under a program each project skill a repo leaves out of its chats, and each program that takes no skills", async () => {
    const programs = harnesses();
    programs[1].repo_left_out = [
      skill("ship", {
        level: "workspace",
        label: "workspace",
        on: false,
        left_out: "purlis could not read the skills folder of web whole",
        folder: "workspaces/alpha/skills/ship",
      }),
    ];
    core(() =>
      studio([], {
        harnesses: programs,
        not_supported: [
          {
            name: "goose",
            title: "Goose",
            said: "Skills are not supported on Goose: it has no skill format purlis can hand skills by.",
          },
        ],
      }),
    );
    renderStudio(AT_THE_ROOT, "harness");

    const row = await screen.findByTestId("repo-left-out-opencode-ship");
    expect(row).toHaveTextContent("workspace");
    expect(row).toHaveTextContent("Left out by a repo");
    expect(row).toHaveTextContent("purlis could not read the skills folder of web whole");
    const goose = screen.getByTestId("skill-studio-not-supported-goose");
    expect(goose).toHaveTextContent("Goose");
    expect(goose).toHaveTextContent("Skills are not supported on Goose");
  });
});

/** An extension's router with two chats open, one that fell back. */
const RELEVANCE: StudioRouter = {
  said: "Chats here are routed by the router relevance, asked at each chat's start.",
  problem: false,
  extension: "relevance",
  written: "relevance",
  budget: { skills: 30, chars: 6000 },
  preview:
    "The rows' hidden marks are what the built-in router hides for a chat at the workspace's root, since the router relevance is asked only at each chat's start. A chat started in a clone counts that clone's repo skills too, and its own Skills view says what it hides.",
  grade: null,
  graded: null,
  chats: [
    {
      session: 3,
      routing: {
        router: "relevance",
        ms: 34,
        fell_back: false,
        said: "Routed by the router relevance in 34 ms.",
      },
      hidden: ["figma-handoff", "gpu-profile"],
    },
    {
      session: 4,
      routing: {
        router: "relevance",
        ms: 100,
        fell_back: true,
        said: "The router relevance did not answer within 100 ms, so this chat lists what the built-in router shows.",
      },
      hidden: [],
    },
  ],
};

describe("the Routing page (#3692)", () => {
  it("says the router, the budget, and that no chat here was routed by an extension", async () => {
    core(() => studio([], { router: BUILT_IN }));
    renderStudio(inWorkspace("ops"), "routing");

    const facts = await screen.findByTestId("skill-routing-facts");
    expect(within(facts).getByTestId("skill-routing-router")).toHaveTextContent("Built-in");
    expect(facts).toHaveTextContent(
      "30 skills, 6,000 characters of descriptions, on every program",
    );
    // What it counts, and that a program's own catalogue is not counted (#3324).
    expect(within(facts).getByTestId("skill-routing-budget-counts")).toHaveTextContent(
      "the built-in ones, the project's, and those of the repo the chat works in. A program's own skills, in your folders and its plugins, are not counted",
    );
    expect(within(facts).getByRole("button", { name: "Change…" })).toBeEnabled();
    expect(screen.getByTestId("skill-routing-no-chats")).toBeInTheDocument();
    expect(screen.queryByTestId("router-grade")).toBeNull();
  });

  it("says what the rows' hidden marks preview: the built-in router's choice at the workspace's root (#4215)", async () => {
    core(() => studio([], { router: RELEVANCE }));
    renderStudio(inWorkspace("ops"), "routing");

    const facts = await screen.findByTestId("skill-routing-facts");
    expect(within(facts).getByTestId("skill-routing-preview")).toHaveTextContent(
      "what the built-in router hides for a chat at the workspace's root, since the router relevance is asked only at each chat's start",
    );
  });

  it("keeps Change… out of reach at the project's root, which chooses no router", async () => {
    core(() => studio([], { router: BUILT_IN }));
    renderStudio(AT_THE_ROOT, "routing");

    const facts = await screen.findByTestId("skill-routing-facts");
    expect(within(facts).getByRole("button", { name: "Change…" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
  });

  it("lists each chat's start with its timing or its fallback, and opens what it hid beside it", async () => {
    core(() => studio([], { router: RELEVANCE }));
    const user = userEvent.setup();
    renderStudio(inWorkspace("ops"), "routing");

    const quick = await screen.findByTestId("routed-chat-3");
    expect(quick).toHaveTextContent("34 ms");
    expect(quick).toHaveTextContent("It hid 2 skills at the chat's start.");
    const slow = screen.getByTestId("routed-chat-4");
    expect(slow).toHaveTextContent("Fell back to the built-in router");
    expect(slow).toHaveTextContent("did not answer within 100 ms");
    expect(screen.getByTestId("skill-routing-router")).toHaveTextContent("relevance");
    expect(screen.getByTestId("skill-studio-router-grade")).toHaveTextContent(
      /Not graded on this library yet/,
    );
    expect(screen.getByRole("button", { name: "Grade the router…" })).toBeInTheDocument();

    await user.click(quick);
    const item = await screen.findByTestId("routed-chat-item");
    expect(within(item).getByRole("heading", { name: /Chat 3/ })).toBeInTheDocument();
    expect(within(item).getByText("figma-handoff, gpu-profile")).toBeInTheDocument();
    expect(within(item).getByText("34 ms", { selector: "dd" })).toBeInTheDocument();
  });

  it("shows a prompt's fallback on an open Routing page as it is told (#4445)", async () => {
    const [quick, ...others] = RELEVANCE.chats;
    const said = "Its last prompt went on with no pointer.";
    let prompt: { fell_back: boolean; said: string } | null = null;
    let studioReads = 0;
    const reads = () => studioReads;
    mockIPC(
      (cmd) => {
        if (cmd === "skill_studio") studioReads += 1;
        if (cmd === "skill_studio")
          return studio([], {
            router: {
              ...RELEVANCE,
              chats: [{ ...quick, routing: { ...quick.routing, last_prompt: prompt } }, ...others],
            },
          });
        if (cmd === "skills_deleted" || cmd === "skill_drafts") return [];
        if (cmd === "skill_deletes_cut_short") return { settled: [], unsettled: [] };
        throw new Error(`unexpected ${cmd}`);
      },
      { shouldMockEvents: true },
    );
    renderStudio(inWorkspace("ops"), "routing");
    const row = await screen.findByTestId("routed-chat-3");
    expect(row).not.toHaveTextContent("Last prompt fell back");
    const before = reads();

    // A chat this page does not list reads nothing; this one reads it again.
    prompt = { fell_back: true, said };
    await act(() => emit("chat-prompt-routing", { plane: PLANE, session: 99 }));
    expect(reads()).toBe(before);
    await act(() => emit("chat-prompt-routing", { plane: PLANE, session: 3 }));
    await waitFor(() =>
      expect(screen.getByTestId("routed-chat-3")).toHaveTextContent("Last prompt fell back"),
    );

    prompt = { fell_back: false, said: "Its last prompt took 4 ms." };
    await act(() => emit("chat-prompt-routing", { plane: PLANE, session: 3 }));
    await waitFor(() =>
      expect(screen.getByTestId("routed-chat-3")).not.toHaveTextContent("Last prompt fell back"),
    );
  });

  it("shows a prompt the router was slow on as fallen back on that chat's row and beside it (#4297)", async () => {
    const said =
      "Its last prompt went on with no pointer: 'relevance' did not answer within 100 ms.";
    const [quick, fell] = RELEVANCE.chats;
    core(() =>
      studio([], {
        router: {
          ...RELEVANCE,
          chats: [
            {
              ...quick,
              routing: {
                ...quick.routing,
                last_prompt: { fell_back: true, said },
              },
            },
            {
              ...fell,
              routing: {
                ...fell.routing,
                last_prompt: { fell_back: false, said: "Its last prompt took 4 ms." },
              },
            },
          ],
        },
      }),
    );
    const user = userEvent.setup();
    renderStudio(inWorkspace("ops"), "routing");

    const row = await screen.findByTestId("routed-chat-3");
    expect(row).toHaveTextContent("Last prompt fell back");
    expect(row).not.toHaveTextContent("34 ms");
    expect(row).toHaveTextContent(said);
    expect(row).toHaveTextContent("It hid 2 skills at the chat's start.");
    // The start's fallback is the graver: the list itself is the built-in router's.
    const other = screen.getByTestId("routed-chat-4");
    expect(other).toHaveTextContent("Fell back to the built-in router");
    expect(other).toHaveTextContent("Its last prompt took 4 ms.");
    // The line is cut at its end, so it leads with what the mark says.
    expect(row.textContent).toMatch(/Its last prompt went on.*It hid 2 skills/);
    expect(other.textContent).toMatch(/did not answer within 100 ms.*Its last prompt took 4 ms/);

    await user.click(row);
    const item = await screen.findByTestId("routed-chat-item");
    expect(within(item).getByText(said, { selector: "dd" })).toBeInTheDocument();
    expect(within(item).getByText("34 ms", { selector: "dd" })).toBeInTheDocument();
  });

  it("says why a choice that names no router is the built-in one", async () => {
    core(() =>
      studio([], {
        router: {
          ...BUILT_IN,
          problem: true,
          written: "x/y",
          said: 'the workspace ops chooses "x/y" as its router, which is neither built-in nor an extension\'s id, so its chats are routed by the built-in router.',
        },
      }),
    );
    renderStudio(inWorkspace("ops"), "routing");

    const router = await screen.findByTestId("skill-routing-router");
    expect(router).toHaveTextContent("Not a router");
    expect(router).toHaveTextContent(/so its chats are routed by the built-in router/);
  });

  it("changes a workspace's router through Change…, saying what changes before it does", async () => {
    let router = BUILT_IN;
    const asked = core(() => studio([], { router }), {
      skill_routers: () => [{ id: "relevance", name: "Relevance router" }],
      skill_router_choose: () => {
        router = { ...RELEVANCE, chats: [] };
        return { notes: [] };
      },
    });
    const user = userEvent.setup();
    renderStudio(inWorkspace("ops"), "routing");

    await user.click(await screen.findByRole("button", { name: "Change…" }));
    const dialog = await screen.findByRole("dialog", { name: "Change the router" });
    expect(within(dialog).getByText("For chats in ops")).toBeInTheDocument();
    const change = within(dialog).getByRole("button", { name: "Change router" });
    expect(change).toBeDisabled();
    expect(within(dialog).getByRole("radio", { name: "Built-in" })).toBeChecked();
    expect(within(dialog).queryByRole("radio", { name: "Its workspace's router" })).toBeNull();

    await user.click(await within(dialog).findByRole("radio", { name: "relevance" }));
    expect(dialog).toHaveTextContent(
      "Chats in ops started after this are routed by the router relevance. Chats open now keep theirs.",
    );
    await user.click(change);

    expect(
      await screen.findByText(
        "Chats in ops started after this are routed by the router relevance.",
      ),
    ).toBeInTheDocument();
    expect(asked.filter((one) => one.cmd === "skill_router_choose").map((one) => one.args)).toEqual(
      [{ plane: "p", by: { at: "workspace", name: "ops" }, router: "relevance" }],
    );
    await waitFor(() =>
      expect(screen.getByTestId("skill-routing-router")).toHaveTextContent("relevance"),
    );
  });

  it("offers a router that reads prompts a switch, off by default, with the line beside it", async () => {
    let prompts_on = false;
    const asked = core(() => studio([], { router: { ...RELEVANCE } }), {
      skill_routers: () => [
        { id: "relevance", name: "Relevance router", reads_prompts: true, prompts_on },
      ],
      skill_router_reads_prompts: () => {
        prompts_on = true;
        return { notes: [] };
      },
    });
    const user = userEvent.setup();
    renderStudio(inWorkspace("ops"), "routing");

    const reading = await screen.findByRole("switch", { name: "Let it read prompts" });
    expect(reading).not.toBeChecked();
    expect(screen.getByTestId("skill-routing-reads-every-prompt")).toHaveTextContent(
      "reads every prompt of every chat it routes",
    );

    await user.click(reading);

    await waitFor(() =>
      expect(screen.getByRole("switch", { name: "Let it read prompts" })).toBeChecked(),
    );
    expect(
      asked.filter((one) => one.cmd === "skill_router_reads_prompts").map((one) => one.args),
    ).toEqual([{ plane: "p", router: "relevance", on: true }]);
    expect(screen.getByTestId("skill-routing-reads-every-prompt")).toBeInTheDocument();
  });

  it("offers no switch to a router that reads no prompts", async () => {
    core(() => studio([], { router: { ...RELEVANCE } }), {
      skill_routers: () => [
        { id: "relevance", name: "Relevance router", reads_prompts: false, prompts_on: false },
      ],
    });
    renderStudio(inWorkspace("ops"), "routing");

    await screen.findByTestId("skill-routing-router");
    await waitFor(() => expect(screen.queryByRole("switch")).toBeNull());
    expect(screen.queryByTestId("skill-routing-reads-every-prompt")).toBeNull();
  });

  it("offers a persona its workspace's router, and takes its own choice out with it", async () => {
    const asked = core(
      () => studio([], { router: { ...BUILT_IN, written: "built-in" }, persona: "reviewer" }),
      {
        skill_routers: () => [],
        skill_router_choose: () => ({ notes: [] }),
      },
    );
    const user = userEvent.setup();
    renderStudio(asPersona("reviewer"), "routing");

    await user.click(await screen.findByRole("button", { name: "Change…" }));
    const dialog = await screen.findByRole("dialog", { name: "Change the router" });
    expect(within(dialog).getByText("For chats as reviewer")).toBeInTheDocument();
    expect(await within(dialog).findByTestId("router-change-none")).toBeInTheDocument();
    expect(within(dialog).getByRole("radio", { name: "Built-in" })).toBeChecked();

    await user.click(within(dialog).getByRole("radio", { name: "Its workspace's router" }));
    await user.click(within(dialog).getByRole("button", { name: "Change router" }));
    await waitFor(() =>
      expect(
        asked.filter((one) => one.cmd === "skill_router_choose").map((one) => one.args),
      ).toEqual([{ plane: "p", by: { at: "persona", name: "reviewer" }, router: null }]),
    );
  });

  it("says an extension's newest grade beside the library's commit (SK-15e)", async () => {
    const grade =
      "Graded on the library at aaaaaaaaaaaa, 2026-10-02: 1 of 2 trigger tests pass with the router relevance, and 2 of 2 with the built-in router.";
    core(() => studio([], { router: { ...RELEVANCE, grade } }));
    renderStudio(inWorkspace("ops"), "routing");

    expect(await screen.findByTestId("skill-studio-router-grade")).toHaveTextContent(grade);
    expect(screen.getByRole("button", { name: "Grade the router…" })).toBeInTheDocument();
  });

  it("opens the router's newest grade's scorecard beside the list (#3617)", async () => {
    const grade =
      "Graded on the library at aaaaaaaaaaaa, 2026-10-02: 1 of 2 trigger tests pass with the router relevance, and 2 of 2 with the built-in router.";
    const asked = core(() => studio([], { router: { ...RELEVANCE, grade, graded: "01GRADE" } }), {
      router_grade_scorecard: () => ({
        grade: "01GRADE",
        router: "relevance",
        graded: "2026-10-02 10:00 UTC",
        library: "the library at aaaaaaaaaaaa",
        ran_on: "Claude Code",
        ended: "Finished",
        outcome: "graded",
        arms: { router: 50, built_in: 100 },
        spent: { chats: 12, tokens: 12000, cost_usd: null, limit_usd: 1.5 },
        asked: 0,
        skills: [],
      }),
    });
    renderStudio(inWorkspace("ops"), "routing");

    await userEvent.click(await screen.findByRole("button", { name: "Scorecard" }));
    expect(await screen.findByTestId("router-grade-scorecard")).toBeInTheDocument();
    expect(
      asked.filter((one) => one.cmd === "router_grade_scorecard").map((one) => one.args),
    ).toEqual([{ plane: PLANE, workspace: "ops", persona: null, grade: "01GRADE" }]);
  });

  it("opens the router's grade plan beside the list, then its grade running (#4222)", async () => {
    const plan = {
      router: "relevance",
      library: 2,
      skills: [{ skill: "deploy", tests: 2, left_out: [], refused: null }],
      chats: 12,
      runs: 3,
      chat_minutes: 5,
      commit: "a".repeat(40),
      token_cap: 200000,
      cap_set_here: false,
      cost_cap_usd: 15,
      priced: { usd_per_million: 75, of_the_model: false, model: null },
      profile: "work",
      sandbox: null,
      refused: null,
      models: [],
      model: null,
      estimate: null,
      estimate_same_library: null,
      running: null,
      consent: "c0ffee",
    };
    const step = {
      plane: PLANE,
      router: "relevance",
      workspace: "ops",
      persona: null,
      grade: "01GRADE",
      ran: 0,
      of: 12,
      tokens: null,
      cost_usd: null,
      limit_usd: 15,
      cap: 200000,
      ended: null,
      said: null,
      recorded: false,
    };
    const asked = core(() => studio([], { router: RELEVANCE }), {
      router_grade_plan: () => plan,
      start_options: () => ({ profiles: [] }),
      router_grade_start: () => "01GRADE",
      router_grade_running: () => step,
    });
    const user = userEvent.setup();
    renderStudio(inWorkspace("ops"), "routing");

    await user.click(await screen.findByRole("button", { name: "Grade the router…" }));
    expect(await screen.findByTestId("router-grade-plan")).toBeInTheDocument();
    expect(asked.some((one) => one.cmd === "router_grade_start")).toBe(false);

    await user.click(screen.getByRole("button", { name: "Start grading · 12 chats" }));
    expect(await screen.findByTestId("router-grade-running")).toBeInTheDocument();
    expect(screen.queryByTestId("router-grade-plan")).toBeNull();
    expect(asked.find((one) => one.cmd === "router_grade_running")?.args).toEqual({
      plane: PLANE,
      grade: "01GRADE",
    });
  });

  it("lets a router written as no router's name be replaced by the built-in one", async () => {
    const asked = core(
      () =>
        studio([], {
          router: {
            ...BUILT_IN,
            problem: true,
            written: null,
            said: "the workspace ops's settings.skills.router is not a router's name, so its chats are routed by the built-in router.",
          },
        }),
      {
        skill_routers: () => {
          throw "purlis could not read this machine's extensions";
        },
        skill_router_choose: () => ({ notes: [] }),
      },
    );
    const user = userEvent.setup();
    renderStudio(inWorkspace("ops"), "routing");

    await user.click(await screen.findByRole("button", { name: "Change…" }));
    const dialog = await screen.findByRole("dialog", { name: "Change the router" });
    expect(
      await within(dialog).findByText(/purlis could not read the router extensions/),
    ).toBeInTheDocument();
    await user.click(within(dialog).getByRole("radio", { name: "Built-in" }));
    await user.click(within(dialog).getByRole("button", { name: "Change router" }));
    await waitFor(() =>
      expect(
        asked.filter((one) => one.cmd === "skill_router_choose").map((one) => one.args),
      ).toEqual([{ plane: "p", by: { at: "workspace", name: "ops" }, router: "built-in" }]),
    );
  });
});
