// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A skill's Grades tab** (#3688, S6), drawn at the window seam with the core faked at its
 * command boundary: the latest grade as bars, every grade with how it ended, the grade-after-
 * each-edit row, a skill with no trigger tests, and where each press goes.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import {
  GradesTab,
  gradingCrumbTo,
  gradingPath,
  type GradingGo,
  type TestsFile,
} from "./SkillGrades";
import { SKILL_CHAT, type SkillChatAsk } from "./skillDraftAsks";
import { StudioChatsHere, type StudioChatsLent } from "./studioChats";
import type { GradeRow, SkillDraft, SkillDraftFile, SkillLevel } from "./bindings";
import { findGrid } from "./ui/table/table.testkit";

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/purlis-demo";
const LEVEL = { at: "project", local: false } as const;

/** A grade's row, `over` said on top of a whole one. */
function row(grade: string, over: Partial<GradeRow> = {}): GradeRow {
  return {
    grade,
    graded: "2026-10-28 10:00 UTC",
    version: "0123456789ab",
    ran_on: "Claude Code · sonnet",
    ended: "Finished",
    outcome: "graded",
    arms: { with: 86, without: 68, previous: 80 },
    outcome_arms: null,
    spent: {
      chats: 36,
      tokens: 300000,
      cost_usd: 1.92,
      limit_usd: 3,
      resumed: false,
      earlier_tokens: null,
      earlier_usd: null,
    },
    after_edit: false,
    ...over,
  };
}

const GRADES = [
  row("01C"),
  row("01B", {
    graded: "2026-10-22 09:00 UTC",
    outcome: "at-limit",
    ended: "Stopped at its dollar limit",
    arms: { with: 80, without: 69, previous: null },
    outcome_arms: null,
  }),
  row("01A", {
    graded: "2026-10-20 08:00 UTC",
    outcome: "interrupted",
    ended: "Interrupted: not scored",
    arms: { with: null, without: null, previous: null },
    outcome_arms: null,
    spent: {
      chats: 2,
      tokens: null,
      cost_usd: 0.21,
      limit_usd: null,
      resumed: false,
      earlier_tokens: null,
      earlier_usd: null,
    },
  }),
];

/** A step of the grade `grade` of deploy, running: of this copy, or of the copy at `level`. */
const step = (grade: string, level: SkillLevel = LEVEL) => ({
  plane: PLANE,
  skill: "deploy",
  level,
  grade,
  ran: 3,
  of: 12,
  tokens: 1000,
  cost_usd: 0.25,
  limit_usd: 3,
  cap: 500000,
  asked: 0,
  ended: null,
  said: null,
  draft: null,
});

/** deploy's grades `grades`, its `tests` trigger tests, and a grade `running` where one is. */
const grades = (rows: GradeRow[] = GRADES, tests = 2, running: string | null = null) => ({
  grades: rows,
  said: null,
  tests,
  evals: 0,
  tests_refused: null,
  running: running === null ? null : step(running),
});

/** A core answering the tab, and each ask it was sent. */
function core(answers: Record<string, unknown> = {}) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC(
    (cmd, args) => {
      asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
      if (cmd in answers) {
        const answer = answers[cmd];
        if (answer instanceof Error) throw answer.message;
        return typeof answer === "function" ? (answer as () => unknown)() : answer;
      }
      if (cmd === "skill_grades") return grades();
      return null;
    },
    { shouldMockEvents: true },
  );
  return asked;
}

/** One file of a draft: a new file, `after` its text. */
const file = (path: string, after: string): SkillDraftFile => ({
  path,
  before: null,
  after,
  hunks: [],
  stale: false,
});

/** A chat's draft of deploy's checks: both kinds, in its one draft. */
const DRAFT: SkillDraft = {
  id: "d1",
  name: "deploy",
  level: LEVEL,
  chat: 14,
  chat_open: true,
  new: false,
  stale: false,
  files: [
    file(
      "evals/triggers.json",
      JSON.stringify([
        { query: "deploy the api to staging", should_trigger: true },
        { query: "what changed since the last release?", should_trigger: false },
      ]),
    ),
    file(
      "evals/evals.json",
      JSON.stringify({
        skill_name: "deploy",
        evals: [
          {
            id: 1,
            prompt: "Deploy svc to staging from a branch with a failing test",
            expectations: ["It stops before deploying"],
          },
        ],
      }),
    ),
  ],
};

/** The Grades tab of deploy over a core that answers its grades `answer`. */
function renderTabWith(answer: unknown) {
  core({ skill_grades: answer });
  return renderTab({ open: vi.fn() });
}

/** The Grades tab of deploy, and where each press went. */
function renderTab(
  testsFile?: TestsFile,
  more: { evalsFile?: TestsFile; draft?: SkillDraft } = {},
) {
  const onGrading = vi.fn<GradingGo>();
  render(
    <GradesTab
      plane={PLANE}
      name="deploy"
      level={LEVEL}
      changed={0}
      onGrading={onGrading}
      testsFile={testsFile}
      {...more}
    />,
  );
  return onGrading;
}

describe("a skill's Grades tab (S6)", () => {
  it("shows each kind's latest grade as bars under its question, and every grade with how it ended", async () => {
    core({
      skill_grades: {
        ...grades([
          row("01C", { outcome_arms: { with: 75, without: 20, previous: null } }),
          ...GRADES.slice(1),
        ]),
        evals: 1,
      },
    });
    const onGrading = renderTab();
    const picked = await screen.findByRole("region", { name: "Does it get picked?" });
    expect(picked).toHaveTextContent("trigger tests · evals/triggers.json");
    expect(picked).toHaveTextContent("2 prompts");
    const bars = within(picked).getByRole("group", {
      name: "Does it get picked? The latest grade of deploy",
    });
    const meters = within(bars).getAllByRole("meter");
    expect(meters.map((one) => one.getAttribute("aria-valuenow"))).toEqual(["86", "68", "80"]);
    expect(within(bars).getByRole("meter", { name: "Without it" })).toBeTruthy();

    const job = screen.getByRole("region", { name: "Does it do the job?" });
    expect(job).toHaveTextContent("outcome evals · evals/evals.json");
    expect(job).toHaveTextContent("1 task");
    const jobBars = within(job).getByRole("group", {
      name: "Does it do the job? The latest grade of deploy",
    });
    expect(
      within(jobBars)
        .getAllByRole("meter")
        .map((one) => one.getAttribute("aria-valuenow")),
    ).toEqual(["75", "20"]);

    expect(screen.getByTestId("skill-checks-latest")).toHaveTextContent(
      "Latest grade · 2026-10-28 10:00 UTC · Claude Code · sonnet · $1.92",
    );

    const list = await findGrid(screen, "Grades of deploy");
    await waitFor(() => expect(within(list).getAllByRole("row")).toHaveLength(4));
    const [, graded, limit, interrupted] = within(list).getAllByRole("row");
    expect(graded).toHaveTextContent("Graded");
    expect(limit).toHaveTextContent("Stopped at its dollar limit");
    expect(limit).toHaveTextContent("—");
    expect(interrupted).toHaveTextContent("Interrupted: not scored");
    expect(interrupted).toHaveTextContent("$0.21");

    fireEvent.click(
      within(limit).getByRole("button", { name: "Scorecard of the grade of 2026-10-22 09:00 UTC" }),
    );
    expect(onGrading).toHaveBeenLastCalledWith({ at: "scorecard", grade: "01B" }, "go");
    expect(
      within(interrupted).getByRole("button", {
        name: "What ran of the grade of 2026-10-20 08:00 UTC",
      }),
    ).toHaveTextContent("What ran");
  });

  it("answers in one sentence, above the kinds, once a grade compared them", async () => {
    core({
      skill_grades: {
        ...grades([row("01C", { outcome_arms: { with: 75, without: 20, previous: null } })]),
        evals: 1,
      },
    });
    renderTab();
    expect(await screen.findByTestId("skill-checks-answer")).toHaveTextContent(
      "With deploy, chats picked it right 86% of the time and met 75% of what the job asks; without it, 68% and 20%.",
    );
  });

  it("answers for the one kind a grade compared, and for none where it compared nothing", async () => {
    core();
    renderTab();
    expect(await screen.findByTestId("skill-checks-answer")).toHaveTextContent(
      "With deploy, chats picked it right 86% of the time; without it, 68%.",
    );
    cleanup();
    core({ skill_grades: grades([GRADES[2]]) });
    renderTab();
    await findGrid(screen, "Grades of deploy");
    expect(screen.queryByTestId("skill-checks-answer")).toBeNull();
  });

  it("says what a skill with no checks would check, and offers no grading after each edit", async () => {
    const asked = core({ skill_grades: grades([], 0) });
    renderTab();
    const none = await screen.findByTestId("skill-checks-none");
    expect(none).toHaveTextContent("deploy has no checks yet");
    expect(none).toHaveTextContent("Does it get picked?");
    expect(none).toHaveTextContent("Does it do the job?");
    expect(screen.queryByRole("grid")).toBeNull();
    const turnOn = screen.getByRole("button", { name: "Turn on…" });
    expect(turnOn.getAttribute("aria-disabled") === "true" || turnOn.hasAttribute("disabled")).toBe(
      true,
    );
    // Its grades say it has none: no plan is asked (#3928).
    expect(asked.some((one) => one.cmd === "skill_grade_plan")).toBe(false);
  });

  it("asks one chat for both kinds of checks in one press", async () => {
    core({ skill_grades: grades([], 0) });
    renderTab({ open: vi.fn() });
    const none = await screen.findByTestId("skill-checks-none");
    const asks: SkillChatAsk[] = [];
    const heard = (event: Event) => asks.push((event as CustomEvent<SkillChatAsk>).detail);
    window.addEventListener(SKILL_CHAT, heard);
    fireEvent.click(
      within(none).getByRole("button", {
        name: "Ask a chat at purlis-demo to write checks",
      }),
    );
    window.removeEventListener(SKILL_CHAT, heard);
    expect(asks).toEqual([{ plane: PLANE, kind: "checks", level: LEVEL, name: "deploy" }]);
  });

  it("writes them by hand from the template, which opens to write in (#3925)", async () => {
    const asked = core({ skill_grades: grades([], 0), skill_trigger_tests_start: { notes: [] } });
    const open = vi.fn();
    renderTab({ open });
    const none = await screen.findByTestId("skill-checks-none");
    fireEvent.click(within(none).getByRole("button", { name: "Write them yourself" }));
    // The core starts the file, then it opens to write in.
    await waitFor(() => expect(open).toHaveBeenCalledTimes(1));
    expect(asked.find((one) => one.cmd === "skill_trigger_tests_start")?.args).toMatchObject({
      plane: PLANE,
      level: LEVEL,
      name: "deploy",
    });
  });

  it("holds both offers where the skill's files can't be opened, and says why", async () => {
    core({ skill_grades: grades([], 0) });
    renderTab({ reason: "Its file is outside the project." });
    const none = await screen.findByTestId("skill-checks-none");
    // Neither would land in the skill: not the template, nor a chat's draft.
    for (const name of ["Ask a chat at purlis-demo to write checks", "Write them yourself"]) {
      const offer = within(none).getByRole("button", { name });
      expect(offer.getAttribute("aria-disabled") === "true" || offer.hasAttribute("disabled")).toBe(
        true,
      );
    }
  });

  it("offers a skill with only “picked” checks a chat for the job's, kept as a draft", async () => {
    core();
    renderTab({ open: vi.fn() }, { evalsFile: { open: vi.fn() } });
    const job = await screen.findByRole("region", { name: "Does it do the job?" });
    expect(job).toHaveTextContent("None yet.");
    expect(within(job).queryByRole("group")).toBeNull();
    const asks: SkillChatAsk[] = [];
    const heard = (event: Event) => asks.push((event as CustomEvent<SkillChatAsk>).detail);
    window.addEventListener(SKILL_CHAT, heard);
    fireEvent.click(
      within(job).getByRole("button", { name: "Ask a chat at purlis-demo to write these" }),
    );
    window.removeEventListener(SKILL_CHAT, heard);
    expect(asks).toEqual([{ plane: PLANE, kind: "evals", level: LEVEL, name: "deploy" }]);
  });

  it("shows a studio chat writing the job's checks in place of its button, with Open chat (#5185)", async () => {
    core();
    const show = vi.fn();
    const lent: StudioChatsLent = {
      chats: [
        {
          session: 7,
          name: "chat 7",
          link: {
            skill: "deploy",
            level: LEVEL,
            job: "outcome-evals",
            done: false,
            started: null,
            finished: null,
          },
        },
      ],
      show,
      openSkill: vi.fn(),
      openDraft: vi.fn(),
      stop: vi.fn(),
    };
    render(
      <StudioChatsHere.Provider value={lent}>
        <GradesTab
          plane={PLANE}
          name="deploy"
          level={LEVEL}
          changed={0}
          onGrading={vi.fn<GradingGo>()}
          testsFile={{ open: vi.fn() }}
          evalsFile={{ open: vi.fn() }}
        />
      </StudioChatsHere.Provider>,
    );
    const job = await screen.findByRole("region", { name: "Does it do the job?" });
    expect(job).toHaveTextContent("A chat at purlis-demo is writing outcome evals · running");
    expect(within(job).queryByRole("button", { name: /to write these/ })).toBeNull();
    fireEvent.click(within(job).getByRole("button", { name: "Open chat" }));
    expect(show).toHaveBeenCalledWith(7);
  });

  it("holds a kind's offer where the skill's files can't be opened", async () => {
    core();
    renderTab({ reason: "Its file is outside the project." });
    const job = await screen.findByRole("region", { name: "Does it do the job?" });
    const offer = within(job).getByRole("button", {
      name: "Ask a chat at purlis-demo to write these",
    });
    expect(offer.getAttribute("aria-disabled") === "true" || offer.hasAttribute("disabled")).toBe(
      true,
    );
  });

  it("opens a kind's file to edit its checks", async () => {
    core({ skill_grades: { ...grades(), evals: 1 } });
    const open = vi.fn();
    const openEvals = vi.fn();
    renderTab({ open }, { evalsFile: { open: openEvals } });
    const picked = await screen.findByRole("region", { name: "Does it get picked?" });
    fireEvent.click(within(picked).getByRole("button", { name: "Edit these checks" }));
    expect(open).toHaveBeenCalledTimes(1);
    const job = screen.getByRole("region", { name: "Does it do the job?" });
    fireEvent.click(within(job).getByRole("button", { name: "Edit these checks" }));
    expect(openEvals).toHaveBeenCalledTimes(1);
  });

  it("says why the trigger tests were not started, and opens nothing", async () => {
    core({
      skill_grades: grades([], 0),
      skill_trigger_tests_start: new Error("This project can't be written, so nothing was."),
    });
    const open = vi.fn();
    renderTab({ open });
    const none = await screen.findByTestId("skill-checks-none");
    fireEvent.click(within(none).getByRole("button", { name: "Write them yourself" }));
    await screen.findByText("This project can't be written, so nothing was.");
    expect(open).not.toHaveBeenCalled();
  });

  it("says why its trigger tests were not read, and does not call them missing", async () => {
    core({
      skill_grades: {
        ...grades([], 0),
        tests_refused: "The skill's trigger tests are not JSON (line 1, column 2).",
      },
    });
    renderTab();
    await screen.findByRole("region", { name: "Does it get picked?" });
    expect(screen.queryByTestId("skill-checks-none")).toBeNull();
  });

  it("offers a first grade where it has checks and no grades", async () => {
    core({ skill_grades: grades([]) });
    const onGrading = renderTab();
    const checks = await screen.findByTestId("skill-checks");
    expect(checks).toHaveTextContent("Not graded yet.");
    fireEvent.click(within(checks).getByRole("button", { name: "Grade deploy…" }));
    expect(onGrading).toHaveBeenCalledWith({ at: "plan" }, "go");
  });

  it("offers to grade again beside the heading once graded", async () => {
    core();
    const onGrading = renderTab();
    const checks = await screen.findByTestId("skill-checks");
    fireEvent.click(within(checks).getByRole("button", { name: "Grade again…" }));
    expect(onGrading).toHaveBeenCalledWith({ at: "plan" }, "go");
  });

  it("shows the checks a chat wrote as one draft, accepted whole", async () => {
    const asked = core({ skill_grades: grades([], 0), skill_draft_accept: { notes: [] } });
    renderTab({ open: vi.fn() }, { draft: DRAFT });
    const draft = await screen.findByTestId("skill-checks-draft");
    expect(draft).toHaveTextContent("Chat 14 wrote checks for deploy");
    expect(draft).toHaveTextContent("Does it get picked? · 2 prompts");
    expect(draft).toHaveTextContent("deploy the api to staging");
    expect(draft).toHaveTextContent("Does it do the job? · 1 task");
    expect(draft).toHaveTextContent("Deploy svc to staging from a branch with a failing test");
    // While it waits, the skill has no checks to call missing: the draft is the way on.
    expect(screen.queryByTestId("skill-checks-none")).toBeNull();
    fireEvent.click(within(draft).getByRole("button", { name: "Accept checks" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_draft_accept")?.args).toEqual({
        plane: PLANE,
        id: "d1",
      }),
    );
  });

  it("discards a draft of checks, and offers no redo while it is current", async () => {
    const asked = core({ skill_grades: grades([], 0), skill_draft_reject: null });
    renderTab({ open: vi.fn() }, { draft: DRAFT });
    const draft = await screen.findByTestId("skill-checks-draft");
    // A redo is for a draft whose files changed under it: a current one has nothing to redo.
    expect(within(draft).queryByRole("button", { name: /redo/ })).toBeNull();
    fireEvent.click(within(draft).getByRole("button", { name: "Discard" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_draft_reject")?.args).toEqual({
        plane: PLANE,
        id: "d1",
      }),
    );
  });

  it("asks the chat to redo a draft of checks that is out of date, and offers no Accept", async () => {
    core({ skill_grades: grades([], 0) });
    renderTab({ open: vi.fn() }, { draft: { ...DRAFT, stale: true } });
    const draft = await screen.findByTestId("skill-checks-draft");
    expect(draft).toHaveTextContent("Out of date");
    expect(within(draft).queryByRole("button", { name: "Accept checks" })).toBeNull();
    const asks: SkillChatAsk[] = [];
    const heard = (event: Event) => asks.push((event as CustomEvent<SkillChatAsk>).detail);
    window.addEventListener(SKILL_CHAT, heard);
    fireEvent.click(within(draft).getByRole("button", { name: "Ask the chat to redo them" }));
    window.removeEventListener(SKILL_CHAT, heard);
    expect(asks).toEqual([
      {
        plane: PLANE,
        kind: "redo",
        id: "d1",
        name: "deploy",
        level: { at: "project", local: false },
        chat: 14,
      },
    ]);
  });

  it("says a drafted file it can't read as checks, and holds Accept", async () => {
    core({ skill_grades: grades([], 0) });
    const broken = {
      ...DRAFT,
      files: [
        file("evals/triggers.json", '[{"query": "deploy the api"'),
        file("evals/evals.json", JSON.stringify({ evals: [{ id: 1, task: "Deploy svc" }] })),
      ],
    };
    renderTab({ open: vi.fn() }, { draft: broken });
    const draft = await screen.findByTestId("skill-checks-draft");
    expect(draft).toHaveTextContent(
      "Does it get picked? · evals/triggers.json can't be read as checks, so a grade couldn't run it.",
    );
    expect(draft).toHaveTextContent(
      "Does it do the job? · evals/evals.json can't be read as checks, so a grade couldn't run it.",
    );
    const accept = within(draft).getByRole("button", { name: "Accept checks" });
    expect(accept.getAttribute("aria-disabled") === "true" || accept.hasAttribute("disabled")).toBe(
      true,
    );
  });

  it("keeps a skill's grades in reach after its checks are gone", async () => {
    const onGrading = renderTabWith(grades(GRADES, 0));
    await screen.findByTestId("skill-checks-none");
    const list = await findGrid(screen, "Grades of deploy");
    fireEvent.click(
      await within(list).findByRole("button", {
        name: "Scorecard of the grade of 2026-10-28 10:00 UTC",
      }),
    );
    expect(onGrading).toHaveBeenLastCalledWith({ at: "scorecard", grade: "01C" }, "go");
  });

  it("names each kind's columns apart, and draws the job's only where a grade ran them", async () => {
    core({
      skill_grades: {
        ...grades([row("01C", { outcome_arms: { with: 75, without: 20, previous: null } })]),
        evals: 1,
      },
    });
    renderTab();
    // React Aria fills in a grid after drawing it (#4950): findGrid waits for a row, and the
    // columns are waited for in turn.
    const list = await findGrid(screen, "Grades of deploy");
    const names = (await within(list).findAllByRole("columnheader")).map((one) => one.textContent);
    expect(names).toEqual(
      expect.arrayContaining([
        "Picked, without it",
        "Does the job, with it",
        "Does the job, without it",
      ]),
    );
    expect(new Set(names).size).toBe(names.length);
    cleanup();
    core();
    renderTab();
    const only = await findGrid(screen, "Grades of deploy");
    await within(only).findByRole("columnheader", { name: "Graded" });
    expect(within(only).queryByRole("columnheader", { name: /Does the job/ })).toBeNull();
  });

  it("holds each kind's ask by its own file", async () => {
    core();
    renderTab({ reason: "Its file is outside the project." }, { evalsFile: { open: vi.fn() } });
    const job = await screen.findByRole("region", { name: "Does it do the job?" });
    const offer = within(job).getByRole("button", {
      name: "Ask a chat at purlis-demo to write these",
    });
    expect(offer.getAttribute("aria-disabled") === "true" || offer.hasAttribute("disabled")).toBe(
      false,
    );
  });

  it("leaves a draft that changes more than its checks to the skill's own review", async () => {
    core({ skill_grades: grades([], 0) });
    const more = {
      ...DRAFT,
      files: [...DRAFT.files, file("SKILL.md", "---\nname: deploy\n---\n")],
    };
    renderTab({ open: vi.fn() }, { draft: more });
    await screen.findByTestId("skill-checks-none");
    expect(screen.queryByTestId("skill-checks-draft")).toBeNull();
  });

  it("says what grading after each edit may spend before it is on, and turns off in one press", async () => {
    let on: unknown = null;
    const asked = core({ skill_grade_after_edit: () => on, skill_grade_after_edit_off: null });
    const onGrading = renderTab();
    const line = await screen.findByTestId("skill-grade-after-edit");
    expect(line.textContent).toBe(
      "Off. When it's on, each edit you save is graded once, within a limit you accept first.",
    );
    fireEvent.click(screen.getByRole("button", { name: "Turn on…" }));
    expect(onGrading).toHaveBeenCalledWith({ at: "plan", afterEdit: true }, "go");
    expect(asked.some((one) => one.cmd === "skill_grade_after_edit_on")).toBe(false);
    cleanup();

    on = {
      profile: "work",
      model: "sonnet",
      chats: 12,
      token_cap: 200000,
      limit_usd: 15,
      waits: null,
    };
    renderTab();
    await waitFor(() =>
      expect(screen.getByTestId("skill-grade-after-edit").textContent).toBe(
        "Each edit you save grades this skill on work, sonnet: up to 12 chats, and at most $15.00 or 200,000 tokens, each time, paid by that profile's account.",
      ),
    );
    on = null;
    fireEvent.click(screen.getByRole("button", { name: "Turn off" }));
    await waitFor(() =>
      expect(asked.some((one) => one.cmd === "skill_grade_after_edit_off")).toBe(true),
    );
    await waitFor(() =>
      expect(screen.getByTestId("skill-grade-after-edit").textContent).toMatch(/^Off\./),
    );
  });

  it("says why an edit was not graded, and that it waits for you", async () => {
    let waits: string | null = null;
    core({
      skill_grade_after_edit: () => ({
        profile: "work",
        model: null,
        chats: 12,
        token_cap: 200000,
        limit_usd: 15,
        waits,
      }),
    });
    const onGrading = renderTab();
    await screen.findByText(/^Each edit you save grades this skill on work/);
    waits = "A grade of it now runs 18 chats, more than the 12 you accepted.";
    await emit("skill-grade-after-edit", {
      plane: PLANE,
      skill: "deploy",
      level: LEVEL,
      said: waits,
    });
    await screen.findByText(waits);
    await waitFor(() =>
      expect(screen.getByTestId("skill-grade-after-edit").textContent).toBe(
        `It waits for you: ${waits}`,
      ),
    );
    // Its new plan is read and accepted on its page.
    fireEvent.click(screen.getByRole("button", { name: "Read the new plan…" }));
    expect(onGrading).toHaveBeenCalledWith({ at: "plan", afterEdit: true }, "go");
  });

  it("shows a grade running, one press away", async () => {
    const asked = core({ skill_grades: grades(GRADES, 2, "01RUN") });
    const onGrading = renderTab();
    await screen.findByTestId("skill-grades-running");
    fireEvent.click(screen.getByRole("button", { name: "Show it" }));
    expect(onGrading).toHaveBeenCalledWith({ at: "running", grade: "01RUN" }, "go");
    expect(asked.some((one) => one.cmd === "skill_grade_plan")).toBe(false);
  });

  it("moves nothing for a grade of a same-named copy at another level (#3978)", async () => {
    let started = false;
    const asked = core({ skill_grades: () => grades(GRADES, 2, started ? "01NEW" : null) });
    renderTab();
    await screen.findByRole("group", { name: "Does it get picked? The latest grade of deploy" });
    const reads = () => asked.filter((one) => one.cmd === "skill_grades").length;
    const before = reads();
    // The workspace ops's deploy is another skill: its grade starts, ends and waits elsewhere.
    const ops: SkillLevel = { at: "workspace", name: "ops", local: false };
    started = true;
    await emit("skill-grade", step("01NEW", ops));
    await emit("skill-grade", { ...step("01NEW", ops), ended: "stopped", said: "It stopped." });
    await emit("skill-grade-after-edit", {
      plane: PLANE,
      skill: "deploy",
      level: ops,
      said: "Your edit was not graded.",
    });
    await new Promise((done) => setTimeout(done, 50));
    expect(reads()).toBe(before);
    expect(screen.queryByTestId("skill-grades-running")).toBeNull();
    expect(screen.queryByText("It stopped.")).toBeNull();
    expect(screen.queryByText("Your edit was not graded.")).toBeNull();
  });

  it("shows a grade that starts while the tab is open", async () => {
    let started = false;
    core({ skill_grades: () => grades(GRADES, 2, started ? "01NEW" : null) });
    renderTab();
    await screen.findByRole("group", { name: "Does it get picked? The latest grade of deploy" });
    expect(screen.queryByTestId("skill-grades-running")).toBeNull();
    // A grade after an edit starts: its first step reads the tab again.
    started = true;
    await emit("skill-grade", step("01NEW"));
    await screen.findByTestId("skill-grades-running");
  });

  it("is not moved by a grade of a draft of the skill (#2780)", async () => {
    let reads = 0;
    core({
      skill_grades: () => {
        reads += 1;
        return grades(GRADES, 2, null);
      },
    });
    renderTab();
    await screen.findByRole("group", { name: "Does it get picked? The latest grade of deploy" });
    const before = reads;
    await emit("skill-grade", { ...step("01DRAFT"), draft: "d1" });
    await emit("skill-grade", { ...step("01OWN") });
    await waitFor(() => expect(reads).toBe(before + 1));
  });

  it("says in place why its grades could not be read, with Try again", async () => {
    let reads = 0;
    core({
      skill_grades: () => {
        reads += 1;
        if (reads === 1) throw "The grade record could not be read.";
        return grades();
      },
    });
    renderTab();
    const failed = await screen.findByTestId("skill-grades-not-read");
    expect(failed).toHaveTextContent("The grade record could not be read.");
    // The rest of the tab still works.
    expect(screen.getByTestId("skill-grades-after-edit")).toBeTruthy();
    fireEvent.click(within(failed).getByRole("button", { name: "Try again" }));
    await screen.findByRole("group", { name: "Does it get picked? The latest grade of deploy" });
  });
});

describe("the grading pages' path (S6)", () => {
  it("names each page after the skill's Grades, and a chat's way back to its scorecard", () => {
    expect(gradingPath({ at: "plan" }).map((one) => one.label)).toEqual(["New grade"]);
    expect(gradingPath({ at: "plan", afterEdit: true }).map((one) => one.label)).toEqual([
      "Grade after each edit",
    ]);
    expect(gradingPath({ at: "running", grade: "g" }).map((one) => one.label)).toEqual([
      "Grading…",
    ]);
    const chat = { at: "chat", grade: "g", chat: "c" } as const;
    expect(gradingPath(chat).map((one) => one.label)).toEqual(["Scorecard", "Saved chat"]);
    expect(gradingCrumbTo(chat, "scorecard")).toEqual({ at: "scorecard", grade: "g" });
    expect(gradingCrumbTo(chat, "grading")).toBeUndefined();
  });
});
