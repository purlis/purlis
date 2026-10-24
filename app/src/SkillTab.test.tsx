// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillTab, startedBySaid } from "./SkillTab";

afterEach(() => {
  cleanup();
  clearMocks();
});

const RUN = "01JB0000000000000000000001";

const EMPTY_PAGE = { copies: [], uses: [], used_by: [], uses_cut: false };

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
            inside: null,
            ran: [],
          },
        ];
      if (cmd === "skill_details") return EMPTY_PAGE;
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
            inside: null,
            ran: [],
          },
        ];
      if (cmd === "skill_details") return EMPTY_PAGE;
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
});
