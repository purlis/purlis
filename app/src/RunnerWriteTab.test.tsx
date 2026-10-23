// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { RunnerWriteTab } from "./RunnerWriteTab";
import type { RunnerWriteWhole } from "./bindings";

/**
 * A runner chat's change its row cannot show whole opens in its own tab (#2046): every text it
 * carries, whole, and Apply there only where what is shown is what would be written.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const lines = Array.from({ length: 30 }, (_, n) => `line ${n + 1} of the lesson`).join("\n");

const whole = (more: Partial<RunnerWriteWhole> = {}): RunnerWriteWhole => ({
  session: 6,
  pool: "builds",
  said: `Remember in this workspace's memory: ${lines}`,
  applies: true,
  why: null,
  ...more,
});

function core(read: RunnerWriteWhole | string) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "runner_write") {
      if (typeof read === "string") throw read;
      return read;
    }
    if (cmd === "apply_runner_write") return "Applied the change of chat 6 on pool builds.";
    return null;
  });
  return (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args);
}

describe("a runner chat's change in its own tab", () => {
  it("shows every line of it and applies it from the tab", async () => {
    const sent = core(whole());
    render(<RunnerWriteTab plane={PLANE} id={3} />);
    const shown = await screen.findByTestId("runner-write-whole");
    expect(shown.textContent).toContain("line 1 of the lesson\nline 2");
    expect(shown.textContent).toContain("line 30 of the lesson");
    expect(screen.getByText(/on pool builds/)).toBeInTheDocument();
    // Apply is a grant (ADR 0097 §6).
    expect(screen.getByRole("button", { name: "Apply" })).toHaveAttribute("data-variant", "grants");
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() => expect(screen.getByTestId("runner-write-answered")).toBeInTheDocument());
    expect(sent("apply_runner_write")).toEqual([{ plane: PLANE, id: 3, from: "tab" }]);
  });

  it("offers Dismiss alone where what is shown is not what would be written", async () => {
    const sent = core(
      whole({ applies: false, why: "It carries a character that draws as nothing." }),
    );
    render(<RunnerWriteTab plane={PLANE} id={3} />);
    await screen.findByTestId("runner-write-not-applied");
    expect(screen.queryByRole("button", { name: "Apply" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(screen.getByTestId("runner-write-answered")).toBeInTheDocument());
    expect(sent("dismiss_runner_write")).toEqual([{ plane: PLANE, id: 3 }]);
    expect(sent("apply_runner_write")).toEqual([]);
  });

  it("says so where the change waits no more", async () => {
    core("That proposal waits no more.");
    render(<RunnerWriteTab plane={PLANE} id={3} />);
    expect(await screen.findByTestId("runner-write-gone")).toHaveTextContent("waits no more");
  });
});
