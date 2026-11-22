// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ReadinessAnswer, ReadinessCheck } from "../bindings";
import type { LiveSetting } from "./groups";
import { readinessGroup } from "./readiness";

/**
 * **You › Ready for all users**: the open project's forge, read as the person. A check that was
 * not read is never drawn as ready, and a blocker says who fixes it and offers the request.
 */

let answer: ReadinessAnswer;
let asked: number;

function check(id: string, verdict: string, more: Partial<ReadinessCheck> = {}): ReadinessCheck {
  return {
    id,
    title: `Title of ${id}`,
    verdict,
    who: null,
    detail: `${id} detail.`,
    link: null,
    steps: [],
    request: null,
    ...more,
  };
}

const checked = (checks: ReadinessCheck[]): ReadinessAnswer => ({
  state: "checked",
  host: "github.com",
  path: "acme/app",
  ready: checks.every((one) => one.verdict === "ready"),
  checks,
});

beforeEach(() => {
  asked = 0;
  mockIPC((cmd) => {
    if (cmd === "plane_readiness") {
      asked += 1;
      return structuredClone(answer);
    }
    return null;
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
});

function Readiness() {
  const setting = readinessGroup("plane-1").settings[0] as LiveSetting;
  const { control } = setting.useControl();
  return <>{control({ id: "readiness", labelledBy: "readiness-label" })}</>;
}

describe("You › Ready for all users", () => {
  it("says a project whose every check is ready is ready for all users", async () => {
    answer = checked([check("role", "ready")]);
    render(<Readiness />);
    expect(await screen.findByText("Ready for all users")).toBeTruthy();
  });

  it("never says ready for a check that was not read", async () => {
    answer = checked([check("role", "ready"), check("branch-protection", "unknown")]);
    render(<Readiness />);
    expect(await screen.findByText("Not ready for all users")).toBeTruthy();
    expect(screen.getByText("Not read")).toBeTruthy();
  });

  it("names who can fix a blocker, its steps and the request to send them", async () => {
    answer = checked([
      check("oauth-app-access", "blocked", {
        who: "owner",
        steps: ["Open https://github.com/organizations/acme/settings/oauth_application_policy."],
        request: "Please approve purlis for acme.",
      }),
    ]);
    render(<Readiness />);
    expect(await screen.findByText(/The owner can fix this\./)).toBeTruthy();
    expect(screen.getByText(/oauth_application_policy/)).toBeTruthy();
    expect(screen.getByText("Please approve purlis for acme.")).toBeTruthy();
  });

  it("says why nothing was checked, and does not say ready", async () => {
    answer = { state: "notChecked", why: "The project has no origin." };
    render(<Readiness />);
    expect(await screen.findByText(/The project has no origin\./)).toBeTruthy();
    expect(screen.queryByText("Ready for all users")).toBeNull();
  });

  it("asks the forge again when Check again is pressed", async () => {
    answer = checked([check("role", "ready")]);
    render(<Readiness />);
    await screen.findByText("Ready for all users");
    expect(asked).toBe(1);
    answer = checked([check("role", "blocked", { who: "you" })]);
    await userEvent.click(screen.getByRole("button", { name: "Check again" }));
    await waitFor(() => expect(screen.getByText("Not ready for all users")).toBeTruthy());
    expect(asked).toBe(2);
  });
});
