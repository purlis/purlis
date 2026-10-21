// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  SkillsChip,
  UseSkill,
  UseSkillOpener,
  budgetSaid,
  overSaid,
  skillsMatching,
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
    ...more,
  };
}

const BUDGET: SkillBudget = {
  skills: 3,
  chars: 1234,
  manual: 1,
  most_skills: 30,
  most_chars: 6000,
  over: null,
};

function skills(listed: ChatSkill[], budget: SkillBudget | null = BUDGET): ChatSkills {
  return { on: true, harness: "Claude Code", skills: listed, budget };
}

function opened(listed: ChatSkills | undefined, onUse = vi.fn()) {
  render(
    <UseSkill chat="steward 3" skills={listed} busy={false} onUse={onUse} onCancel={vi.fn()} />,
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
    expect(within(rows[2]).getByText("manual only")).toBeTruthy();
    expect(within(rows[0]).queryByText("manual only")).toBeNull();
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
    opened(skills(listed));
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "nothing like it" } });
    expect(screen.getByText("No skill of this chat matches that.")).toBeTruthy();
  });

  it("warns of a set past the budget, saying which part", () => {
    expect(overSaid(BUDGET)).toBeUndefined();
    expect(overSaid({ ...BUDGET, over: "skills" })).toMatch(
      /too many skills for purlis's skill budget/,
    );
    expect(overSaid({ ...BUDGET, over: "chars" })).toMatch(/too much description/);
    opened(skills([skill("deploy")], { ...BUDGET, over: "both" }));
    expect(screen.getByRole("status").textContent).toMatch(
      /too many skills and too much description/,
    );
    expect(budgetSaid({ ...BUDGET, manual: 0 })).toBe("3 skills of 30 · 1,234 of 6,000 characters");
  });

  it("says why a chat has nothing to offer, and that it is still reading", () => {
    opened(undefined);
    expect(screen.getByText("Reading this chat's skills…")).toBeTruthy();
    cleanup();
    opened({ on: true, harness: null, skills: [], budget: null });
    expect(screen.getByTestId("use-skill-none").textContent).toMatch(/runs no tool purlis knows/);
    cleanup();
    opened(skills([]));
    expect(screen.getByTestId("use-skill-none").textContent).toMatch(/Open Skill studio/);
  });
});

describe("the Skills chip on a chat's pane", () => {
  function answering(budget: SkillBudget | null) {
    mockIPC((cmd) => {
      if (cmd === "skill_studio_on") return true;
      if (cmd === "chat_skills") return { on: true, harness: "Codex", skills: [], budget };
      throw new Error(`unexpected ${cmd}`);
    });
  }

  it("shows the chat's skills at a glance and opens Use skill… for it", async () => {
    answering(BUDGET);
    const open = vi.fn();
    render(
      <UseSkillOpener value={open}>
        <SkillsChip plane="p" session={4} />
      </UseSkillOpener>,
    );
    const chip = await screen.findByTestId("pane-skills");
    expect(chip.textContent).toBe("4 skills");
    expect(chip.className).not.toMatch(/\bover\b/);
    fireEvent.click(chip);
    expect(open).toHaveBeenCalledWith(4);
  });

  it("is in the warning colour, saying why, for a set past the budget", async () => {
    answering({ ...BUDGET, over: "chars" });
    render(<SkillsChip plane="p" session={4} />);
    const chip = await screen.findByTestId("pane-skills");
    expect(chip.className).toMatch(/\bover\b/);
    expect(chip.getAttribute("aria-label")).toMatch(/too much description/);
  });
});
