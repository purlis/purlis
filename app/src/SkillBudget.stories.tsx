// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A program's skill budget** (#5353): the table the Routing page and Settings draw, and its
 * Change… dialog with the cost as it is typed. Walked by the keyboard, held to axe, and pictured
 * in every pictured theme cell. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SkillBudgetPreview, StudioBudget } from "./bindings";
import { ReadOnlySaid } from "./ReadOnlyLine";
import { Button } from "./ui/button/Button";
import { SkillBudgetTable } from "./SkillBudget";
import { expectFocusInSight, untilFocused, untilFocusWithin } from "./stories.testkit";

const BUDGET: StudioBudget[] = [
  {
    program: "Claude Code",
    key: "claude",
    skills: 100,
    chars: 15000,
    skills_said: "100, this workspace · project says 70 · Claude Code's default is 60",
    chars_said: "15,000, Claude Code's default",
    skills_here: 100,
    chars_here: null,
  },
  {
    program: "Codex",
    key: "codex",
    skills: 80,
    chars: 9000,
    skills_said: "80, the project, on every program · Codex's default is 30",
    chars_said: "9,000, this workspace · Codex's default is 6,000",
    skills_here: null,
    chars_here: 9000,
  },
  {
    program: "opencode",
    key: "opencode",
    skills: 30,
    chars: 6000,
    skills_said: "30, opencode's default",
    chars_said: "6,000, opencode's default",
    skills_here: null,
    chars_here: null,
  },
];

const RISES: SkillBudgetPreview = {
  listed: 24,
  hidden: 0,
  now_listed: ["deploy", "figma-handoff", "gpu-profile", "lint-rs", "review", "smart-close", "tdd"],
  now_hidden: [],
  more_tokens: 2184,
  past_limit: null,
  over: null,
};

const WARNS: SkillBudgetPreview = {
  listed: 3,
  hidden: 21,
  now_listed: [],
  now_hidden: ["deploy", "review"],
  more_tokens: -1200,
  past_limit:
    "Codex: at 9,000 characters this is past the 8,000 it holds where it doesn't know its window (2% of the window otherwise). Past its limit Codex leaves skills out of its list.",
  over: "3 pinned skills are still listed, so a chat's list goes over this.",
};

function Budget({
  extensionRouter,
  refuses,
  readOnly,
  followed,
  width = "46rem",
}: {
  /** What the core answers for what is typed: the story's `beforeEach` fakes it. */
  preview: SkillBudgetPreview;
  extensionRouter: boolean;
  refuses?: string;
  readOnly?: string;
  /** A control after the table, for a Tab to land on as it leaves. */
  followed?: boolean;
  /** The room it has: a table from 32rem, a stacked form below that. */
  width?: string;
}) {
  return (
    <ReadOnlySaid.Provider value={readOnly}>
      <div style={{ inlineSize: width }}>
        <SkillBudgetTable
          plane="p"
          budget={BUDGET}
          level={{ workspace: "smart-ide", persona: null }}
          extensionRouter={extensionRouter}
          onWrite={(_write, told) => (refuses === undefined ? told.done() : told.refused(refuses))}
        />
      </div>
      {followed && <Button>After the table</Button>}
    </ReadOnlySaid.Provider>
  );
}

const meta = {
  title: "Skill studio/Skill budget",
  component: Budget,
  args: { preview: RISES, extensionRouter: false },
  beforeEach: ({ args }) => {
    mockIPC((cmd) => (cmd === "skill_budget_preview" ? args.preview : null));
    // A mock left behind would answer the next story's font asks, in this worker, with nothing.
    return () => {
      clearMocks();
    };
  },
} satisfies Meta<typeof Budget>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Moves along the grid until the button called `name` has the keyboard. */
async function arrowTo(name: string): Promise<void> {
  for (let step = 0; step < 12; step++) {
    if (document.activeElement?.getAttribute("aria-label") === name) return;
    await userEvent.keyboard("{ArrowRight}");
  }
  throw new Error(`the arrows do not reach ${name}`);
}

/**
 * **The table**: where each number comes from, a reset where this level writes one, and Change…
 * on each row. The arrows reach Change…, Enter opens it, Esc gives the keyboard back.
 */
export const Table: Story = {
  args: { followed: true },
  play: async ({ canvas }) => {
    const grid = canvas.getByRole("grid", { name: "Skill budget on each program" });
    await userEvent.tab();
    await untilFocusWithin(() => grid, "the keyboard reaches the table by Tab");
    await arrowTo("Change Claude Code's skill budget");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    const dialog = await within(document.body).findByRole("dialog", {
      name: "Change Claude Code's skill budget",
    });
    await untilFocused(
      () => within(dialog).getByRole("textbox", { name: "Most skills in a chat's list" }),
      "Change… opens on its first box",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(within(document.body).queryByRole("dialog")).toBeNull());
    await untilFocusWithin(() => grid, "Esc gives the keyboard back to the table");
    // Tab is not trapped in the table. This walks a simulated Tab, which does not take React
    // Aria's way out of a grid as a browser's own does, so it says "gets out", not "in one press".
    const after = canvas.getByRole("button", { name: "After the table" });
    for (let step = 0; step < 12 && document.activeElement !== after; step++) {
      await userEvent.tab();
    }
    await untilFocused(() => after, "Tab leaves the table for what follows it");
  },
};

/** **The cost, as it is typed**: what new chats would list, by name, and about how many tokens. */
export const WhatItCosts: Story = {
  parameters: { screenshot: "page" },
  play: async ({ canvas }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Change opencode's skill budget" }));
    const dialog = await within(document.body).findByRole("dialog");
    await userEvent.keyboard("100");
    await waitFor(() =>
      expect(within(dialog).getByTestId("skill-budget-cost")).toHaveTextContent(
        "About 2,184 more tokens per chat.",
      ),
    );
    await userEvent.tab();
    expectFocusInSight();
  },
};

/** **Past what the program holds, and below the pins**: both warn, and neither stops the change. */
export const ItWarns: Story = {
  args: { preview: WARNS, extensionRouter: true },
  parameters: { screenshot: "page" },
  play: async ({ canvas }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Change Codex's skill budget" }));
    const dialog = await within(document.body).findByRole("dialog");
    await expect(await within(dialog).findByTestId("skill-budget-past-limit")).toBeVisible();
    await expect(within(dialog).getByTestId("skill-budget-over")).toBeVisible();
    await expect(within(dialog).getByTestId("skill-budget-by-the-built-in-router")).toBeVisible();
    await userEvent.keyboard("3");
    await expect(within(dialog).getByRole("button", { name: "Change budget" })).toBeEnabled();
  },
};

/** **A box that holds no number**: it says so when it is left, and the change waits. */
export const NotANumber: Story = {
  parameters: { screenshot: "page" },
  play: async ({ canvas }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Change opencode's skill budget" }));
    const dialog = await within(document.body).findByRole("dialog");
    await userEvent.keyboard("0");
    await userEvent.tab();
    await waitFor(() =>
      expect(within(dialog).getByText(/whole number of at least 1/)).toBeVisible(),
    );
    expectFocusInSight();
  },
};

/** **Reset to default, by the keyboard, and the core's refusal said under the table.** */
export const AResetRefused: Story = {
  args: { refuses: "workspace.json changed on disk since this tab read it, so nothing was saved." },
  play: async ({ canvas }) => {
    const grid = canvas.getByRole("grid", { name: "Skill budget on each program" });
    await userEvent.tab();
    await untilFocusWithin(() => grid, "the keyboard reaches the table by Tab");
    await arrowTo("Reset to default, Claude Code's most skills");
    await userEvent.keyboard("{Enter}");
    await expect(await canvas.findByTestId("skill-budget-refused")).toBeVisible();
    expectFocusInSight();
  },
};

/** **A read-only project**: every button stays a Tab stop that says why, and does nothing. */
export const ReadOnly: Story = {
  args: { readOnly: "This project is read-only here, so purlis writes none of its files." },
  play: async ({ canvas }) => {
    const change = canvas.getByRole("button", { name: "Change Codex's skill budget" });
    await expect(change).toHaveAttribute("aria-disabled", "true");
    await userEvent.tab();
    await arrowTo("Change Claude Code's skill budget");
    expectFocusInSight();
  },
};

/** **Where its room is narrow**, as on the Routing page beside a list: a form, each number's words
 *  whole, its reset under them, and Change… on a line of its own. */
export const Stacked: Story = {
  args: { width: "20rem" },
  play: async ({ canvas }) => {
    await userEvent.tab();
    await arrowTo("Change Claude Code's skill budget");
    await expect(
      canvas.getByRole("button", { name: "Change Claude Code's skill budget" }),
    ).toBeVisible();
    expectFocusInSight();
  },
};
