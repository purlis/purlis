// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillTab, groupSaid, startedBySaid } from "./SkillTab";
import type { SkillStats } from "./bindings";

const NO_TOTALS: SkillStats = { by_harness: [], by_persona: [], by_workspace: [] };

afterEach(() => {
  cleanup();
  clearMocks();
});

const RUN = "01JB0000000000000000000001";

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
            ran_on: null,
          },
        ];
      if (cmd === "skill_stats") return NO_TOTALS;
      throw new Error(`unexpected ${cmd}`);
    });
    render(<SkillTab plane="p" name="deploy" run={RUN} changed={0} />);

    const row = await screen.findByTestId(`skill-run-${RUN}`);
    expect(row.getAttribute("aria-current")).toBe("true");
    const text = row.textContent ?? "";
    expect(text).toContain("The chat read the skill");
    expect(text).toContain("Interrupted");
    expect(text).toContain("Not reported");
    expect(text).toContain("3 s");
    expect(text).toContain("2026-10-20 10:00 UTC");
    expect(screen.getByTestId("skill-runs").textContent).not.toMatch(/pass|fail/i);
  });
});

describe("a skill's Runs panel, for a use from before runs were recorded", () => {
  it("says so in plain words, with no signals to show", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_runs")
        return [
          {
            id: null,
            skill: "deploy",
            persona: null,
            started_by: null,
            started: "2026-03-02T09:00:00+00:00",
            ended: null,
            ms: null,
            tool_calls: null,
            tool_errors: null,
            refusals: null,
            asks: null,
            tokens: null,
            ran_on: null,
          },
        ];
      if (cmd === "skill_stats") return NO_TOTALS;
      throw new Error(`unexpected ${cmd}`);
    });

    render(<SkillTab plane="p" name="deploy" changed={0} />);

    const row = await screen.findByTestId("skill-run");
    expect(row.textContent).toContain("From before runs were recorded");
    expect(row.textContent).toContain("Not recorded");
    expect(row.getAttribute("aria-current")).toBeNull();
  });
});

describe("what started a run", () => {
  it("says a load through purlis's tools in plain words", () => {
    expect(startedBySaid("mcp-load")).toBe("The chat loaded the skill");
    expect(startedBySaid("skill-tool")).toBe("The chat's Skill tool");
  });

  it("names a curation action and a trigger", () => {
    expect(startedBySaid("curation")).toBe("A curation action");
    expect(startedBySaid("trigger")).toBe("A trigger");
  });
});

describe("a skill's totals", () => {
  it("adds up its runs by what they ran on, persona and workspace, with their signals and cost", async () => {
    const row = (group: SkillStats["by_harness"][number]["group"], runs: number, ended: number) => ({
      group,
      runs,
      ended,
      tool_calls: 5,
      tool_errors: 1,
      refusals: 0,
      asks: 2,
      tokens: ended > 0 ? 1200 : null,
      ms: 65_000,
    });
    mockIPC((cmd) => {
      if (cmd === "skill_runs") return [];
      if (cmd === "skill_stats")
        return {
          by_harness: [
            row({ kind: "named", name: "Claude Code" }, 2, 2),
            row({ kind: "not-recorded" }, 1, 0),
          ],
          by_persona: [row({ kind: "named", name: "ops" }, 3, 2)],
          by_workspace: [row({ kind: "none" }, 3, 2)],
        } satisfies SkillStats;
      throw new Error(`unexpected ${cmd}`);
    });
    render(<SkillTab plane="p" name="deploy" changed={0} />);

    const harness = await screen.findByTestId("skill-totals-by_harness");
    expect(harness.textContent).toContain("Claude Code");
    expect(harness.textContent).toContain("2 runs");
    expect(harness.textContent).toContain("1 run, 0 ended");
    expect(harness.textContent).toContain("Not recorded");
    expect(harness.textContent).toContain("1200");
    expect(harness.textContent).toContain("1 min 5 s");
    expect(screen.getByTestId("skill-totals-by_persona").textContent).toContain("ops");
    expect(screen.getByTestId("skill-totals-by_workspace").textContent).toContain("Project root");
    expect(screen.getByTestId("skill-totals").textContent).not.toMatch(/pass|fail|harness/i);
  });

  it("shows nothing to total before a skill has run", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_runs") return [];
      if (cmd === "skill_stats") return NO_TOTALS;
      throw new Error(`unexpected ${cmd}`);
    });
    render(<SkillTab plane="p" name="deploy" changed={0} />);
    await screen.findByTestId("skill-runs-empty");
    expect(screen.queryByTestId("skill-totals")).toBeNull();
  });

  it("says each group in plain words", () => {
    expect(groupSaid({ kind: "named", name: "ops" }, "No persona")).toBe("ops");
    expect(groupSaid({ kind: "none" }, "No persona")).toBe("No persona");
    expect(groupSaid({ kind: "not-recorded" }, "No persona")).toBe("Not recorded");
  });
});
