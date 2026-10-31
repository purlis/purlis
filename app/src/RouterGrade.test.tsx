// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import {
  RouterGrade,
  RouterGradeChatPage,
  RouterGradeScorecardPage,
  routerCostSaid,
  routerEndedSaid,
} from "./RouterGrade";
import type { RouterGradePlan, RouterGradeScorecard, ScoredTest } from "./bindings";
import { routerGradeItem, routerGradeOfItem } from "./skillStudio";

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

describe("Grade the router, cost first (SK-15e)", () => {
  it("shows what grading the router runs before anything runs, then starts it on the press", async () => {
    const asked: Array<{ cmd: string; args: unknown }> = [];
    mockIPC(
      (cmd, args) => {
        asked.push({ cmd, args });
        if (cmd === "router_grade_plan") return plan();
        if (cmd === "router_grade_start") return "01GRADE";
        return null;
      },
      { shouldMockEvents: true },
    );
    let graded = 0;
    render(
      <RouterGrade
        plane="p"
        router="relevance"
        workspace="ops"
        persona={null}
        onGraded={() => (graded += 1)}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Grade the router…" }));
    const cost = await screen.findByTestId("router-grade-cost");
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
    expect(asked.find((one) => one.cmd === "router_grade_plan")?.args).toMatchObject({
      plane: "p",
      workspace: "ops",
      persona: null,
    });
    expect(asked.some((one) => one.cmd === "router_grade_start")).toBe(false);

    fireEvent.click(screen.getByRole("button", { name: "Grade" }));
    await screen.findByTestId("router-grade-running");
    expect(asked.find((one) => one.cmd === "router_grade_start")?.args).toMatchObject({
      consent: "c0ffee",
    });

    await emit("router-grade", {
      plane: "p",
      router: "relevance",
      grade: "01GRADE",
      ran: 18,
      of: 18,
      tokens: 9000,
      cap: 200000,
      ended: "whole",
      said: "Graded on the library at aaaaaaaaaaaa, 2026-10-28: 2 of 3 trigger tests pass with the router relevance, and 3 of 3 with the built-in router.",
    });
    await waitFor(() => expect(graded).toBe(1));
    expect(screen.getByText(/Graded in 18 chats\. Graded on the library/)).toBeInTheDocument();
  });

  it("offers no Grade where no grade can run, or nothing is tested", async () => {
    mockIPC((cmd) =>
      cmd === "router_grade_plan" ? plan({ refused: "it is not sandboxed" }) : null,
    );
    render(
      <RouterGrade
        plane="p"
        router="relevance"
        workspace={null}
        persona="r"
        onGraded={() => undefined}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Grade the router…" }));
    await screen.findByTestId("router-grade-cannot-run");
    expect(screen.getByRole("button", { name: "Grade" })).toBeDisabled();
    cleanup();
    clearMocks();
    mockIPC((cmd) => (cmd === "router_grade_plan" ? plan({ chats: 0, skills: [] }) : null));
    render(
      <RouterGrade
        plane="p"
        router="relevance"
        workspace={null}
        persona="r"
        onGraded={() => undefined}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Grade the router…" }));
    await screen.findByTestId("router-grade-none");
    expect(screen.queryByRole("button", { name: "Grade" })).toBeNull();
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
    render(
      <RouterGrade
        plane="p"
        router="relevance"
        workspace="ops"
        persona={null}
        onGraded={() => undefined}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Grade the router…" }));
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

    fireEvent.click(screen.getByRole("button", { name: "Grade" }));
    await screen.findByTestId("router-grade-running");
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
      "This router has not been graded here before, so purlis cannot say yet what a grade spends. Each chat is one turn, and ends once the skill it tests starts.",
    );
    const estimated = plan({
      estimate: { tokens: 18000, cost_usd: 0.18, chats_then: 12, ...from },
      estimate_same_library: true,
    });
    expect(routerCostSaid(estimated)).toBe(
      "From its grade on work (Claude Code, sonnet), 2026-10-25, of 12 chats, this one should spend about 18,000 tokens, about $0.18.",
    );
    expect(routerCostSaid({ ...estimated, estimate_same_library: false })).toContain(
      "That grade ran on another library, and each chat is handed the whole library, so this one may spend otherwise.",
    );
    expect(
      routerCostSaid(
        plan({
          estimate: { tokens: null, cost_usd: null, chats_then: 12, ...from },
          estimate_same_library: true,
        }),
      ),
    ).toContain("did not say what its chats spent, so purlis cannot say what this one spends");
  });

  it("says each way a grade ends", () => {
    const step = {
      plane: "p",
      router: "relevance",
      grade: "g",
      ran: 4,
      of: 18,
      tokens: null,
      cap: 1000,
      said: null,
    } as const;
    expect(routerEndedSaid({ ...step, ended: "stopped" })).toBe("Stopped after 4 chats.");
    expect(routerEndedSaid({ ...step, ended: "over-cap" })).toBe(
      "Stopped at its cap of 1,000 tokens after 4 chats.",
    );
    expect(
      routerEndedSaid({ ...step, ended: "chats-did-not-start", said: "the router did not answer" }),
    ).toBe("The grade stopped: its chats did not start. the router did not answer");
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
    said: "Graded on the library at aaaaaaaaaaaa, 2026-10-30: 1 of 2 trigger tests pass with the router relevance, and 2 of 2 with the built-in router.",
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
      "Graded 2026-10-30 10:00 UTC, on the library at aaaaaaaaaaaa, on Claude Code · sonnet for $0.12.",
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
