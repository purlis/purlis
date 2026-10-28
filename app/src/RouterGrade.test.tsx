// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { RouterGrade, routerEndedSaid } from "./RouterGrade";
import type { RouterGradePlan } from "./bindings";

afterEach(() => {
  cleanup();
  clearMocks();
});

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
