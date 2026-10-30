// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { RunnerDispatchTab } from "./RunnerDispatchTab";
import type { RunnerDispatchWhole } from "./bindings";

/**
 * A runner chat's held open or dispatch opens in its own tab: the whole brief and what it would
 * start, with Approve there and nowhere a brief is cut.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const brief = Array.from({ length: 20 }, (_, n) => `step ${n + 1} of the check`).join("\n");

const whole = (more: Partial<RunnerDispatchWhole> = {}): RunnerDispatchWhole => ({
  session: 6,
  chat: "devops 2",
  pool: "builds",
  persona: "devops",
  handoff: false,
  name: "check prod",
  place: null,
  creates: false,
  profile: "work",
  long: false,
  brief,
  ...more,
});

function core(read: RunnerDispatchWhole | string) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "runner_dispatch") {
      if (typeof read === "string") throw read;
      return read;
    }
    if (cmd === "approve_runner_dispatch") return "Started chat 9.";
    return null;
  });
  return (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args);
}

describe("a runner chat's dispatch in its own tab", () => {
  it("shows the whole brief and what it would start, and approves it from the tab", async () => {
    const sent = core(whole());
    render(<RunnerDispatchTab plane={PLANE} id={2} />);
    const shown = await screen.findByTestId("runner-dispatch-brief");
    expect(shown.textContent).toBe(brief);
    const facts = screen.getByTestId("runner-dispatch-facts");
    expect(facts).toHaveTextContent("devops");
    expect(facts).toHaveTextContent("check prod");
    expect(facts).toHaveTextContent("work");
    expect(screen.getByText(/can send it more instructions/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Approve" }));
    await waitFor(() => expect(screen.getByTestId("runner-dispatch-answered")).toBeInTheDocument());
    expect(sent("approve_runner_dispatch")).toEqual([{ plane: PLANE, id: 2 }]);
  });

  it("refuses it, and starts nothing", async () => {
    const sent = core(whole({ handoff: true, place: "alpha", creates: true }));
    render(<RunnerDispatchTab plane={PLANE} id={2} />);
    expect(await screen.findByTestId("runner-dispatch-facts")).toHaveTextContent("a new workspace");
    await userEvent.click(screen.getByRole("button", { name: "Refuse" }));
    await waitFor(() => expect(screen.getByTestId("runner-dispatch-answered")).toBeInTheDocument());
    expect(sent("refuse_runner_dispatch")).toEqual([{ plane: PLANE, id: 2 }]);
    expect(sent("approve_runner_dispatch")).toEqual([]);
  });

  it("says when it waits no more", async () => {
    core("That dispatch waits no more.");
    render(<RunnerDispatchTab plane={PLANE} id={2} />);
    expect(await screen.findByTestId("runner-dispatch-gone")).toHaveTextContent("waits no more");
  });
});
