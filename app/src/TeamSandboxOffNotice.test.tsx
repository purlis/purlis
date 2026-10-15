// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { TeamSandboxOffNotice } from "./TeamSandboxOffNotice";
import type { SandboxState, TeamOffState } from "./bindings";

/**
 * **The project's sandbox off reaches each person by one ask** (#2071, SO-8): "Your project
 * turned the sandbox off (commit <sha> by <author>)", with Accept and Keep it on for me,
 * answered by the window alone and never asked again for that commit.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const LABEL = "Your project turned the sandbox off";

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
  answer: null,
};

function core(state: SandboxState, after: SandboxState = QUIET) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "sandbox_state") return state;
    if (cmd === "answer_team_sandbox_off") return after;
    return null;
  });
  return (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args);
}

describe("the project's sandbox off (#2071, SO-8)", () => {
  it("asks once, naming the commit and its author, and says what Accept means", async () => {
    core({ ...QUIET, team_off: OFF });
    render(<TeamSandboxOffNotice plane={PLANE} canAnswer />);
    const notice = await screen.findByRole("status", { name: LABEL });
    expect(notice).toHaveTextContent(OFF.said);
    expect(notice).toHaveTextContent(/plain-file vaults/);
    expect(notice).toHaveTextContent(/Until you answer, your chats keep their own level/);
    expect(screen.getByRole("button", { name: "Accept" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Keep it on for me" })).toBeTruthy();
  });

  it("sends Accept with the commit it showed, and is gone once answered", async () => {
    const sent = core(
      { ...QUIET, team_off: OFF },
      { ...QUIET, team_off: { ...OFF, answer: true } },
    );
    render(<TeamSandboxOffNotice plane={PLANE} canAnswer />);
    await userEvent.click(await screen.findByRole("button", { name: "Accept" }));
    await waitFor(() =>
      expect(sent("answer_team_sandbox_off")).toEqual([
        { plane: PLANE, commit: OFF.commit, accept: true },
      ]),
    );
    await waitFor(() => expect(screen.queryByRole("status", { name: LABEL })).toBeNull());
  });

  it("sends Keep it on for me as no", async () => {
    const sent = core(
      { ...QUIET, team_off: OFF },
      { ...QUIET, team_off: { ...OFF, answer: false } },
    );
    render(<TeamSandboxOffNotice plane={PLANE} canAnswer />);
    await userEvent.click(await screen.findByRole("button", { name: "Keep it on for me" }));
    await waitFor(() =>
      expect(sent("answer_team_sandbox_off")).toEqual([
        { plane: PLANE, commit: OFF.commit, accept: false },
      ]),
    );
  });

  it("asks nothing once answered, or where the project turns nothing off", async () => {
    core({ ...QUIET, team_off: { ...OFF, answer: true } });
    const { unmount } = render(<TeamSandboxOffNotice plane={PLANE} canAnswer />);
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(screen.queryByRole("status", { name: LABEL })).toBeNull();
    unmount();
    core(QUIET);
    render(<TeamSandboxOffNotice plane={PLANE} canAnswer />);
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(screen.queryByRole("status", { name: LABEL })).toBeNull();
  });

  it("offers no answer where this page may not give one", async () => {
    core({ ...QUIET, team_off: OFF });
    render(<TeamSandboxOffNotice plane={PLANE} canAnswer={false} />);
    await screen.findByRole("status", { name: LABEL });
    expect(screen.queryByRole("button", { name: "Accept" })).toBeNull();
    expect(screen.getByText(/from purlis's own window/)).toBeTruthy();
  });
});
