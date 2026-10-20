// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillRunChip } from "./SkillRunChip";
import { SkillRunsHere, fixedRuns, skillOfRun, skillOfView, skillView, toldRun } from "./skillRuns";

afterEach(() => {
  cleanup();
  clearMocks();
});

const RUN = "01JB0000000000000000000001";

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
    expect(skillOfView(skillView("deploy", RUN))).toEqual({ name: "deploy", run: RUN });
    expect(skillOfView(skillView("deploy"))).toEqual({ name: "deploy" });
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
