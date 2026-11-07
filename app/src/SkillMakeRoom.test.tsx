// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Make room** (#5355), at the window seam: the Routing page offers it when these chats are
 * over their skill budget, and it opens the Library's Select mode (#5332) with the skills least
 * used in the last 30 days first, from the run records, nothing ticked, and only Turn off,
 * Archive and Pin. The core is faked at its command boundary; what each change does is the core's
 * seam test (`verbs::set`), and how runs are counted is `skilluse::runs_since`'s.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SkillHiddenOn, StudioBudget, StudioRouter, StudioSkill } from "./bindings";
import { forgetStudio, renderStudio } from "./SkillStudioTab.testkit";
import { inWorkspace } from "./skillStudio";
import { skill, studio } from "./skillsListing.testkit";
import { hiddenCount, installGoesOver, leastUsedFirst, usesSaid } from "./makeRoom";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetStudio();
});

const CLAUDE: StudioBudget = {
  program: "Claude Code",
  key: "claude",
  skills: 3,
  chars: 15000,
  skills_said: "3, this workspace",
  chars_said: "15,000, Claude Code's default",
  skills_here: 3,
  chars_here: null,
};

const ROUTER: StudioRouter = {
  said: "Chats here are routed by the built-in router.",
  problem: false,
  extension: null,
  chats: [],
  grade: null,
  graded: null,
  written: null,
  budget: [CLAUDE],
  past_limits: [],
  preview: "The rows' hidden marks are what the built-in router hides.",
};

/** Hidden on Claude Code's chats, the list being over the budget. */
const HIDDEN: SkillHiddenOn[] = [
  {
    harness: "claude",
    title: "Claude Code",
    hidden: { router: "built-in", why: "over-budget", said: "Hidden: over the budget." },
  },
];

/** A core that lists `skills` in workspace ops, says `uses` of the last 30 days, and keeps
 *  every ask. */
function core(skills: StudioSkill[], uses: { skill: string; runs: number }[]) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    switch (cmd) {
      case "skill_studio":
        return studio(skills, { workspace: "ops", router: ROUTER });
      case "skill_uses":
        return uses;
      case "skills_deleted":
      case "skill_drafts":
        return [];
      case "skill_deletes_cut_short":
        return { settled: [], unsettled: [] };
      case "skill_set_review": {
        const members = given.members as { level: unknown; name: string; label: string }[];
        return {
          skills: members.map((member) => ({ member, changed: false, cannot: null })),
          names: [],
          open_chats: 0,
        };
      }
      case "skill_set_turn":
        return { notes: [], changed: given.names };
      default:
        return { notes: [] };
    }
  });
  return asked;
}

/** Four skills in ops, one hidden over the budget, each used differently lately. */
const OVER = [skill("deploy"), skill("lint"), skill("triage", { hidden: HIDDEN }), skill("notes")];
const USES = [
  { skill: "deploy", runs: 12 },
  { skill: "lint", runs: 1 },
  { skill: "triage", runs: 4 },
];

/** The Library's rows, top to bottom, by name. */
function rowNames(): string[] {
  return within(screen.getByRole("grid", { name: "Skills" }))
    .getAllByRole("row")
    .map((row) => row.getAttribute("data-testid") ?? "")
    .filter((id) => id.startsWith("skill-"))
    .map((id) => id.slice("skill-".length));
}

describe("Make room", () => {
  it("is offered on Routing while these chats are over their budget, and not otherwise", async () => {
    core(OVER, USES);
    renderStudio(inWorkspace("ops"), "routing");
    const offer = await screen.findByTestId("skill-routing-over");
    expect(offer).toHaveTextContent("The router hides 1 skill from these chats' lists");
    expect(within(offer).getByRole("button", { name: "Make room…" })).toBeEnabled();
    cleanup();
    forgetStudio();

    core([skill("deploy"), skill("lint")], USES);
    renderStudio(inWorkspace("ops"), "routing");
    await screen.findByTestId("skill-routing-facts");
    expect(screen.queryByTestId("skill-routing-over")).toBeNull();
  });

  it("opens Select on the Library, least used in the last 30 days first, with nothing ticked", async () => {
    const asked = core(OVER, USES);
    renderStudio(inWorkspace("ops"), "routing");
    const user = userEvent.setup();
    await user.click(
      within(await screen.findByTestId("skill-routing-over")).getByRole("button", {
        name: "Make room…",
      }),
    );

    await screen.findByTestId("select-none");
    await waitFor(() => expect(rowNames()).toEqual(["notes", "lint", "triage", "deploy"]));
    expect(asked.some((one) => one.cmd === "skill_uses")).toBe(true);
    expect(within(screen.getByTestId("skill-notes")).getByTestId("skill-uses")).toHaveTextContent(
      "No runs in the last 30 days",
    );
    expect(within(screen.getByTestId("skill-lint")).getByTestId("skill-uses")).toHaveTextContent(
      "1 run in the last 30 days",
    );
    expect(within(screen.getByTestId("skill-deploy")).getByTestId("skill-uses")).toHaveTextContent(
      "12 runs in the last 30 days",
    );
    for (const box of screen.getAllByRole("checkbox", { name: /^Select / }))
      expect(box).not.toBeChecked();
  });

  it("offers only Turn off, Archive and Pin for what is ticked, and turns them off as one change", async () => {
    const asked = core(OVER, USES);
    renderStudio(inWorkspace("ops"), "routing");
    const user = userEvent.setup();
    await user.click(
      within(await screen.findByTestId("skill-routing-over")).getByRole("button", {
        name: "Make room…",
      }),
    );
    await screen.findByTestId("select-none");
    await user.click(await screen.findByTestId("skill-notes"));
    await user.click(screen.getByTestId("skill-lint"));

    const acts = within(screen.getByTestId("select-acts"));
    expect(acts.getAllByRole("row").map((row) => row.getAttribute("data-testid"))).toEqual([
      "select-act-turn-off",
      "select-act-pin",
      "select-act-archive",
    ]);
    await user.click(acts.getByRole("button", { name: "Turn off…" }));
    const review = await screen.findByTestId("set-review");
    await user.click(await within(review).findByRole("button", { name: "Turn off" }));

    await waitFor(() => {
      const turned = asked.filter((one) => one.cmd === "skill_set_turn");
      expect(turned).toHaveLength(1);
      expect(turned[0].args.names).toEqual(["notes", "lint"]);
    });
  });
});

describe("what Make room says and how it orders", () => {
  it("says a skill's runs of the last 30 days, and none for a skill with none", () => {
    expect(usesSaid(undefined)).toBe("No runs in the last 30 days");
    expect(usesSaid(1)).toBe("1 run in the last 30 days");
    expect(usesSaid(5)).toBe("5 runs in the last 30 days");
  });

  it("counts only the skills hidden over the budget, never one an extension's router hid for its own reason", () => {
    const byRouter: SkillHiddenOn[] = [
      {
        harness: "claude",
        title: "Claude Code",
        hidden: {
          router: { extension: "relevance" },
          why: { "by-router": "off topic" },
          said: "Hidden.",
        },
      },
    ];
    expect(
      hiddenCount([skill("a", { hidden: HIDDEN }), skill("b", { hidden: byRouter }), skill("c")]),
    ).toBe(1);
  });

  it("puts the least used first and keeps the list's order between equals", () => {
    const uses = new Map([
      ["b", 3],
      ["c", 1],
    ]);
    expect(leastUsedFirst(["a", "b", "c", "d"], (name) => name, uses)).toEqual([
      "a",
      "d",
      "c",
      "b",
    ]);
  });

  it("says which programs an install would take past their budget, and none within it", () => {
    const counted = { listed: 2, hidden: 0 };
    expect(installGoesOver([{ program: CLAUDE, counted }], 1)).toEqual([]);
    expect(
      installGoesOver(
        [{ program: { ...CLAUDE, skills: 0 }, counted: { listed: 0, hidden: 0 } }],
        1,
      ),
    ).toEqual([
      "Installing these puts 1 skill in a chat's list on Claude Code, past its budget of 0.",
    ]);
    expect(installGoesOver([{ program: CLAUDE, counted }], 2)).toEqual([
      "Installing these puts 4 skills in a chat's list on Claude Code, past its budget of 3.",
    ]);
    expect(installGoesOver([{ program: CLAUDE, counted: { listed: 3, hidden: 2 } }], 1)).toEqual([
      "Installing these puts 6 skills in a chat's list on Claude Code, past its budget of 3.",
    ]);
  });
});
