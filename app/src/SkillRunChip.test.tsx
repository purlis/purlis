// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillRunChip } from "./SkillRunChip";
import { SkillTab } from "./SkillTab";
import { SkillRunsHere, fixedRuns, skillOfRun, toldRun, wantRun } from "./skillRuns";

afterEach(() => {
  cleanup();
  clearMocks();
});

const RUN = "01JBRUN0000000000000000001";

function studioOn() {
  mockIPC((cmd) => {
    if (cmd === "skill_studio_on") return true;
    throw new Error(`unexpected ${cmd}`);
  });
}

describe("the skill run chip on a chat's tab", () => {
  it("shows the skill in use and its tool calls, and opens the skill's tab on that run", async () => {
    studioOn();
    const onOpen = vi.fn();
    render(
      <SkillRunsHere.Provider
        value={fixedRuns({ 4: { skill: "purlis:deploy", id: RUN, tool_calls: 3 } })}
      >
        <SkillRunChip session={4} onOpen={onOpen} />
        <SkillRunChip session={5} onOpen={onOpen} />
      </SkillRunsHere.Provider>,
    );

    const chip = await screen.findByTestId("tab-skill-run");
    expect(chip.getAttribute("aria-label")).toBe(
      "Using the skill purlis:deploy, 3 tool calls so far. Open its runs",
    );
    expect(screen.getAllByTestId("tab-skill-run")).toHaveLength(1);
    fireEvent.click(chip);
    expect(onOpen).toHaveBeenCalledWith("purlis:deploy", RUN);
    expect(skillOfRun("purlis:deploy")).toBe("deploy");
  });

  it("goes when the core says the chat's run ended", () => {
    const open = toldRun(
      {},
      { plane: "p", session: 4, run: { skill: "deploy", id: RUN, tool_calls: 1 } },
    );
    expect(open[4]?.tool_calls).toBe(1);
    expect(toldRun(open, { plane: "p", session: 4, run: null })).toEqual({});
  });
});

describe("a skill's Runs panel", () => {
  it("lists each run's signals, never a verdict, and marks the run the chip opened it on", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_runs")
        return [
          {
            id: RUN,
            skill: "deploy",
            persona: "ops",
            started_by: "read",
            started: "2026-10-20T10:00:00+00:00",
            ended: "interrupted",
            ms: 3500,
            tool_calls: 7,
            tool_errors: 1,
            refusals: 2,
            asks: 1,
            tokens: null,
          },
        ];
      throw new Error(`unexpected ${cmd}`);
    });
    wantRun("deploy", RUN);

    render(<SkillTab plane="p" name="deploy" changed={0} />);

    const row = await screen.findByTestId(`skill-run-${RUN}`);
    expect(row.getAttribute("aria-current")).toBe("true");
    const text = row.textContent ?? "";
    expect(text).toContain("The chat read the skill");
    expect(text).toContain("Interrupted");
    expect(text).toContain("Not reported");
    expect(text).toContain("3 s");
    expect(screen.getByTestId("skill-runs").textContent).not.toMatch(/pass|fail/i);
  });
});
