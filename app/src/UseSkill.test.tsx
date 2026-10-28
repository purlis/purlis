// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { renderHook, waitFor } from "@testing-library/react";
import {
  SkillBudgetsHere,
  SkillsChip,
  UseSkill,
  UseSkillOpener,
  budgetSaid,
  budgetShort,
  pinsTogetherSaid,
  skillsMatching,
  useChatSkillBudgets,
  warnsSaid,
} from "./UseSkill";
import type { ChatSkill, ChatSkills, SkillBudget } from "./bindings";

afterEach(() => {
  cleanup();
  clearMocks();
});

function skill(name: string, more: Partial<ChatSkill> = {}): ChatSkill {
  return {
    name,
    level: "project",
    label: "project",
    description: `Does ${name}.`,
    manual: false,
    arguments: [],
    hint: null,
    hidden: null,
    ...more,
    takes_arguments:
      (more.arguments !== undefined && more.arguments.length > 0) ||
      (more.hint !== undefined && more.hint !== null),
  };
}

const BUDGET: SkillBudget = {
  skills: 3,
  chars: 1234,
  manual: 1,
  most_skills: 30,
  most_chars: 6000,
  hidden: 0,
  still_over: null,
  pinned_by: [],
  pins_together: null,
  warns: null,
};

/** #3391: forty of the workspace's pins and thirty of the persona's, past the 64 a chat reads. */
const TOGETHER: SkillBudget = {
  ...BUDGET,
  pinned_by: [
    { at: "workspace", name: "ops" },
    { at: "persona", name: "reviewer" },
  ],
  pins_together: {
    held: [
      { by: { at: "workspace", name: "ops" }, pins: 40 },
      { by: { at: "persona", name: "reviewer" }, pins: 30 },
    ],
    most: 64,
  },
};

function skills(listed: ChatSkill[], budget: SkillBudget | null = BUDGET): ChatSkills {
  return {
    on: true,
    known: true,
    skills: listed,
    budget,
    repos: [],
    not_supported: null,
    routing: null,
  };
}

function opened(listed: ChatSkills | undefined, onUse = vi.fn(), onOpenStudio = vi.fn()) {
  render(
    <UseSkill
      chat="steward 3"
      skills={listed}
      busy={false}
      onUse={onUse}
      onCancel={vi.fn()}
      onOpenStudio={onOpenStudio}
    />,
  );
  return onUse;
}

describe("Use skill…", () => {
  it("lists exactly the chat's skills, the manual-only ones marked, with its budget above", () => {
    opened(
      skills([
        skill("deploy"),
        skill("handoff", { level: "built-in", label: "built-in" }),
        skill("release", { manual: true }),
      ]),
    );
    const rows = screen.getAllByTestId("use-skill-row");
    expect(rows.map((row) => row.querySelector(".use-skill-name")?.textContent)).toEqual([
      "deploy",
      "handoff",
      "release",
    ]);
    expect(within(rows[2]).getByText("· manual only")).toBeTruthy();
    expect(within(rows[0]).queryByText("· manual only")).toBeNull();
    // Each skill is named by its name alone, and its level is its description (#2911).
    expect(screen.getByRole("option", { name: "deploy" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "release" })).toHaveAccessibleDescription(
      /manual only/,
    );
    expect(screen.getByTestId("use-skill-budget").textContent).toBe(
      "3 skills of 30 · 1,234 of 6,000 characters · 1 manual-only skill free",
    );
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("types a skill that takes nothing at once", () => {
    const onUse = opened(skills([skill("deploy")]));
    fireEvent.click(screen.getByTestId("use-skill-row"));
    expect(onUse).toHaveBeenCalledWith("deploy", []);
  });

  it("asks for each declared argument in order before it types the skill", () => {
    const onUse = opened(
      skills([skill("review", { arguments: ["pr", "focus"], hint: "<pr> [focus]" })]),
    );
    fireEvent.click(screen.getByTestId("use-skill-row"));
    expect(screen.getByRole("heading").textContent).toBe("Use review");
    const boxes = screen.getAllByRole("textbox");
    expect(boxes).toHaveLength(2);
    expect(boxes[0].getAttribute("placeholder")).toBe("<pr> [focus]");
    fireEvent.change(boxes[0], { target: { value: "42" } });
    fireEvent.change(boxes[1], { target: { value: "the tests" } });
    fireEvent.click(screen.getByRole("button", { name: "Type it" }));
    expect(onUse).toHaveBeenCalledWith("review", ["42", "the tests"]);
  });

  it("gives a skill that only says what to pass one box with that hint", () => {
    const onUse = opened(skills([skill("triage", { hint: "[issue number]" })]));
    fireEvent.click(screen.getByTestId("use-skill-row"));
    expect(screen.getAllByRole("textbox")).toHaveLength(1);
    expect(screen.getByText("[issue number]")).toBeTruthy();
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "12" } });
    fireEvent.click(screen.getByRole("button", { name: "Type it" }));
    expect(onUse).toHaveBeenCalledWith("triage", ["12"]);
  });

  it("finds a skill by any words of its name or description", () => {
    const listed = [skill("deploy", { description: "Ships the app." }), skill("review")];
    expect(skillsMatching(listed, "SHIPS app").map((one) => one.name)).toEqual(["deploy"]);
    expect(skillsMatching(listed, "").map((one) => one.name)).toEqual(["deploy", "review"]);
    // A list over seven gets the family's filter (#2910), its total in its placeholder.
    const many = [...listed, ...["a", "b", "c", "d", "e", "f"].map((name) => skill(name))];
    opened(skills(many));
    const filter = screen.getByRole("searchbox", { name: "Filter skills" });
    expect(filter.getAttribute("placeholder")).toBe("Filter 8 skills");
    fireEvent.change(filter, { target: { value: "nothing like it" } });
    expect(screen.getByText("No skill is called “nothing like it”.")).toBeTruthy();
  });

  it("draws no filter over a list of seven or fewer", () => {
    opened(skills([skill("deploy"), skill("review")]));
    expect(screen.queryByRole("searchbox")).toBeNull();
  });

  it("warns of a set near or past the budget, saying which part", () => {
    expect(warnsSaid(BUDGET)).toBeUndefined();
    expect(warnsSaid({ ...BUDGET, warns: "skills" })).toMatch(
      /This chat's skills are near or past purlis's skill budget/,
    );
    expect(warnsSaid({ ...BUDGET, warns: "chars" })).toMatch(/descriptions are near or past/);
    opened(skills([skill("deploy")], { ...BUDGET, warns: "both" }));
    expect(screen.getByRole("status").textContent).toMatch(
      /This chat's skills and their descriptions are near or past/,
    );
    expect(budgetSaid({ ...BUDGET, manual: 0 })).toBe("3 skills of 30 · 1,234 of 6,000 characters");
    expect(budgetShort(BUDGET)).toBe("3 of 30 skills");
    expect(budgetShort({ ...BUDGET, warns: "chars" })).toBe("3 of 30 skills, near the budget");
    expect(budgetShort({ ...BUDGET, skills: 1, most_skills: 1 })).toBe("1 of 1 skill");
  });

  it("says a workspace's and a persona's pins pass 64 together, each studio one press away (#3391)", () => {
    expect(pinsTogetherSaid(BUDGET)).toBeUndefined();
    expect(pinsTogetherSaid(TOGETHER)).toBe(
      "This chat reads none of its pins: workspace ops pins 40 skills and persona reviewer pins 30 skills, more than the 64 a chat reads together. Unpin some in Skill studio · ops or Skill studio · reviewer.",
    );
    const openStudio = vi.fn();
    opened(skills([skill("deploy")], TOGETHER), vi.fn(), openStudio);
    const line = screen.getByTestId("use-skill-pins-together");
    expect(line.getAttribute("role")).toBe("status");
    expect(line.textContent).toMatch(
      /workspace ops pins 40 skills and persona reviewer pins 30 skills/,
    );
    fireEvent.click(within(line).getByRole("button", { name: "Open Skill studio · reviewer" }));
    expect(openStudio).toHaveBeenCalledWith({ at: "persona", name: "reviewer" });
    fireEvent.click(within(line).getByRole("button", { name: "Open Skill studio · ops" }));
    expect(openStudio).toHaveBeenLastCalledWith({ at: "workspace", name: "ops" });
  });

  it("says how the chat's router extension did at its start, and why it fell back (SK-15b)", () => {
    opened({
      ...skills([skill("deploy")]),
      routing: {
        router: "relevance",
        ms: 34,
        fell_back: false,
        said: "Routed by the router relevance in 34 ms.",
      },
    });
    const taken = screen.getByTestId("use-skill-routing");
    expect(taken.textContent).toBe("Routed by the router relevance in 34 ms.");
    expect(taken.getAttribute("role")).toBeNull();
    cleanup();
    opened({
      ...skills([skill("deploy")]),
      routing: {
        router: "relevance",
        ms: 100,
        fell_back: true,
        said: "The router relevance did not answer within 100 ms, so this chat lists what the built-in router shows.",
      },
    });
    const fell = screen.getByTestId("use-skill-routing");
    expect(fell.getAttribute("role")).toBe("status");
    expect(fell.textContent).toMatch(/did not answer within 100 ms/);
    cleanup();
    opened(skills([skill("deploy")]));
    expect(screen.queryByTestId("use-skill-routing")).toBeNull();
  });

  it("says how many skills the router hid behind the index skill, and marks each (SK-15)", () => {
    const over = { ...BUDGET, warns: "skills" as const, hidden: 2 };
    expect(budgetSaid({ ...over, manual: 0 })).toBe(
      "3 skills of 30 · 1,234 of 6,000 characters · 2 skills in the index skill",
    );
    expect(warnsSaid(over)).toMatch(/so 2 skills are hidden behind the index skill/);
    expect(
      warnsSaid({ ...over, still_over: "pins", pinned_by: [{ at: "workspace", name: "ops" }] }),
    ).toBe(
      "This chat's pinned skills are past purlis's skill budget, so all of them are listed. Unpin some in Skill studio · ops, or make some manual-only.",
    );
    // Both pin: the way out names each (SK-15d).
    expect(
      warnsSaid({
        ...over,
        still_over: "pins",
        pinned_by: [
          { at: "workspace", name: "ops" },
          { at: "persona", name: "reviewer" },
        ],
      }),
    ).toMatch(/Unpin some in Skill studio · ops or Skill studio · reviewer, or make some/);
    expect(warnsSaid({ ...over, still_over: "not-hideable" })).toMatch(
      /purlis's own skills and the ones this chat's program and repos bring are past/,
    );
    opened(
      skills(
        [
          skill("deploy"),
          skill("review", {
            hidden: {
              router: "built-in",
              why: "over-budget",
              said: "hidden by the built-in router",
            },
          }),
        ],
        over,
      ),
    );
    const rows = screen.getAllByTestId("use-skill-row");
    expect(within(rows[1]).getByText("· in the index skill")).toBeTruthy();
    expect(within(rows[0]).queryByText("· in the index skill")).toBeNull();
  });

  it("says why a chat has nothing to offer, and that it is still reading", () => {
    opened(undefined);
    expect(screen.getByText("Reading this chat's skills…")).toBeTruthy();
    cleanup();
    opened({
      on: true,
      known: false,
      skills: [],
      budget: null,
      repos: [],
      not_supported: null,
      routing: null,
    });
    expect(screen.getByTestId("use-skill-none").textContent).toMatch(/runs no tool purlis knows/);
    cleanup();
    // A declared harness skills are not supported on says so, in the core's one line (SK-30).
    const goose =
      "Skills are not supported on Goose: it has no skill format purlis can hand skills by, and its declaration does not say it takes MCP servers, so its chats get no skills.";
    opened({
      on: true,
      known: false,
      skills: [],
      budget: null,
      repos: [],
      not_supported: goose,
      routing: null,
    });
    expect(screen.getByTestId("use-skill-none").textContent).toBe(goose);
    cleanup();
    opened(skills([]));
    expect(screen.getByTestId("use-skill-none").textContent).toMatch(/Open Skill studio/);
  });
});

describe("the Skills chip on a chat's pane", () => {
  function studioOn() {
    mockIPC((cmd) => {
      if (cmd === "skill_studio_on") return true;
      throw new Error(`unexpected ${cmd}`);
    });
  }

  it("shows the chat's skills at a glance and opens Use skill… for it", async () => {
    studioOn();
    const open = vi.fn();
    render(
      <UseSkillOpener value={open}>
        <SkillBudgetsHere.Provider value={{ 4: BUDGET }}>
          <SkillsChip session={4} />
          <SkillsChip session={5} />
        </SkillBudgetsHere.Provider>
      </UseSkillOpener>,
    );
    const chip = await screen.findByTestId("pane-skills");
    expect(screen.getAllByTestId("pane-skills")).toHaveLength(1);
    expect(chip.textContent).toBe("4 skills");
    expect(chip.hasAttribute("data-over")).toBe(false);
    fireEvent.click(chip);
    expect(open).toHaveBeenCalledWith(4);
  });

  it("is in the warning look, saying why, for a set near or past the budget", async () => {
    studioOn();
    render(
      <SkillBudgetsHere.Provider value={{ 4: { ...BUDGET, warns: "chars" } }}>
        <SkillsChip session={4} />
      </SkillBudgetsHere.Provider>,
    );
    const chip = await screen.findByTestId("pane-skills");
    expect(chip.hasAttribute("data-over")).toBe(true);
    // A manual-only skill is in the set's count, free in the listing's.
    expect(chip.textContent).toBe("4 skills");
    expect(chip.getAttribute("aria-label")).toMatch(/descriptions are near or past/);
  });

  it("is in the warning look for pins past 64 together, under the budget or not (#3391)", async () => {
    studioOn();
    render(
      <SkillBudgetsHere.Provider value={{ 4: TOGETHER }}>
        <SkillsChip session={4} />
      </SkillBudgetsHere.Provider>,
    );
    const chip = await screen.findByTestId("pane-skills");
    expect(chip.hasAttribute("data-over")).toBe(true);
    expect(chip.getAttribute("aria-label")).toMatch(
      /^This chat reads none of its pins: workspace ops pins 40 skills and persona reviewer pins 30/,
    );
  });

  it("reads each open chat's budget once, for the chip and the tab's menu row alike", async () => {
    const asked: number[] = [];
    mockIPC((cmd, args) => {
      if (cmd !== "chat_skills") throw new Error(`unexpected ${cmd}`);
      const session = (args as { session: number }).session;
      asked.push(session);
      return { on: true, known: true, skills: [], budget: { ...BUDGET, skills: session } };
    });
    const { result, rerender } = renderHook(
      ({ sessions }) => useChatSkillBudgets("p", sessions, true),
      { initialProps: { sessions: [4, 5] } },
    );
    await waitFor(() => expect(Object.keys(result.current[0])).toEqual(["4", "5"]));
    rerender({ sessions: [4, 5, 6] });
    await waitFor(() => expect(result.current[0][6]?.skills).toBe(6));
    expect(asked.sort()).toEqual([4, 5, 6]);
  });
});
