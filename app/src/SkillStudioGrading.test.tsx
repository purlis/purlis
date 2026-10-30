// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The grading pages' ways back, in the Skill studio** (#3978): Back to the scorecard, Leave it
 * running and Back to its grades each land on the page they name and pile up no steps, and a
 * scorecard or a saved chat put back from a retired tab at the project's root opens for a
 * workspace's or a persona's skill too. Drawn at the window seam, the core faked at its command
 * boundary.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, renderHook, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { STUDIO_HISTORY, forgetStudio, renderStudioAt } from "./SkillStudioTab.testkit";
import {
  AT_THE_ROOT,
  STUDIO_START,
  backIsTo,
  inWorkspace,
  skillRowId,
  skillRowNamed,
  type StudioPlace,
} from "./skillStudio";
import { skill, studio } from "./skillsListing.testkit";
import { openViewAt, useViewHistory } from "./ui/panel/viewHistory";
import type { GradeScorecard, SkillLevel, StudioSkill } from "./bindings";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetStudio();
});

const DEPLOY = skill("deploy");
const OPS: SkillLevel = { at: "workspace", name: "ops", local: false };
const OPS_DEPLOY = skill("deploy", {
  level: "workspace",
  label: "workspace",
  folder: "workspaces/ops/skills/deploy",
});

/** The scorecard of grade `01G` of the copy at `level`. */
function card(level: SkillLevel): GradeScorecard {
  return {
    grade: "01G",
    skill: "deploy",
    level,
    graded: "2026-10-28 10:00 UTC",
    version: "0123456789ab",
    ran_on: "Claude Code · sonnet",
    ended: "Finished",
    delta: 10,
    against: null,
    delta_without: null,
    after_edit: false,
    asked: 0,
    outcome: "graded",
    arms: { with: 60, without: 50, previous: null },
    spent: { chats: 4, tokens: 1000, cost_usd: 0.2, limit_usd: 3 },
    tests: [],
  };
}

/** A core whose root lists `root` and whose workspace ops lists `ops`, the grade's scorecard of
 *  the copy at `level`, and a grade `01RUN` running; each ask kept. */
function core(root: StudioSkill[], ops: StudioSkill[], level: SkillLevel) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC(
    (cmd, args) => {
      const given = (args ?? {}) as Record<string, unknown>;
      asked.push({ cmd, args: given });
      switch (cmd) {
        case "skill_studio": {
          const workspace = (given.workspace as string | null) ?? null;
          return studio(workspace === "ops" ? ops : root, { workspace });
        }
        case "skill_grade_scorecard":
          return card(level);
        case "skill_grade_chat":
          throw "This grade's chat was not kept on this machine.";
        case "skill_grades":
          return {
            grades: [],
            said: null,
            tests: 2,
            tests_refused: null,
            running: {
              plane: "p",
              skill: "deploy",
              level,
              grade: "01RUN",
              ran: 1,
              of: 12,
              tokens: null,
              cost_usd: null,
              limit_usd: 3,
              cap: 200000,
              asked: 0,
              ended: null,
              said: null,
            },
          };
        case "skill_grade_after_edit":
          return null;
        case "skill_details":
          return {
            copies: [],
            uses: [],
            used_by: [],
            uses_cut: false,
            pinned_by: [],
            pins_refused: [],
          };
        case "skill_stats":
          return { by_harness: [], by_persona: [], by_workspace: [] };
        case "start_options":
          return { profiles: [] };
        case "skill_updates":
          return { offers: [], unconfirmed: [], unasked: [] };
        default:
          return [];
      }
    },
    { shouldMockEvents: true },
  );
  return asked;
}

/** The studio tab's history as it stands. */
function walked() {
  return renderHook(() => useViewHistory<StudioPlace>(STUDIO_HISTORY, STUDIO_START)).result.current;
}

describe("a grading page's way back (#3978)", () => {
  it("is a step back only where the step behind is the page it names", () => {
    const grades: StudioPlace = {
      page: "library",
      scope: AT_THE_ROOT,
      skill: skillRowId(DEPLOY),
      tab: "grades",
    };
    expect(backIsTo(grades, grades)).toBe(true);
    expect(backIsTo(undefined, grades)).toBe(false);
    expect(backIsTo({ page: "library", scope: AT_THE_ROOT }, grades)).toBe(false);
    expect(backIsTo({ ...grades, tab: "overview" }, grades)).toBe(false);
    expect(backIsTo({ ...grades, grading: { at: "scorecard", grade: "01G" } }, grades)).toBe(false);
  });

  it("lands Back to the scorecard on the scorecard, with Library behind a restored chat", async () => {
    core([DEPLOY], [], { at: "project", local: false });
    // A saved chat put back from a retired tab: Library is the step behind it.
    openViewAt(STUDIO_HISTORY, STUDIO_START);
    renderStudioAt({
      page: "library",
      scope: AT_THE_ROOT,
      skill: skillRowNamed("deploy"),
      tab: "grades",
      grading: { at: "chat", grade: "01G", chat: "01C" },
    });
    const user = userEvent.setup();
    await screen.findByTestId("grade-chat");

    await user.click(screen.getByRole("button", { name: "Back to the scorecard" }));

    await screen.findByTestId("grade-scorecard");
    expect(walked().at.grading).toEqual({ at: "scorecard", grade: "01G" });
    expect(walked().behind).toEqual(STUDIO_START);
  });

  it("goes back from a chat to the scorecard it was opened from, adding no step", async () => {
    core([DEPLOY], [], { at: "project", local: false });
    const scorecard: StudioPlace = {
      page: "library",
      scope: AT_THE_ROOT,
      skill: skillRowNamed("deploy"),
      tab: "grades",
      grading: { at: "scorecard", grade: "01G" },
    };
    openViewAt(STUDIO_HISTORY, scorecard);
    renderStudioAt({ ...scorecard, grading: { at: "chat", grade: "01G", chat: "01C" } });
    const user = userEvent.setup();
    await screen.findByTestId("grade-chat");

    await user.click(screen.getByRole("button", { name: "Back to the scorecard" }));

    await screen.findByTestId("grade-scorecard");
    expect(walked()).toMatchObject({ canGoBack: false, canGoForward: true });
  });

  it("leaves a running grade for its Grades tab without a step, so Back does not reopen it", async () => {
    core([DEPLOY], [], { at: "project", local: false });
    const grades: StudioPlace = {
      page: "library",
      scope: AT_THE_ROOT,
      skill: skillRowNamed("deploy"),
      tab: "grades",
    };
    openViewAt(STUDIO_HISTORY, grades);
    renderStudioAt({ ...grades, grading: { at: "running", grade: "01RUN" } });
    const user = userEvent.setup();
    await screen.findByTestId("skill-grade-running");

    await user.click(screen.getByRole("button", { name: "Leave it running" }));

    await screen.findByTestId("skill-grades");
    expect(walked()).toMatchObject({ canGoBack: false, canGoForward: true });
    expect(walked().at.grading).toBeUndefined();
  });
});

describe("a scorecard put back at the project's root (#3978)", () => {
  it("opens as that scorecard for a workspace's skill, and goes to the workspace", async () => {
    const asked = core([], [OPS_DEPLOY], OPS);
    renderStudioAt({
      page: "library",
      scope: AT_THE_ROOT,
      skill: skillRowNamed("deploy"),
      tab: "grades",
      grading: { at: "scorecard", grade: "01G" },
    });

    await screen.findByTestId("grade-scorecard");
    await waitFor(() => expect(walked().at.scope).toEqual(inWorkspace("ops")));
    await waitFor(() =>
      expect(asked.some((one) => one.cmd === "skill_studio" && one.args.workspace === "ops")).toBe(
        true,
      ),
    );
    expect(await screen.findByTestId("grade-scorecard")).toBeInTheDocument();
    expect(walked().at.grading).toEqual({ at: "scorecard", grade: "01G" });
  });

  it("opens a saved chat of a workspace's skill as that chat", async () => {
    core([], [OPS_DEPLOY], OPS);
    renderStudioAt({
      page: "library",
      scope: AT_THE_ROOT,
      skill: skillRowNamed("deploy"),
      tab: "grades",
      grading: { at: "chat", grade: "01G", chat: "01C" },
    });

    await screen.findByTestId("grade-chat");
    await waitFor(() => expect(walked().at.scope).toEqual(inWorkspace("ops")));
    expect(screen.getByTestId("grade-chat")).toBeInTheDocument();
  });

  it("opens on the row of the copy it graded, never a same-named one", async () => {
    const local = skill("deploy", { local: true, folder: "skills.local/deploy" });
    core([DEPLOY, local], [], { at: "project", local: true });
    renderStudioAt({
      page: "library",
      scope: AT_THE_ROOT,
      skill: skillRowNamed("deploy"),
      tab: "grades",
      grading: { at: "scorecard", grade: "01G" },
    });

    await screen.findByTestId("grade-scorecard");
    await waitFor(() => expect(walked().at.skill).toBe(skillRowId(local)));
    expect(walked().at.scope).toEqual(AT_THE_ROOT);
  });

  it("stays at the root for a workspace the project no longer has", async () => {
    core([], [], { at: "workspace", name: "gone", local: false });
    renderStudioAt({
      page: "library",
      scope: AT_THE_ROOT,
      skill: skillRowNamed("deploy"),
      tab: "grades",
      grading: { at: "scorecard", grade: "01G" },
    });

    await screen.findByTestId("grade-scorecard");
    await new Promise((done) => setTimeout(done, 50));
    expect(walked().at.scope).toEqual(AT_THE_ROOT);
  });

  it("stays where it is for the project's own copy", async () => {
    core([DEPLOY], [], { at: "project", local: false });
    renderStudioAt({
      page: "library",
      scope: AT_THE_ROOT,
      skill: skillRowNamed("deploy"),
      tab: "grades",
      grading: { at: "scorecard", grade: "01G" },
    });

    await screen.findByRole("button", { name: "Grade again" });
    expect(walked().at.scope).toEqual(AT_THE_ROOT);
  });
});
