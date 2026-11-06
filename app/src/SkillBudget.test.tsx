// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SkillBudgetPreview, StudioBudget } from "./bindings";
import {
  BUDGET_INVALID,
  budgetCostSaid,
  budgetTyped,
  SkillBudgetTable,
  writeBudgetTo,
} from "./SkillBudget";

const CLAUDE: StudioBudget = {
  program: "Claude Code",
  key: "claude",
  skills: 100,
  chars: 15000,
  skills_said: "100, this workspace · project says 70 · Claude Code's default is 60",
  chars_said: "15,000, Claude Code's default",
  skills_here: 100,
  chars_here: null,
};

const NOTHING_MOVES: SkillBudgetPreview = {
  listed: 16,
  hidden: 0,
  now_listed: [],
  now_hidden: [],
  more_tokens: 0,
  past_limit: null,
  over: null,
};

type Asked = { cmd: string; args: Record<string, unknown> };

function faked(
  preview: (args: Record<string, unknown>) => SkillBudgetPreview = () => NOTHING_MOVES,
) {
  const asked: Asked[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args: args as Record<string, unknown> });
    if (cmd === "skill_budget_preview") return preview(args as Record<string, unknown>);
    if (cmd === "skill_budget_set") return true;
    return null;
  });
  return asked;
}

function table(level = { workspace: "alpha", persona: null as string | null }) {
  return render(
    <SkillBudgetTable
      plane="p"
      budget={[CLAUDE]}
      level={level}
      onWrite={writeBudgetTo("p", level)}
    />,
  );
}

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("a typed skill budget", () => {
  it("is a number of at least 1, or nothing for the number below, and has no maximum", () => {
    expect(budgetTyped("100")).toBe(100);
    expect(budgetTyped(" 7 ")).toBe(7);
    expect(budgetTyped("")).toBeNull();
    expect(budgetTyped("0")).toBeUndefined();
    expect(budgetTyped("-3")).toBeUndefined();
    expect(budgetTyped("1.5")).toBeUndefined();
    expect(budgetTyped("many")).toBeUndefined();
    expect(budgetTyped("99999999999999")).toBe(4_294_967_295);
  });

  it("says its cost: what is listed and hidden, by name, and about how many tokens", () => {
    expect(
      budgetCostSaid({
        ...NOTHING_MOVES,
        listed: 12,
        hidden: 8,
        now_hidden: ["a", "b", "c", "d", "e", "f", "g"],
        more_tokens: -1200,
      }),
    ).toEqual([
      "New chats list 12 skills and hide 8.",
      "Now hidden: a, b, c, d, e and 2 more.",
      "About 1,200 fewer tokens per chat.",
    ]);
    expect(budgetCostSaid({ ...NOTHING_MOVES, now_listed: ["a"], more_tokens: 91 })).toEqual([
      "New chats list 16 skills and hide 0.",
      "Now listed: a.",
      "About 91 more tokens per chat.",
    ]);
    expect(budgetCostSaid(NOTHING_MOVES)).toContain("No change in what each chat spends.");
    expect(budgetCostSaid({ ...NOTHING_MOVES, more_tokens: 1 })).toContain(
      "About 1 more token per chat.",
    );
  });
});

describe("the skill budget table", () => {
  it("says where each number comes from", () => {
    faked();
    table();
    expect(screen.getByTestId("skill-budget-skills-claude")).toHaveTextContent(
      "100, this workspace · project says 70 · Claude Code's default is 60",
    );
    expect(screen.getByTestId("skill-budget-chars-claude")).toHaveTextContent(
      "15,000, Claude Code's default",
    );
  });

  it("resets one number at this level and keeps the other", async () => {
    const asked = faked();
    table();
    await userEvent.click(
      screen.getByRole("button", { name: "Reset to default, Claude Code's most skills" }),
    );
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_budget_set")?.args).toMatchObject({
        workspace: "alpha",
        program: "claude",
        skills: null,
        chars: null,
      }),
    );
  });

  it("offers no reset of a number this level does not write", () => {
    faked();
    table();
    expect(
      screen.getByRole("button", { name: "Reset to default, Claude Code's most characters" }),
    ).toHaveAttribute("aria-disabled", "true");
  });

  it("says a reset the core refused", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_budget_set") throw "This project is read-only.";
      return NOTHING_MOVES;
    });
    table();
    await userEvent.click(
      screen.getByRole("button", { name: "Reset to default, Claude Code's most skills" }),
    );
    expect(await screen.findByTestId("skill-budget-refused")).toHaveTextContent(
      "This project is read-only.",
    );
  });
});

describe("Change…", () => {
  async function opened() {
    await userEvent.click(
      screen.getByRole("button", { name: "Change Claude Code's skill budget" }),
    );
    return within(await screen.findByTestId("skill-budget-change"));
  }

  it("starts from what this level writes and leaves the rest empty", async () => {
    faked();
    table();
    const dialog = await opened();
    expect(dialog.getByRole("textbox", { name: "Most skills in a chat's list" })).toHaveValue(
      "100",
    );
    expect(dialog.getByRole("textbox", { name: "Most characters in a chat's list" })).toHaveValue(
      "",
    );
  });

  it("says the cost of what is typed, asking for the numbers typed", async () => {
    const asked = faked((args) =>
      args.skills === 4
        ? { ...NOTHING_MOVES, listed: 4, hidden: 12, now_hidden: ["deploy"], more_tokens: -700 }
        : NOTHING_MOVES,
    );
    table();
    const dialog = await opened();
    const box = dialog.getByRole("textbox", { name: "Most skills in a chat's list" });
    await userEvent.clear(box);
    await userEvent.type(box, "4");
    await waitFor(() =>
      expect(dialog.getByTestId("skill-budget-cost")).toHaveTextContent(
        "New chats list 4 skills and hide 12.",
      ),
    );
    expect(dialog.getByTestId("skill-budget-cost")).toHaveTextContent("Now hidden: deploy.");
    expect(dialog.getByTestId("skill-budget-cost")).toHaveTextContent(
      "About 700 fewer tokens per chat.",
    );
    expect(asked.at(-1)?.args).toMatchObject({ workspace: "alpha", program: "claude", skills: 4 });
  });

  it("warns past a program's limit and below the pins, and still takes the number", async () => {
    faked(() => ({
      ...NOTHING_MOVES,
      past_limit: "Codex: at 9,000 characters this is past the 8,000 it holds.",
      over: "3 pinned skills are still listed, so a chat's list goes over this.",
    }));
    table();
    const dialog = await opened();
    expect(await dialog.findByTestId("skill-budget-past-limit")).toHaveTextContent("Codex");
    expect(dialog.getByTestId("skill-budget-over")).toHaveTextContent("3 pinned skills");
    const box = dialog.getByRole("textbox", { name: "Most skills in a chat's list" });
    await userEvent.clear(box);
    await userEvent.type(box, "2");
    expect(dialog.getByRole("button", { name: "Change budget" })).not.toHaveAttribute(
      "aria-disabled",
      "true",
    );
  });

  it("will not save a box that holds no number, and says why", async () => {
    faked();
    table();
    const dialog = await opened();
    const box = dialog.getByRole("textbox", { name: "Most skills in a chat's list" });
    await userEvent.clear(box);
    await userEvent.type(box, "0");
    await userEvent.tab();
    expect(await dialog.findByText(BUDGET_INVALID)).toBeInTheDocument();
    await waitFor(() =>
      expect(dialog.getByTestId("skill-budget-cost")).not.toHaveTextContent("New chats list"),
    );
    expect(dialog.getByRole("button", { name: "Change budget" })).toBeDisabled();
  });

  it("writes this level's numbers and closes", async () => {
    const asked = faked();
    table();
    const dialog = await opened();
    const box = dialog.getByRole("textbox", { name: "Most characters in a chat's list" });
    await userEvent.type(box, "20000");
    await userEvent.click(dialog.getByRole("button", { name: "Change budget" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_budget_set")?.args).toMatchObject({
        workspace: "alpha",
        program: "claude",
        skills: 100,
        chars: 20000,
      }),
    );
    await waitFor(() => expect(screen.queryByTestId("skill-budget-change")).toBeNull());
  });

  it("stays open and says why where the core refuses", async () => {
    const onWrite = vi.fn((_write, told: { refused: (why: string) => void }) =>
      told.refused("It changed on disk."),
    );
    faked();
    render(
      <SkillBudgetTable
        plane="p"
        budget={[CLAUDE]}
        level={{ workspace: null, persona: null }}
        onWrite={onWrite}
      />,
    );
    const dialog = await opened();
    await userEvent.type(
      dialog.getByRole("textbox", { name: "Most characters in a chat's list" }),
      "9",
    );
    await userEvent.click(dialog.getByRole("button", { name: "Change budget" }));
    expect(await dialog.findByTestId("skill-budget-change-refused")).toHaveTextContent(
      "It changed on disk.",
    );
  });
});
