// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillRunChip } from "./SkillRunChip";
import type { OpenRun } from "./bindings";
import { SkillRunsHere, fixedRuns, sameRun, tokensShort, toldRun } from "./skillRuns";
import {
  AT_THE_ROOT,
  skillRuns,
  skillStudioAt,
  skillRowNamed,
  studioOfSkillTab,
} from "./skillStudio";

afterEach(() => {
  cleanup();
  clearMocks();
});

const RUN = "01JB0000000000000000000001";

/** A run of `deploy` with one tool call, on a harness that says no usage. */
const OPEN: OpenRun = {
  skill: "deploy",
  name: "deploy",
  id: RUN,
  tool_calls: 1,
  tokens: null,
  level: null,
  copy_workspace: null,
};

/** A core that answers nothing: the chip reads only the runs it is given. */
function aCoreAskedNothing() {
  mockIPC((cmd) => {
    throw new Error(`unexpected ${cmd}`);
  });
}

describe("the skill run chip on a chat's pane head", () => {
  it("shows the skill in use and its tool calls, and opens the skill's tab on that run", async () => {
    aCoreAskedNothing();
    const onOpen = vi.fn();
    render(
      <SkillRunsHere.Provider
        value={fixedRuns({
          4: { ...OPEN, skill: "purlis:deploy", name: "deploy", tool_calls: 3 },
        })}
      >
        <SkillRunChip session={4} onOpen={onOpen} />
        <SkillRunChip session={5} onOpen={onOpen} />
      </SkillRunsHere.Provider>,
    );

    const chip = await screen.findByTestId("pane-skill-run");
    expect(chip).toHaveAccessibleName("Using the skill deploy, 3 tool calls so far. Open its runs");
    expect(screen.getAllByTestId("pane-skill-run")).toHaveLength(1);
    fireEvent.click(chip);
    expect(onOpen).toHaveBeenCalledWith(expect.objectContaining({ name: "deploy", id: RUN }), 4);
    // A skill's own tab an older launch kept comes back as its page in the studio (#3682).
    expect(studioOfSkillTab({ from: null, view: "skill", key: `deploy@${RUN}` })).toEqual(
      skillRuns(AT_THE_ROOT, "deploy", RUN),
    );
    expect(studioOfSkillTab({ from: null, view: "skill", key: "deploy" })).toEqual(
      skillStudioAt(AT_THE_ROOT, "library", { skill: skillRowNamed("deploy") }),
    );
    expect(studioOfSkillTab({ from: null, view: "vault", key: "deploy" })).toBeUndefined();
  });

  it("opens the skill the core counts the run of, never a name of its own reading (#3385)", async () => {
    aCoreAskedNothing();
    const onOpen = vi.fn();
    render(
      <SkillRunsHere.Provider
        value={fixedRuns({
          // opencode is handed the built-in `secrets` as `purlis-secrets`.
          4: { ...OPEN, skill: "purlis-secrets", name: "secrets" },
          // A plane skill whose own name starts so is its own.
          5: { ...OPEN, skill: "purlis-notes", name: "purlis-notes" },
        })}
      >
        <SkillRunChip session={4} onOpen={onOpen} />
        <SkillRunChip session={5} onOpen={onOpen} />
      </SkillRunsHere.Provider>,
    );

    const chips = await screen.findAllByTestId("pane-skill-run");
    for (const chip of chips) fireEvent.click(chip);
    expect(onOpen.mock.calls.map(([run, session]) => [run.name, session])).toEqual([
      ["secrets", 4],
      ["purlis-notes", 5],
    ]);
    expect(chips.map((chip) => chip.querySelector(".skill-run-name")?.textContent)).toEqual([
      "secrets",
      "purlis-notes",
    ]);
  });

  it("shows the tokens spent since the run opened, live, where the harness says usage (SK-26)", async () => {
    aCoreAskedNothing();
    const onOpen = vi.fn();
    const { rerender } = render(
      <SkillRunsHere.Provider
        value={fixedRuns({ 4: { ...OPEN, tool_calls: 3, tokens: 12_000, level: "persona" } })}
      >
        <SkillRunChip session={4} onOpen={onOpen} />
      </SkillRunsHere.Provider>,
    );

    const chip = await screen.findByTestId("pane-skill-run");
    expect(chip).toHaveAccessibleName(
      "Using the skill deploy, 3 tool calls and 12k tokens so far. Open its runs",
    );
    expect(screen.getByTestId("pane-skill-run-tokens")).toHaveTextContent("12k tokens");
    // A persona's own skill opens on that persona's chats: the chip hands its level on.
    fireEvent.click(chip);
    expect(onOpen).toHaveBeenCalledWith(expect.objectContaining({ level: "persona" }), 4);

    rerender(
      <SkillRunsHere.Provider value={fixedRuns({ 4: { ...OPEN, tool_calls: 4, tokens: 13_400 } })}>
        <SkillRunChip session={4} onOpen={onOpen} />
      </SkillRunsHere.Provider>,
    );
    expect(await screen.findByText("13k tokens")).toBeInTheDocument();
  });

  it("names the workspace whose copy it runs where its persona named another's, and opens that copy's runs (#6083)", async () => {
    const onOpen = vi.fn();
    render(
      <SkillRunsHere.Provider
        value={fixedRuns({ 4: { ...OPEN, level: "workspace", copy_workspace: "ops" } })}
      >
        <SkillRunChip session={4} onOpen={onOpen} />
      </SkillRunsHere.Provider>,
    );

    const chip = await screen.findByTestId("pane-skill-run");
    expect(chip).toHaveAccessibleName(
      "Using the skill deploy of workspace ops, 1 tool call so far. Open its runs",
    );
    fireEvent.click(chip);
    expect(onOpen).toHaveBeenCalledWith(
      expect.objectContaining({ level: "workspace", copy_workspace: "ops" }),
      4,
    );
  });

  it("shows no tokens, never a zero, where the harness says no usage", async () => {
    aCoreAskedNothing();
    render(
      <SkillRunsHere.Provider value={fixedRuns({ 4: OPEN })}>
        <SkillRunChip session={4} onOpen={() => undefined} />
      </SkillRunsHere.Provider>,
    );
    const chip = await screen.findByTestId("pane-skill-run");
    expect(chip).not.toHaveAccessibleName(/token/);
    expect(screen.queryByTestId("pane-skill-run-tokens")).toBeNull();
  });

  it("says tokens in a few characters, and a new count is a new run to draw", () => {
    expect([850, 1_200, 12_345, 999_499, 1_400_000].map(tokensShort)).toEqual([
      "850",
      "1.2k",
      "12k",
      "999k",
      "1.4M",
    ]);
    expect(sameRun(OPEN, { ...OPEN, tokens: 10 })).toBe(false);
    expect(sameRun(OPEN, { ...OPEN, level: "persona" })).toBe(false);
    expect(sameRun(OPEN, { ...OPEN, copy_workspace: "ops" })).toBe(false);
    expect(sameRun(OPEN, { ...OPEN })).toBe(true);
  });

  it("goes when the core says the chat's run ended", () => {
    const open = toldRun({}, { plane: "p", session: 4, run: OPEN });
    expect(open[4]?.tool_calls).toBe(1);
    expect(toldRun(open, { plane: "p", session: 4, run: null })).toEqual({});
  });
});
