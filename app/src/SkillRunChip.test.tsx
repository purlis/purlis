// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillRunChip } from "./SkillRunChip";
import { SkillRunsHere, fixedRuns, skillOfView, skillView, toldRun } from "./skillRuns";

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
        value={fixedRuns({
          4: { skill: "purlis:deploy", name: "deploy", id: RUN, tool_calls: 3 },
        })}
      >
        <SkillRunChip session={4} onOpen={onOpen} />
        <SkillRunChip session={5} onOpen={onOpen} />
      </SkillRunsHere.Provider>,
    );

    const chip = await screen.findByTestId("tab-skill-run");
    expect(chip.getAttribute("aria-label")).toBe(
      "Using the skill deploy, 3 tool calls so far. Open its runs",
    );
    expect(screen.getAllByTestId("tab-skill-run")).toHaveLength(1);
    fireEvent.click(chip);
    expect(onOpen).toHaveBeenCalledWith("deploy", RUN);
    expect(skillOfView(skillView("deploy", RUN))).toEqual({ name: "deploy", run: RUN });
    expect(skillOfView(skillView("deploy"))).toEqual({ name: "deploy" });
  });

  it("opens the skill the core counts the run of, never a name of its own reading (#3385)", async () => {
    studioOn();
    const onOpen = vi.fn();
    render(
      <SkillRunsHere.Provider
        value={fixedRuns({
          // opencode is handed the built-in `secrets` as `purlis-secrets`.
          4: { skill: "purlis-secrets", name: "secrets", id: RUN, tool_calls: 1 },
          // A plane skill whose own name starts so is its own.
          5: { skill: "purlis-notes", name: "purlis-notes", id: RUN, tool_calls: 1 },
        })}
      >
        <SkillRunChip session={4} onOpen={onOpen} />
        <SkillRunChip session={5} onOpen={onOpen} />
      </SkillRunsHere.Provider>,
    );

    const chips = await screen.findAllByTestId("tab-skill-run");
    for (const chip of chips) fireEvent.click(chip);
    expect(onOpen.mock.calls).toEqual([
      ["secrets", RUN],
      ["purlis-notes", RUN],
    ]);
    expect(chips.map((chip) => chip.querySelector(".skill-run-name")?.textContent)).toEqual([
      "secrets",
      "purlis-notes",
    ]);
  });

  it("goes when the core says the chat's run ended", () => {
    const open = toldRun(
      {},
      { plane: "p", session: 4, run: { skill: "deploy", name: "deploy", id: RUN, tool_calls: 1 } },
    );
    expect(open[4]?.tool_calls).toBe(1);
    expect(toldRun(open, { plane: "p", session: 4, run: null })).toEqual({});
  });
});
