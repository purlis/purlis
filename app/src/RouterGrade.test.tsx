// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import {
  RouterGradeChatPage,
  RouterGradePlanPage,
  RouterGradeRunningPage,
  RouterGradeScorecardPage,
  routerCostSaid,
} from "./RouterGrade";
import { endSaid } from "./SkillGrade";
import type {
  RouterGradePlan,
  RouterGradeScorecard,
  RouterGradeStep,
  ScoredTest,
} from "./bindings";
import {
  ROUTER_GRADE_PLAN,
  routerGradeItem,
  routerGradeOfItem,
  routerGradingItem,
  routerGradingOfItem,
} from "./skillStudio";

afterEach(() => {
  cleanup();
  clearMocks();
});

// jsdom has no `matchMedia`, and xterm asks it for the display's pixel ratio as it opens.
vi.stubGlobal("matchMedia", (query: string) => ({
  matches: false,
  media: query,
  addEventListener: () => {},
  removeEventListener: () => {},
  addListener: () => {},
  removeListener: () => {},
}));

function plan(over: Partial<RouterGradePlan> = {}): RouterGradePlan {
  return {
    router: "relevance",
    library: 3,
    skills: [
      { skill: "deploy", tests: 2, left_out: [], refused: null },
      {
        skill: "rollback",
        tests: 1,
        left_out: [{ at: 1, why: "it starts with -" }],
        refused: null,
      },
    ],
    chats: 18,
    runs: 3,
    chat_minutes: 5,
    commit: "a".repeat(40),
    token_cap: 200000,
    cap_set_here: false,
    cost_cap_usd: 15,
    priced: { usd_per_million: 75, of_the_model: false, model: null },
    profile: "work",
    sandbox: "full sandbox",
    refused: null,
    models: ["opus", "sonnet"],
    model: null,
    estimate: null,
    estimate_same_library: null,
    running: null,
    consent: "c0ffee",
    ...over,
  };
}

/** The plan's Start, as its words say it for a plan of `chats`. */
const start = (chats = 18) =>
  screen.getByRole("button", { name: `Start grading · ${chats} chats` });

/** Whether `button` can't be pressed now: disabled, or unavailable with its reason. */
const unavailable = (button: HTMLElement) =>
  button.hasAttribute("disabled") || button.getAttribute("aria-disabled") === "true";

/** The plan page of the router relevance, and what it told Routing. */
function renderPlan(over: { workspace?: string | null; persona?: string | null } = {}) {
  const told = {
    started: vi.fn<(grade: string) => void>(),
    running: vi.fn<(grade: string) => void>(),
    cancel: vi.fn(),
  };
  const page = (changed: number) => (
    <RouterGradePlanPage
      plane="p"
      router="relevance"
      workspace={over.workspace === undefined ? "ops" : over.workspace}
      persona={over.persona ?? null}
      changed={changed}
      onStarted={told.started}
      onRunning={told.running}
      onCancel={told.cancel}
    />
  );
  const drawn = render(page(0));
  return { ...told, rerender: (changed: number) => drawn.rerender(page(changed)) };
}

/** A step of the router relevance's grade 01GRADE, as the core tells it. */
const step = (over: Partial<RouterGradeStep> = {}): RouterGradeStep => ({
  plane: "p",
  router: "relevance",
  workspace: "ops",
  persona: null,
  grade: "01GRADE",
  ran: 0,
  of: 18,
  tokens: null,
  cost_usd: null,
  limit_usd: 15,
  cap: 200000,
  ended: null,
  said: null,
  recorded: true,
  ...over,
});

describe("Grade the router: its plan, a review page on Routing (SK-15e, #4222)", () => {
  it("shows its totals and what it runs before anything runs, then starts it on Start", async () => {
    const asked: Array<{ cmd: string; args: unknown }> = [];
    mockIPC(
      (cmd, args) => {
        asked.push({ cmd, args });
        if (cmd === "router_grade_plan") return plan();
        if (cmd === "start_options") return { profiles: [] };
        if (cmd === "router_grade_start") return "01GRADE";
        return null;
      },
      { shouldMockEvents: true },
    );
    const told = renderPlan();
    const cost = await screen.findByTestId("router-grade-cost");
    const totals = screen.getByRole("group", {
      name: "What a grade of the router relevance runs and spends",
    });
    expect(totals).toHaveTextContent("Chats183 tests × 3 × 2 arms");
    expect(totals).toHaveTextContent("At most$15.00or 200,000 tokens");
    expect(totals).toHaveTextContent("EstimateNot knownnot graded here before");
    expect(totals).toHaveTextContent("Runs onIts own modelwork");
    expect(cost.textContent).toContain("18 chats, one turn each");
    expect(cost.textContent).toContain(
      "3 trigger tests of 2 skills 3 times with the router relevance in place and 3 times with the built-in router",
    );
    expect(cost.textContent).toContain("handed the library of 3 skills");
    expect(cost.textContent).toContain("200,000 tokens, the default cap, or cost $15.00");
    expect(cost.textContent).toContain("its account pays for them");
    expect(cost.textContent).toContain("recorded on the library at aaaaaaaaaaaa");
    expect(screen.getAllByTestId("router-grade-skill")[1].textContent).toContain(
      "left out: test 2: it starts with -",
    );
    expect(screen.getByText(/Nothing runs until you press Start/)).toHaveTextContent(
      "It spends at most $15.00 of work's account.",
    );
    expect(asked.find((one) => one.cmd === "router_grade_plan")?.args).toMatchObject({
      plane: "p",
      workspace: "ops",
      persona: null,
    });
    expect(asked.some((one) => one.cmd === "router_grade_start")).toBe(false);

    fireEvent.click(start());
    await waitFor(() => expect(told.started).toHaveBeenCalledWith("01GRADE"));
    expect(asked.find((one) => one.cmd === "router_grade_start")?.args).toMatchObject({
      consent: "c0ffee",
    });
  });

  it("offers no Start where no grade can run, or nothing is tested", async () => {
    mockIPC((cmd) =>
      cmd === "router_grade_plan" ? plan({ refused: "it is not sandboxed" }) : null,
    );
    renderPlan({ workspace: null, persona: "r" });
    await screen.findByTestId("router-grade-cannot-run");
    expect(unavailable(start())).toBe(true);
    cleanup();
    clearMocks();
    mockIPC((cmd) => (cmd === "router_grade_plan" ? plan({ chats: 0, skills: [] }) : null));
    renderPlan({ workspace: null, persona: "r" });
    await screen.findByTestId("router-grade-none");
    expect(unavailable(start(0))).toBe(true);
  });

  it("refuses a plan that changed under you until you read the new one", async () => {
    let plans = 0;
    const asked: string[] = [];
    mockIPC(
      (cmd) => {
        asked.push(cmd);
        if (cmd === "router_grade_plan") {
          plans += 1;
          return plans === 1 ? plan() : plan({ consent: "f00d", cost_cap_usd: 30, chats: 24 });
        }
        if (cmd === "start_options") return { profiles: [] };
        if (cmd === "router_grade_start")
          throw "The router's plan changed since it was shown (its library or trigger tests, the profile or model its chats run on, or its limits), so nothing was started: read the plan again.";
        return null;
      },
      { shouldMockEvents: true },
    );
    const told = renderPlan();
    await screen.findByTestId("router-grade-cost");
    fireEvent.click(start());

    // The new plan is held, never drawn under Start: Start waits until it is read.
    const changed = await screen.findByTestId("router-grade-plan-changed");
    expect(changed).toHaveTextContent(
      "The plan changed since you opened it: it now runs 24 chats on work, and may spend at most $30.00. Read it again before you start.",
    );
    expect(screen.getByTestId("router-grade-cost").textContent).toContain("cost $15.00");
    expect(unavailable(start())).toBe(true);
    fireEvent.click(start());
    expect(asked.filter((one) => one === "router_grade_start")).toHaveLength(1);

    fireEvent.click(screen.getByRole("button", { name: "Show the new plan" }));
    await waitFor(() =>
      expect(screen.getByTestId("router-grade-cost").textContent).toContain("cost $30.00"),
    );
    expect(screen.queryByTestId("router-grade-plan-changed")).toBeNull();
    expect(unavailable(start(24))).toBe(false);
    expect(told.started).not.toHaveBeenCalled();
  });

  it("holds a plan the project's change made anew, and draws one that consents to the same", async () => {
    let plans = 0;
    mockIPC((cmd) => {
      if (cmd === "router_grade_plan") {
        plans += 1;
        // The second read consents to the same; the third does not.
        return plans <= 2 ? plan() : plan({ consent: "f00d", chats: 6 });
      }
      if (cmd === "start_options") return { profiles: [] };
      return null;
    });
    const told = renderPlan();
    await screen.findByTestId("router-grade-cost");
    told.rerender(1);
    await waitFor(() => expect(plans).toBe(2));
    expect(screen.queryByTestId("router-grade-plan-changed")).toBeNull();
    told.rerender(2);
    expect(await screen.findByTestId("router-grade-plan-changed")).toHaveTextContent(
      "it now runs 6 chats on work",
    );
    expect(unavailable(start())).toBe(true);
  });

  it("says a grade runs in the project, and offers its page where it is this router's", async () => {
    mockIPC((cmd, args) => {
      if (cmd === "router_grade_plan") return plan({ running: "01RUNNING" });
      if (cmd === "start_options") return { profiles: [] };
      if (cmd === "router_grade_running")
        return (args as { grade: string }).grade === "01RUNNING"
          ? step({ grade: "01RUNNING", ran: 2 })
          : null;
      return null;
    });
    const told = renderPlan();
    await screen.findByTestId("router-grade-cost");
    expect(unavailable(start())).toBe(true);
    fireEvent.click(await screen.findByRole("button", { name: "Show it" }));
    expect(told.running).toHaveBeenCalledWith("01RUNNING");
    expect(screen.getByText(/A grade of the router relevance is running/)).toBeInTheDocument();
  });

  it("offers no page of the same router's grade in another scope", async () => {
    mockIPC((cmd) => {
      if (cmd === "router_grade_plan") return plan({ running: "01THERE" });
      if (cmd === "start_options") return { profiles: [] };
      if (cmd === "router_grade_running") return step({ grade: "01THERE", workspace: "web" });
      return null;
    });
    renderPlan();
    await screen.findByTestId("router-grade-another-running");
    await new Promise((settled) => setTimeout(settled, 0));
    expect(screen.queryByRole("button", { name: "Show it" })).toBeNull();
  });

  it("frees Start when a skill's grade running in the project ends", async () => {
    let plans = 0;
    mockIPC(
      (cmd) => {
        if (cmd === "router_grade_plan") {
          plans += 1;
          return plan({ running: plans === 1 ? "01SKILL" : null });
        }
        if (cmd === "start_options") return { profiles: [] };
        return null;
      },
      { shouldMockEvents: true },
    );
    renderPlan();
    await screen.findByTestId("router-grade-another-running");
    expect(unavailable(start())).toBe(true);
    await emit("skill-grade", { plane: "p", grade: "01SKILL", ended: "whole" });
    await waitFor(() => expect(unavailable(start())).toBe(false));
  });

  it("names no grade of another's to show", async () => {
    mockIPC((cmd) => {
      if (cmd === "router_grade_plan") return plan({ running: "01SKILL" });
      if (cmd === "start_options") return { profiles: [] };
      // A skill's grade: the app holds no router step for it.
      if (cmd === "router_grade_running") return null;
      return null;
    });
    renderPlan();
    expect(
      await screen.findByText("A grade is running in this project: one runs at a time."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Show it" })).toBeNull();
  });

  it("runs on the profile and model picked, held to the cap set, consented to on the plan shown (#3618)", async () => {
    const asked: Array<{ cmd: string; args: Record<string, unknown> }> = [];
    mockIPC(
      (cmd, args) => {
        asked.push({ cmd, args: args as Record<string, unknown> });
        if (cmd === "router_grade_plan") {
          const { profile, model } = args as { profile: string | null; model: string | null };
          return plan({ profile: profile ?? "work", model, consent: `c-${profile}-${model}` });
        }
        if (cmd === "router_grade_set_cap")
          return plan({
            token_cap: 50000,
            cost_cap_usd: 3.75,
            cap_set_here: true,
            consent: "capped",
          });
        if (cmd === "start_options") return { profiles: [{ name: "work" }, { name: "home" }] };
        if (cmd === "router_grade_start") return "01GRADE";
        return null;
      },
      { shouldMockEvents: true },
    );
    const told = renderPlan();
    await screen.findByTestId("router-grade-cost");
    await userEvent.click(
      screen.getByRole("button", { name: /Model the router relevance's grade runs on/ }),
    );
    await userEvent.click(await screen.findByRole("option", { name: "sonnet" }));
    await waitFor(() =>
      expect(screen.getByTestId("router-grade-cost").textContent).toContain("on work, on sonnet"),
    );
    expect(asked.filter((one) => one.cmd === "router_grade_plan").at(-1)?.args).toMatchObject({
      profile: null,
      model: "sonnet",
    });

    const field = screen.getByRole("textbox", { name: "Token cap" });
    fireEvent.change(field, { target: { value: "50,000" } });
    fireEvent.click(screen.getByRole("button", { name: "Set the cap" }));
    await waitFor(() =>
      expect(screen.getByTestId("router-grade-cost").textContent).toContain(
        "have spent 50,000 tokens, the cap you set on this machine, or cost $3.75",
      ),
    );
    expect(asked.find((one) => one.cmd === "router_grade_set_cap")?.args).toMatchObject({
      workspace: "ops",
      model: "sonnet",
      tokens: 50000,
    });
    expect(asked.some((one) => one.cmd === "router_grade_start")).toBe(false);

    fireEvent.click(start());
    await waitFor(() => expect(told.started).toHaveBeenCalledWith("01GRADE"));
    expect(asked.find((one) => one.cmd === "router_grade_start")?.args).toMatchObject({
      profile: null,
      model: "sonnet",
      consent: "capped",
    });
  });

  it("says what its last grade spent per chat, or that it is not known (#3618)", () => {
    const from = {
      ts: "2026-10-25T10:00:00+00:00",
      profile: "work",
      harness: "Claude Code",
      model: "sonnet",
      same_profile: true,
      interrupted: false,
    };
    expect(routerCostSaid(plan())).toBe(
      "purlis knows no grade of this router here to go by (one purlis stopped under before it scored a test says nothing of what a grade spends), so it cannot say yet what this one spends. Each chat is one turn, and ends once the skill it tests starts.",
    );
    const estimated = plan({
      estimate: { tokens: 18000, cost_usd: 0.18, chats_then: 12, ...from },
      estimate_same_library: true,
    });
    expect(routerCostSaid(estimated)).toBe(
      "From its grade on work (Claude Code, sonnet), 2026-10-25, of 12 chats, this one should spend about 18,000 tokens, about $0.18.",
    );
    expect(routerCostSaid({ ...estimated, estimate_same_library: false })).toContain(
      "That grade may have run on another library (its skills differ, or hold changes not committed), and each chat is handed the whole library, so this one may spend otherwise.",
    );
    const unsaid = routerCostSaid(
      plan({
        estimate: { tokens: null, cost_usd: null, chats_then: 12, ...from },
        estimate_same_library: false,
      }),
    );
    expect(unsaid).toContain(
      "did not say what its chats spent, so purlis cannot say what this one spends",
    );
    expect(unsaid).not.toContain("another library");
  });
});

/** The running page of the grade 01GRADE, and what it told Routing. */
function renderRunning() {
  const told = { ended: vi.fn(), back: vi.fn() };
  render(
    <RouterGradeRunningPage
      plane="p"
      router="relevance"
      grade="01GRADE"
      onEnded={told.ended}
      onBack={told.back}
    />,
  );
  return told;
}

describe("A router's grade running, a page on Routing (#4222)", () => {
  it("starts from the step the app holds, moves on each step told, and opens its scorecard at its end", async () => {
    mockIPC(
      (cmd) => {
        if (cmd === "router_grade_running") return step({ ran: 4, tokens: 3000, cost_usd: 0.25 });
        return null;
      },
      { shouldMockEvents: true },
    );
    const told = renderRunning();
    const totals = await screen.findByRole("group", {
      name: "How far the grade of the router relevance is",
    });
    expect(totals).toHaveTextContent("Chats4 of 18");
    expect(totals).toHaveTextContent("Spent$0.25of at most $15.00");
    expect(totals).toHaveTextContent("Tokens3,000of 200,000 tokens");
    // Another grade's step moves nothing here.
    await emit("router-grade", step({ grade: "01OTHER", ran: 9 }));
    await emit("router-grade", step({ ran: 7, tokens: 5000 }));
    await waitFor(() => expect(totals).toHaveTextContent("Chats7 of 18"));
    expect(told.ended).not.toHaveBeenCalled();

    await emit("router-grade", step({ ran: 18, ended: "whole" }));
    await waitFor(() => expect(told.ended).toHaveBeenCalledTimes(1));
  });

  it("stops it on Stop, and keeps it going on Leave it running", async () => {
    const asked: Array<{ cmd: string; args: unknown }> = [];
    mockIPC(
      (cmd, args) => {
        asked.push({ cmd, args });
        if (cmd === "router_grade_running") return step({ ran: 1 });
        return null;
      },
      { shouldMockEvents: true },
    );
    const told = renderRunning();
    await screen.findByRole("group", { name: "How far the grade of the router relevance is" });
    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_grade_stop")?.args).toMatchObject({
        plane: "p",
        grade: "01GRADE",
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Leave it running" }));
    expect(told.back).toHaveBeenCalledTimes(1);
  });

  it("stays to say an end its scorecard cannot, with the way to it", async () => {
    mockIPC((cmd) => (cmd === "router_grade_running" ? step({ ran: 2 }) : null), {
      shouldMockEvents: true,
    });
    const told = renderRunning();
    await screen.findByRole("group", { name: "How far the grade of the router relevance is" });
    await emit(
      "router-grade",
      step({
        ran: 2,
        ended: "whole",
        said: "The grade ran, but this project could not be written, so it was not recorded.",
      }),
    );
    expect(await screen.findByTestId("router-grade-ended")).toHaveTextContent(
      "Graded in 2 chats. The grade ran, but this project could not be written, so it was not recorded.",
    );
    expect(told.ended).not.toHaveBeenCalled();
    expect(unavailable(screen.getByRole("button", { name: "Stop" }))).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Open its scorecard" }));
    expect(told.ended).toHaveBeenCalledTimes(1);
  });

  it("offers no scorecard for an end that was not recorded", async () => {
    mockIPC(
      (cmd) =>
        cmd === "router_grade_running"
          ? step({ ended: "whole", said: "It was not recorded.", recorded: false })
          : null,
      { shouldMockEvents: true },
    );
    const told = renderRunning();
    await screen.findByTestId("router-grade-ended");
    expect(screen.queryByRole("button", { name: "Open its scorecard" })).toBeNull();
    expect(told.ended).not.toHaveBeenCalled();
  });

  it("reads an end that came before it listened, and opens its scorecard", async () => {
    mockIPC((cmd) => (cmd === "router_grade_running" ? step({ ended: "stopped" }) : null), {
      shouldMockEvents: true,
    });
    const told = renderRunning();
    await waitFor(() => expect(told.ended).toHaveBeenCalled());
  });

  it("keeps Stop and names the grade's own router when another is chosen meanwhile", async () => {
    mockIPC((cmd) => (cmd === "router_grade_running" ? step({ ran: 1 }) : null), {
      shouldMockEvents: true,
    });
    const told = { ended: vi.fn(), back: vi.fn() };
    render(
      <RouterGradeRunningPage
        plane="p"
        router={null}
        grade="01GRADE"
        onEnded={told.ended}
        onBack={told.back}
      />,
    );
    await screen.findByRole("group", { name: "How far the grade of the router relevance is" });
    expect(screen.getByRole("button", { name: "Stop" })).toBeInTheDocument();
    await emit("router-grade", step({ ran: 2, ended: "whole" }));
    expect(await screen.findByTestId("router-grade-ended")).toHaveTextContent(
      "no longer chosen here",
    );
    expect(told.ended).not.toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "Open its scorecard" })).toBeNull();
  });

  it("says a grade that ended while the page was away, and opens its scorecard", async () => {
    mockIPC((cmd) => (cmd === "router_grade_running" ? null : null), { shouldMockEvents: true });
    const told = renderRunning();
    await screen.findByTestId("router-grade-gone");
    fireEvent.click(screen.getByRole("button", { name: "Open its scorecard" }));
    expect(told.ended).toHaveBeenCalledTimes(1);
  });

  it("says each way a grade ends, in a skill's grade's words", () => {
    expect(endSaid(step({ ran: 4, ended: "stopped" }))).toBe(
      "Stopped after 4 chats. What ran is scored.",
    );
    expect(endSaid(step({ ran: 4, cap: 1000, ended: "over-cap" }))).toBe(
      "Stopped at its cap of 1,000 tokens after 4 chats. What ran is scored.",
    );
    expect(endSaid(step({ ended: "chats-did-not-start", said: "the router did not answer" }))).toBe(
      "The grade stopped: its chats did not start. the router did not answer",
    );
  });
});

/** One trigger test of `skill`, run three times in each arm, each chat kept where `kept`. */
function scoredTest(skill: string, routerPasses: boolean, kept: boolean): ScoredTest {
  const runs = (arm: "router" | "built-in-router", pass: boolean) => ({
    arm,
    runs: 3,
    triggered: pass ? 3 : 0,
    pass,
    asked: 0,
    chats: [1, 2, 3].map((run) => ({ id: `${skill}-${arm}-${run}`, run, kept })),
  });
  return {
    query: `${skill} it`,
    should_trigger: true,
    arms: [runs("router", routerPasses), runs("built-in-router", true)],
  };
}

function scorecard(over: Partial<RouterGradeScorecard> = {}): RouterGradeScorecard {
  return {
    grade: "01GRADE",
    router: "relevance",
    graded: "2026-10-30 10:00 UTC",
    library: "the library at aaaaaaaaaaaa",
    ran_on: "Claude Code · sonnet",
    ended: "Finished",
    outcome: "graded",
    arms: { router: 50, built_in: 100 },
    spent: { chats: 12, tokens: 12000, cost_usd: 0.12, limit_usd: 1.5 },
    asked: 0,
    skills: [
      { skill: "deploy", tests: [scoredTest("deploy", false, true)] },
      { skill: "rollback", tests: [scoredTest("rollback", true, false)] },
    ],
    ...over,
  };
}

describe("A router's grade's scorecard and its chats (#3617)", () => {
  it("names a router's grade's scorecard and its chats as places on Routing", () => {
    expect(routerGradeOfItem(routerGradeItem({ grade: "01G" }))).toEqual({ grade: "01G" });
    expect(routerGradeOfItem(routerGradeItem({ grade: "01G", chat: "01C" }))).toEqual({
      grade: "01G",
      chat: "01C",
    });
    expect(routerGradeOfItem("chat-4")).toBeUndefined();
    expect(routerGradeOfItem(undefined)).toBeUndefined();
    // Its plan and a grade running are places of their own, never a scorecard (#4222).
    expect(routerGradeOfItem(ROUTER_GRADE_PLAN)).toBeUndefined();
    expect(routerGradeOfItem(routerGradingItem("01G"))).toBeUndefined();
    expect(routerGradingOfItem(routerGradingItem("01G"))).toBe("01G");
    expect(routerGradingOfItem(routerGradeItem({ grade: "01G" }))).toBeUndefined();
    expect(routerGradingOfItem(undefined)).toBeUndefined();
  });

  it("opens each skill's trigger tests in both arms, its totals, and each kept chat", async () => {
    const asked: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "router_grade_scorecard") return scorecard();
      return null;
    });
    const opened: string[] = [];
    render(
      <RouterGradeScorecardPage
        plane="p"
        router="relevance"
        workspace="ops"
        persona={null}
        grade="01GRADE"
        onChat={(chat) => opened.push(chat)}
      />,
    );
    expect((await screen.findByTestId("router-grade-scorecard-about")).textContent).toContain(
      "Graded 2026-10-30 10:00 UTC, on the library at aaaaaaaaaaaa, on Claude Code · sonnet for $0.12. Finished.",
    );
    expect(asked[0].args).toMatchObject({ workspace: "ops", persona: null, grade: "01GRADE" });
    const totals = screen.getByRole("group", { name: "Totals of the grade of relevance" });
    expect(totals).toHaveTextContent("With the router50%of tests passed");
    expect(totals).toHaveTextContent("With the built-in router100%-50 points");
    expect(totals).toHaveTextContent("Spent$0.12of $1.50 · 12 chats");
    expect(screen.getByRole("heading", { name: "deploy" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "rollback" })).toBeTruthy();
    const [deploy, rollback] = await screen.findAllByTestId("grade-scorecard-router");
    expect(deploy).toHaveTextContent("0 of 3");
    expect(deploy).toHaveTextContent("Failed");
    fireEvent.click(deploy.querySelectorAll("button")[0]);
    expect(opened).toEqual(["deploy-router-1"]);
    // Not kept on this machine: marked, not removed.
    expect(rollback.querySelectorAll("button")[0].getAttribute("aria-disabled")).toBe("true");
  });

  it("says a grade purlis stopped under scored no test", async () => {
    mockIPC((cmd) =>
      cmd === "router_grade_scorecard"
        ? scorecard({ outcome: "interrupted", ended: "Interrupted: not scored", skills: [] })
        : null,
    );
    render(
      <RouterGradeScorecardPage
        plane="p"
        router="relevance"
        workspace={null}
        persona={null}
        grade="01GRADE"
        onChat={() => undefined}
      />,
    );
    expect(await screen.findByTestId("router-grade-scorecard-none")).toBeTruthy();
  });

  it("draws a kept chat read-only, with the next of the grade's kept chats", async () => {
    mockIPC((cmd) => {
      if (cmd === "router_grade_scorecard") return scorecard();
      if (cmd === "router_grade_chat") return "\u001b[1mclaude\u001b[0m\r\n> deploy it\r\n";
      return null;
    });
    const opened: string[] = [];
    let back = 0;
    render(
      <RouterGradeChatPage
        plane="p"
        router="relevance"
        workspace="ops"
        persona={null}
        grade="01GRADE"
        chat="deploy-router-1"
        onChat={(chat) => opened.push(chat)}
        onScorecard={() => (back += 1)}
      />,
    );
    await screen.findByTestId("grade-chat");
    await waitFor(() =>
      expect(
        screen.getByRole("heading", { name: "Chat 1 of the 2026-10-30 10:00 UTC grade" }),
      ).toBeTruthy(),
    );
    fireEvent.click(screen.getByRole("button", { name: "Next chat" }));
    expect(opened).toEqual(["deploy-router-2"]);
    fireEvent.click(screen.getByRole("button", { name: "Back to the scorecard" }));
    expect(back).toBe(1);
  });
});
