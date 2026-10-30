// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The grade plan and a grade running, as pages** (#3688, S6), drawn at the window seam with the
 * core faked at its command boundary: what the plan says before anything runs, Start and what it
 * spends, the plan that changed under the person refused until they read it, and a grade's
 * progress with Stop.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import {
  GradePlanPage,
  GradeRunningPage,
  afterEditSaid,
  costSaid,
  endedSaid,
  limitSaid,
} from "./SkillGrade";
import type { SkillGradePlan } from "./bindings";

afterEach(() => {
  cleanup();
  clearMocks();
});

function plan(over: Partial<SkillGradePlan> = {}): SkillGradePlan {
  return {
    tests: [
      { query: "ship it to production", should_trigger: true, hides: false },
      { query: "what is prod⟦U+202E⟧", should_trigger: false, hides: true },
    ],
    chats: 12,
    runs: 3,
    tests_refused: null,
    left_out: [],
    outcome_evals: 1,
    outcomes_refused: null,
    estimate: null,
    record_said: null,
    running: null,
    token_cap: 200000,
    cost_cap_usd: 15,
    priced: { usd_per_million: 75, of_the_model: false, model: null },
    cap_set_here: false,
    chat_minutes: 5,
    profile: "work",
    models: ["opus", "sonnet", "haiku"],
    model: null,
    sandbox: "full sandbox",
    refused: null,
    consent: "c0ffee",
    previous: null,
    previous_refused: null,
    ...over,
  };
}

const LEVEL = { at: "project", local: false } as const;

/** The plan's Start, as its words say it for a plan of `chats`. */
const start = (chats = 12) =>
  screen.getByRole("button", { name: `Start grading · ${chats} chats` });

/** Whether `button` can't be pressed now: disabled, or unavailable with its reason. */
const unavailable = (button: HTMLElement) =>
  button.hasAttribute("disabled") || button.getAttribute("aria-disabled") === "true";

/** The plan page of deploy, and what it told the studio. */
function renderPlan(over: { afterEdit?: boolean; changed?: number } = {}) {
  const told = {
    started: vi.fn<(grade: string) => void>(),
    afterEdit: vi.fn(),
    running: vi.fn<(grade: string) => void>(),
    cancel: vi.fn(),
  };
  const page = (changed: number) => (
    <GradePlanPage
      plane="p"
      name="deploy"
      level={LEVEL}
      changed={changed}
      afterEdit={over.afterEdit}
      onStarted={told.started}
      onAfterEdit={told.afterEdit}
      onRunning={told.running}
      onCancel={told.cancel}
    />
  );
  const drawn = render(page(over.changed ?? 0));
  return { ...told, rerender: (changed: number) => drawn.rerender(page(changed)) };
}

/** A step of deploy's grade 01GRADE, as the core tells it. */
const step = (over: Record<string, unknown> = {}) => ({
  plane: "p",
  skill: "deploy",
  grade: "01GRADE",
  ran: 0,
  of: 12,
  tokens: null,
  cost_usd: null,
  limit_usd: 15,
  cap: 200000,
  asked: 0,
  ended: null,
  said: null,
  ...over,
});

/** deploy's grades as the core answers them, with the grade `running` at its latest step. */
const grades = (running: ReturnType<typeof step> | null) => ({
  grades: [],
  said: null,
  tests: 2,
  tests_refused: null,
  running,
});

describe("Grade, cost first", () => {
  it("shows what a grade runs and who pays before anything runs, then starts it on the press", async () => {
    const asked: Array<{ cmd: string; args: unknown }> = [];
    mockIPC(
      (cmd, args) => {
        asked.push({ cmd, args });
        if (cmd === "skill_grade_plan") return plan();
        if (cmd === "start_options") return { profiles: [{ name: "work" }] };
        if (cmd === "skill_grade_start") return "01GRADE";
        return null;
      },
      { shouldMockEvents: true },
    );
    const told = renderPlan();
    // The totals come first: the chats, the most it may spend, the estimate, what it runs on.
    const totals = await screen.findByRole("group", {
      name: "What a grade of deploy runs and spends",
    });
    expect(totals).toHaveTextContent("Chats12");
    expect(totals).toHaveTextContent("At most$15.00");
    expect(totals).toHaveTextContent("EstimateNot known");
    expect(totals).toHaveTextContent("Runs onIts own modelwork");
    const cost = screen.getByTestId("skill-grade-cost");
    expect(cost.textContent).toContain("12 chats, one turn each");
    expect(cost.textContent).toContain("up to 12 chats × 5 min");
    expect(cost.textContent).toContain(
      "in the project's full sandbox, with none of your own Allows",
    );
    expect(cost.textContent).toContain(
      "2 trigger tests 3 times with the skill and 3 times without it",
    );
    expect(cost.textContent).toContain("has not been graded here before");
    expect(cost.textContent).toContain("its account pays for them");
    expect(cost.textContent).toContain(
      "It stops once its chats have spent 200,000 tokens, the default cap, or cost $15.00, its tokens at the dearest known price of $75.00 a million tokens, as no model is named for its chats.",
    );
    expect(screen.getAllByTestId("skill-grade-test")[1].textContent).toContain(
      "holds a character no reader would see",
    );
    expect(screen.getByText(/1 outcome eval waits/)).toBeTruthy();
    expect(asked.some((one) => one.cmd === "skill_grade_start")).toBe(false);

    // Start, fixed at the foot, says what it may spend of whose account.
    expect(start()).toHaveAccessibleDescription(
      "Nothing runs until you press Start. It spends at most $15.00 of work's account.",
    );
    fireEvent.click(start());
    await waitFor(() => expect(told.started).toHaveBeenCalledTimes(1));
    expect(told.started.mock.calls[0][0]).toBe("01GRADE");
    expect(asked.find((one) => one.cmd === "skill_grade_start")?.args).toMatchObject({
      plane: "p",
      skill: "deploy",
      profile: null,
      consent: "c0ffee",
    });
  });

  it("says the previous version it compares with, and why one is not (SK-57)", async () => {
    mockIPC(
      (cmd) => {
        if (cmd === "skill_grade_plan")
          return plan({
            chats: 18,
            previous: { commit: "fedcba987654", subject: "step two" },
          });
        if (cmd === "start_options") return { profiles: [] };
        return null;
      },
      { shouldMockEvents: true },
    );
    renderPlan();
    const cost = await screen.findByTestId("skill-grade-cost");
    expect(cost.textContent).toContain(
      "3 times without it, and 3 times with its previous version, fedcba987654 (step two), which it is compared with, up to 18 chats",
    );
    cleanup();
    clearMocks();
    mockIPC(
      (cmd) => {
        if (cmd === "skill_grade_plan")
          return plan({
            previous_refused: "version fedcba987654 of deploy is not a whole skill of this name",
          });
        if (cmd === "start_options") return { profiles: [] };
        return null;
      },
      { shouldMockEvents: true },
    );
    renderPlan();
    expect((await screen.findByTestId("skill-grade-previous-refused")).textContent).toContain(
      "not a whole skill of this name",
    );
  });

  it("refuses a plan that changed under you until you read the new one", async () => {
    let plans = 0;
    const asked: string[] = [];
    mockIPC(
      (cmd) => {
        asked.push(cmd);
        if (cmd === "skill_grade_plan") {
          plans += 1;
          return plans === 1 ? plan() : plan({ consent: "f00d", cost_cap_usd: 30, chats: 18 });
        }
        if (cmd === "start_options") return { profiles: [] };
        if (cmd === "skill_grade_start")
          throw "The plan of deploy changed since it was shown (what it grades, the profile or model its chats run on, or its limits), so nothing was started: read the new plan and press again.";
        return null;
      },
      { shouldMockEvents: true },
    );
    const told = renderPlan();
    await screen.findByTestId("skill-grade-cost");
    fireEvent.click(start());

    // The new plan is held, never drawn under Start: Start waits until it is read.
    const changed = await screen.findByTestId("skill-grade-plan-changed");
    expect(changed).toHaveTextContent(
      "The plan changed since you opened it: it now runs 18 chats on work, and may spend at most $30.00. Read it again before you start.",
    );
    expect(screen.getByTestId("skill-grade-cost").textContent).toContain("cost $15.00");
    expect(unavailable(start())).toBe(true);
    fireEvent.click(start());
    expect(asked.filter((one) => one === "skill_grade_start")).toHaveLength(1);

    fireEvent.click(screen.getByRole("button", { name: "Show the new plan" }));
    await waitFor(() =>
      expect(screen.getByTestId("skill-grade-cost").textContent).toContain("cost $30.00"),
    );
    expect(screen.queryByTestId("skill-grade-plan-changed")).toBeNull();
    expect(unavailable(start(18))).toBe(false);
    expect(told.started).not.toHaveBeenCalled();
  });

  it("holds a plan the project's change made anew, as one that changed under you", async () => {
    let plans = 0;
    mockIPC((cmd) => {
      if (cmd === "skill_grade_plan") {
        plans += 1;
        return plans === 1 ? plan() : plan({ consent: "f00d", tests: [plan().tests[0]], chats: 6 });
      }
      if (cmd === "start_options") return { profiles: [] };
      return null;
    });
    const told = renderPlan();
    await screen.findByTestId("skill-grade-cost");
    told.rerender(1);
    expect(await screen.findByTestId("skill-grade-plan-changed")).toHaveTextContent(
      "it now runs 6 chats on work",
    );
    expect(unavailable(start())).toBe(true);
  });

  it("offers the models the profile's program can run, and grades on the one picked (SK-60)", async () => {
    const asked: Array<{ cmd: string; args: Record<string, unknown> }> = [];
    mockIPC(
      (cmd, args) => {
        asked.push({ cmd, args: args as Record<string, unknown> });
        if (cmd === "skill_grade_plan") {
          const model = (args as { model: string | null }).model;
          return plan({
            model,
            priced:
              model === null
                ? { usd_per_million: 75, of_the_model: false, model: null }
                : { usd_per_million: 22.5, of_the_model: true, model },
          });
        }
        if (cmd === "start_options") return { profiles: [{ name: "work" }] };
        if (cmd === "skill_grade_start") return "01GRADE";
        return null;
      },
      { shouldMockEvents: true },
    );
    renderPlan();
    await screen.findByTestId("skill-grade-cost");
    await userEvent.click(screen.getByRole("button", { name: /Model deploy's grade runs on/ }));
    for (const model of ["The profile's own model", "opus", "sonnet", "haiku"])
      expect(await screen.findByRole("option", { name: model })).toBeTruthy();
    await userEvent.click(screen.getByRole("option", { name: "sonnet" }));
    await waitFor(() =>
      expect(screen.getByTestId("skill-grade-cost").textContent).toContain(
        "its tokens at sonnet's $22.50 a million tokens",
      ),
    );
    expect(asked.filter((one) => one.cmd === "skill_grade_plan").at(-1)?.args).toMatchObject({
      profile: null,
      model: "sonnet",
    });
    fireEvent.click(start());
    await waitFor(() => expect(asked.some((one) => one.cmd === "skill_grade_start")).toBe(true));
    expect(asked.find((one) => one.cmd === "skill_grade_start")?.args).toMatchObject({
      model: "sonnet",
      consent: "c0ffee",
    });
  });

  it("offers no model where the profile's program offers none", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_grade_plan") return plan({ models: [] });
      if (cmd === "start_options") return { profiles: [] };
      return null;
    });
    renderPlan();
    await screen.findByTestId("skill-grade-cost");
    expect(screen.queryByRole("button", { name: /Model deploy's grade runs on/ })).toBeNull();
  });

  it("says in the grade the chats that asked you something (SK-59)", () => {
    const step = {
      plane: "p",
      skill: "deploy",
      grade: "g",
      ran: 10,
      of: 12,
      tokens: null,
      cost_usd: null,
      limit_usd: null,
      cap: 200000,
      asked: 2,
      ended: "whole" as const,
      said: null,
    };
    expect(endedSaid(step)).toBe(
      "Graded in 10 chats. Its scorecard says how each test went. 2 chats asked you something, and purlis ended them there without counting them: the grade's scorecard opens each.",
    );
    expect(endedSaid({ ...step, asked: 0 })).toBe(
      "Graded in 10 chats. Its scorecard says how each test went.",
    );
  });

  it("says a skill with no trigger tests has none, and starts nothing", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_grade_plan") return plan({ tests: [], chats: 0, outcome_evals: 0 });
      if (cmd === "start_options") return { profiles: [] };
      return null;
    });
    renderPlan();
    await screen.findByTestId("skill-grade-none");
    expect(unavailable(start(0))).toBe(true);
  });

  it("says what the last grade spent per chat, and never a figure it did not measure", () => {
    const from = {
      ts: "2026-10-25T10:00:00+00:00",
      profile: "work",
      harness: "Claude Code",
      model: null as string | null,
      same_profile: true as boolean | null,
    };
    expect(
      costSaid(plan({ estimate: { tokens: 120000, cost_usd: 1.2, chats_then: 6, ...from } })),
    ).toBe(
      "From its grade on work (Claude Code), 2026-10-25, of 6 chats, this one should spend about 120,000 tokens, about $1.20.",
    );
    expect(
      costSaid(
        plan({
          estimate: { tokens: 6000, cost_usd: null, chats_then: 3, ...from, same_profile: false },
        }),
      ),
    ).toContain("From its grade on another profile, work (Claude Code)");
    // SK-60: the model its chats reported is named, as another model spends otherwise.
    expect(
      costSaid(
        plan({
          estimate: {
            tokens: 6000,
            cost_usd: null,
            chats_then: 3,
            ...from,
            model: "claude-haiku-4-5",
          },
        }),
      ),
    ).toContain("From its grade on work (Claude Code, claude-haiku-4-5)");
    // #3273: graded, and its chats said nothing of what they spent: the cost is not known.
    const unknown = costSaid(
      plan({ estimate: { tokens: null, cost_usd: null, chats_then: 6, ...from } }),
    );
    expect(unknown).toContain("did not say what its chats spent");
    expect(unknown).not.toContain("not been graded");
    expect(costSaid(plan())).toContain("has not been graded here before");
    expect(
      costSaid(
        plan({
          estimate: { tokens: null, cost_usd: null, chats_then: 6, ...from, same_profile: null },
        }),
      ),
    ).toContain("Its last grade on work (Claude Code)");
    // A grade purlis stopped under before any chat ran: graded, and no chat to go by.
    expect(
      costSaid(plan({ estimate: { tokens: null, cost_usd: null, chats_then: 0, ...from } })),
    ).toContain("ran no chat");
    expect(
      endedSaid({
        plane: "p",
        skill: "deploy",
        grade: "g",
        ran: 1,
        of: 12,
        tokens: null,
        cost_usd: null,
        limit_usd: null,
        cap: 200000,
        asked: 0,
        ended: "chats-did-not-start",
        said: "The default profile is signed out.",
      }),
    ).toBe("The grade stopped: its chats did not start. The default profile is signed out.");
  });
});

describe("a grade's token cap", () => {
  it("is set from the plan, before anything runs, and the plan says it again", async () => {
    const asked: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "skill_grade_plan") return plan();
      if (cmd === "skill_grade_set_cap")
        return plan({ token_cap: 50000, cost_cap_usd: 3.75, cap_set_here: true });
      if (cmd === "start_options") return { profiles: [] };
      return null;
    });
    renderPlan();
    const field = await screen.findByRole("textbox", { name: "Token cap" });
    fireEvent.change(field, { target: { value: "50,000" } });
    fireEvent.click(screen.getByRole("button", { name: "Set the cap" }));
    await waitFor(() =>
      expect(screen.getByTestId("skill-grade-cost").textContent).toContain(
        "have spent 50,000 tokens, the cap you set on this machine, or cost $3.75, its tokens at the dearest known price",
      ),
    );
    expect(asked.find((one) => one.cmd === "skill_grade_set_cap")?.args).toMatchObject({
      tokens: 50000,
    });
    expect(asked.some((one) => one.cmd === "skill_grade_start")).toBe(false);
  });

  it("says what the grade record left out, beside the plan (#3273)", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_grade_plan")
        return plan({ record_said: "A row of the grade record was skipped." });
      if (cmd === "start_options") return { profiles: [] };
      return null;
    });
    renderPlan();
    expect((await screen.findByTestId("skill-grade-record")).textContent).toBe(
      "A row of the grade record was skipped.",
    );
  });

  it("says a grade stopped at its cap, and what it spent", () => {
    expect(
      endedSaid({
        plane: "p",
        skill: "deploy",
        grade: "g",
        ran: 9,
        of: 12,
        tokens: 201345,
        cost_usd: null,
        limit_usd: null,
        cap: 200000,
        asked: 0,
        ended: "over-cap",
        said: null,
      }),
    ).toBe(
      "Stopped at its cap of 200,000 tokens after 9 chats, having spent 201,345 tokens. What ran is scored.",
    );
  });
});

describe("a grade's dollar limit (#3469)", () => {
  it("says the price it was worked out at: the model's, or the dearest and why", () => {
    expect(
      limitSaid(
        plan({
          cost_cap_usd: 3,
          priced: { usd_per_million: 15, of_the_model: true, model: "claude-sonnet-4-6" },
        }),
      ),
    ).toBe("$3.00, its tokens at claude-sonnet-4-6's $15.00 a million tokens");
    expect(limitSaid(plan())).toBe(
      "$15.00, its tokens at the dearest known price of $75.00 a million tokens, as no model is named for its chats",
    );
    expect(
      limitSaid(plan({ priced: { usd_per_million: 75, of_the_model: false, model: "opus-9" } })),
    ).toContain("as opus-9's price is not known");
  });

  it("says a grade stopped at its dollar limit", () => {
    expect(
      endedSaid({
        plane: "p",
        skill: "deploy",
        grade: "g",
        ran: 3,
        of: 12,
        tokens: 30000,
        cost_usd: null,
        limit_usd: null,
        cap: 200000,
        asked: 0,
        ended: "over-limit",
        said: null,
      }),
    ).toBe(
      "Stopped at its dollar limit after 3 chats, having spent 30,000 tokens. What ran is scored.",
    );
  });
});

describe("a trigger test a grade leaves out", () => {
  it("is named by its number in the file, with why (#3275)", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_grade_plan")
        return plan({ left_out: [{ at: 1, why: "it starts with a dash" }] });
      if (cmd === "start_options") return { profiles: [] };
      return null;
    });
    renderPlan();
    expect((await screen.findByTestId("skill-grade-left-out")).textContent).toBe(
      "Left out: Test 2: it starts with a dash",
    );
  });
});

describe("a grade a fault ended", () => {
  it("says it ended before it was scored, and is told as ended (#3353)", () => {
    expect(
      endedSaid({
        plane: "p",
        skill: "deploy",
        grade: "g",
        ran: 0,
        of: 12,
        tokens: null,
        cost_usd: null,
        limit_usd: null,
        cap: 200000,
        asked: 0,
        ended: "failed",
        said: "A fault in purlis ended the grade before it was scored.",
      }),
    ).toBe("A fault in purlis ended the grade before it was scored.");
  });
});

describe("a grade that cannot run, or runs already", () => {
  it("says why no grade can run on the profile, and offers no Grade press", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_grade_plan")
        return plan({
          sandbox: null,
          refused:
            "Its chats would start with the sandbox off, and a grade runs only in the full sandbox.",
        });
      if (cmd === "start_options") return { profiles: [] };
      return null;
    });
    renderPlan();
    const said = await screen.findByTestId("skill-grade-cannot-run");
    expect(said.textContent).toContain(
      "No grade can run on work: Its chats would start with the sandbox off",
    );
    expect(unavailable(start())).toBe(true);
  });

  it("offers to show a grade running already, and starts no second one", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_grade_plan") return plan({ running: "01RUNNING" });
      if (cmd === "start_options") return { profiles: [] };
      return null;
    });
    const told = renderPlan();
    await screen.findByTestId("skill-grade-cost");
    expect(unavailable(start())).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Show it" }));
    expect(told.running).toHaveBeenCalledWith("01RUNNING");
  });
});

describe("a grade running (S6)", () => {
  it("shows its chats, its spend against the limit and its tokens, with Stop", async () => {
    const asked: Array<{ cmd: string; args: unknown }> = [];
    mockIPC(
      (cmd, args) => {
        asked.push({ cmd, args });
        if (cmd === "skill_grades") return grades(step());
        if (cmd === "skill_grade_stop") throw "That grade is not running here.";
        return null;
      },
      { shouldMockEvents: true },
    );
    const onEnded = vi.fn();
    render(
      <GradeRunningPage
        plane="p"
        name="deploy"
        level={LEVEL}
        grade="01GRADE"
        onEnded={onEnded}
        onBack={() => undefined}
      />,
    );
    const totals = await screen.findByRole("group", { name: "How far the grade of deploy is" });
    expect(totals).toHaveTextContent("Chats0 of 12");
    expect(totals).toHaveTextContent("of at most $15.00");
    // Its skill's grades say where it is: no plan is asked (#3928).
    expect(asked.some((one) => one.cmd === "skill_grade_plan")).toBe(false);

    await emit("skill-grade", step({ ran: 4, tokens: 36000, cost_usd: 0.32 }));
    await waitFor(() => expect(totals).toHaveTextContent("Chats4 of 12"));
    expect(totals).toHaveTextContent("Spent$0.32of at most $15.00");
    expect(totals).toHaveTextContent("Tokens36,000of 200,000 tokens");
    // Another skill's grade, or another grade, moves nothing here.
    await emit("skill-grade", step({ grade: "01OTHER", ran: 9 }));
    expect(totals).toHaveTextContent("Chats4 of 12");

    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_grade_stop")?.args).toMatchObject({
        grade: "01GRADE",
      }),
    );
    // A stop the core refused says so, and Stop can be pressed again.
    await screen.findByText("That grade is not running here.");
    await waitFor(() =>
      expect(unavailable(screen.getByRole("button", { name: "Stop" }))).toBe(false),
    );
    await emit("skill-grade", step({ ran: 4, ended: "stopped" }));
    await waitFor(() => expect(onEnded).toHaveBeenCalledTimes(1));
  });

  it("stays to say an end with words, with a way to its scorecard", async () => {
    mockIPC(
      (cmd) => {
        if (cmd === "skill_grades") return grades(step());
        return null;
      },
      { shouldMockEvents: true },
    );
    const onEnded = vi.fn();
    render(
      <GradeRunningPage
        plane="p"
        name="deploy"
        level={LEVEL}
        grade="01GRADE"
        onEnded={onEnded}
        onBack={() => undefined}
      />,
    );
    await screen.findByRole("group", { name: "How far the grade of deploy is" });
    await emit(
      "skill-grade",
      step({ ended: "chats-did-not-start", said: "The default profile is signed out." }),
    );
    await screen.findByText(
      "The grade stopped: its chats did not start. The default profile is signed out.",
    );
    expect(onEnded).not.toHaveBeenCalled();
    expect(unavailable(screen.getByRole("button", { name: "Stop" }))).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Open its scorecard" }));
    expect(onEnded).toHaveBeenCalledTimes(1);
  });

  it("starts from the step the app last told, where it opens while the grade runs", async () => {
    mockIPC((cmd) => (cmd === "skill_grades" ? grades(step({ ran: 7, cost_usd: 1.5 })) : null), {
      shouldMockEvents: true,
    });
    render(
      <GradeRunningPage
        plane="p"
        name="deploy"
        level={LEVEL}
        grade="01GRADE"
        onEnded={() => undefined}
        onBack={() => undefined}
      />,
    );
    const totals = await screen.findByRole("group", { name: "How far the grade of deploy is" });
    expect(totals).toHaveTextContent("Chats7 of 12");
    expect(totals).toHaveTextContent("Spent$1.50of at most $15.00");
  });

  it("says a grade that ended while the page was away, and opens its scorecard", async () => {
    mockIPC((cmd) => (cmd === "skill_grades" ? grades(null) : null));
    const onEnded = vi.fn();
    render(
      <GradeRunningPage
        plane="p"
        name="deploy"
        level={LEVEL}
        grade="01GRADE"
        onEnded={onEnded}
        onBack={() => undefined}
      />,
    );
    const gone = await screen.findByTestId("skill-grade-gone");
    fireEvent.click(within(gone).getByRole("button", { name: "Open its scorecard" }));
    expect(onEnded).toHaveBeenCalledTimes(1);
  });
});

describe("Grade after each edit (SK-58), from its plan", () => {
  const ON = {
    profile: "work",
    model: "sonnet",
    chats: 12,
    token_cap: 200000,
    limit_usd: 15,
    waits: null,
  };

  it("says what each edit runs and spends, and turns on only on its press", async () => {
    const asked: Array<{ cmd: string; args: Record<string, unknown> }> = [];
    mockIPC(
      (cmd, args) => {
        asked.push({ cmd, args: args as Record<string, unknown> });
        if (cmd === "skill_grade_plan") return plan();
        if (cmd === "start_options") return { profiles: [{ name: "work" }] };
        if (cmd === "skill_grade_after_edit_on") return ON;
        return null;
      },
      { shouldMockEvents: true },
    );
    const told = renderPlan({ afterEdit: true });
    const offer = await screen.findByTestId("skill-grade-after-edit-offer");
    expect(offer.textContent).toContain("up to 12 chats and at most $15.00");
    const turnOn = screen.getByRole("button", { name: "Turn on grading after each edit" });
    expect(turnOn).toHaveAccessibleDescription(
      "Nothing runs now. Each edit you save then runs up to 12 chats, at most $15.00 each time, of work's account.",
    );
    expect(asked.some((one) => one.cmd === "skill_grade_after_edit_on")).toBe(false);

    fireEvent.click(turnOn);
    await waitFor(() => expect(told.afterEdit).toHaveBeenCalledWith(ON));
    expect(asked.find((one) => one.cmd === "skill_grade_after_edit_on")?.args).toMatchObject({
      plane: "p",
      skill: "deploy",
      profile: null,
      model: null,
      consent: "c0ffee",
    });
    expect(asked.some((one) => one.cmd === "skill_grade_start")).toBe(false);
    expect(afterEditSaid(ON)).toBe(
      "Each edit you save grades this skill on work, sonnet: up to 12 chats, and at most $15.00 or 200,000 tokens, each time, paid by that profile's account.",
    );
  });
});
