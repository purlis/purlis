// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { TeamPolicy } from "./teamPolicy";
import type { SandboxState, TeamOffState } from "../bindings";

/** The team's say over Your level, in Settings (#2071, PR C). */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const IDS = { id: "x", labelledBy: "x-label", describedBy: "x-help" };
const QUIET: SandboxState = {
  on: true,
  offer: false,
  said: null,
  never: [],
  hosts_changed: null,
  presets_changed: null,
  presets: [],
  persona_hosts: [],
  besides: { project_hosts: 0, your_hosts: 0, folders: 0 },
  policy: null,
};
const OFF: TeamOffState = {
  commit: "0123456789abcdef0123456789abcdef01234567",
  author: "Ada Lovelace",
  said: "Your project turned the sandbox off (commit 0123456 by Ada Lovelace).",
  answer: true,
};

function core(state: SandboxState, after: SandboxState = state) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "sandbox_state") return state;
    if (cmd === "answer_team_sandbox_off") return after;
    return null;
  });
  return (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args);
}

describe("the team's policy beside Your level", () => {
  it("says why no level goes below the full sandbox under a never policy", async () => {
    const held =
      'This project\'s policy requires the full sandbox (sandbox.policy = "never" in charter.toml): no chat starts without it, and nobody turns it down on their machine.';
    core({ ...QUIET, level_held: held });
    render(<TeamPolicy plane={PLANE} ids={IDS} />);
    expect(await screen.findByText(held)).toBeTruthy();
  });

  it("says each person chooses where the team says nothing more", async () => {
    core(QUIET);
    render(<TeamPolicy plane={PLANE} ids={IDS} />);
    expect(await screen.findByText(/Each person chooses their own level/)).toBeTruthy();
  });

  it("lets a person who accepted the team's off change their mind", async () => {
    const sent = core(
      { ...QUIET, team_off: OFF },
      { ...QUIET, team_off: { ...OFF, answer: false } },
    );
    render(<TeamPolicy plane={PLANE} ids={IDS} />);
    expect(await screen.findByText(/You accepted it/)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Keep it on for me" }));
    await waitFor(() =>
      expect(sent("answer_team_sandbox_off")).toEqual([{ plane: PLANE, accept: false }]),
    );
    expect(await screen.findByText(/You kept it on/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Accept" })).toBeTruthy();
  });
});
