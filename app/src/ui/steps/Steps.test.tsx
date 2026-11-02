// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { StepList, type Step } from "./Steps";

const STEPS: Step[] = [
  { id: "where", name: "Where chats run", state: "done", line: "Docker on this Mac" },
  { id: "pool", name: "Pool", state: "attention", line: "Docker stopped" },
  { id: "signins", name: "Sign-ins", state: "later" },
];

const draw = (onGo = vi.fn(), showing = "pool") =>
  render(<StepList aria-label="Steps" steps={STEPS} showing={showing} onGo={onGo} />);

afterEach(cleanup);

describe("StepList", () => {
  it("says how far along the person is", () => {
    draw();
    expect(screen.getByText("1 of 3 done")).toBeTruthy();
  });

  it("says each state in words as well as by its mark", () => {
    draw();
    expect(screen.getByRole("button", { name: /Where chats run.*Done/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Pool.*Needs attention/ })).toBeTruthy();
  });

  it("marks the step on screen as the current one", () => {
    draw();
    expect(screen.getByRole("button", { name: /^Pool/ }).getAttribute("aria-current")).toBe("step");
    expect(screen.getByRole("button", { name: /^Where/ }).getAttribute("aria-current")).toBeNull();
  });

  it("jumps back to a step reached", async () => {
    const onGo = vi.fn();
    draw(onGo);
    await userEvent.click(screen.getByRole("button", { name: /^Where/ }));
    expect(onGo).toHaveBeenCalledWith("where");
  });

  it("doesn't jump ahead to a step not reached", async () => {
    const onGo = vi.fn();
    draw(onGo);
    const later = screen.getByRole("button", { name: /^Sign-ins/ });
    expect((later as HTMLButtonElement).disabled).toBe(true);
    await userEvent.click(later);
    expect(onGo).not.toHaveBeenCalled();
  });
});
