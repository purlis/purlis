// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { GitMissing } from "./GitMissing";
import { useGitPresence, WATCH_MS } from "./gitPresence";
import type { GitPresence } from "./bindings";

/**
 * **Git before the action** (#1813): a press that needs git runs at once where git is there,
 * and otherwise waits behind the guided screen and carries on once git is installed. The core
 * is stood in for by `mockIPC`: `git_presence` answers what the test says the machine has.
 */

afterEach(() => {
  cleanup();
  clearMocks();
  vi.useRealTimers();
});

const PRESENT: GitPresence = { kind: "present" };
const NO_TOOLS: GitPresence = { kind: "missing", install: { kind: "developer-tools" } };
const NO_PACKAGE: GitPresence = {
  kind: "missing",
  install: { kind: "package", command: "sudo apt install git" },
};

/** A machine whose git `now()` says, and what the window asked of it. */
function machine(now: () => GitPresence) {
  const asked: string[] = [];
  mockIPC((cmd) => {
    asked.push(cmd);
    if (cmd === "git_presence") return now();
    if (cmd === "install_developer_tools") return null;
    return null;
  });
  return asked;
}

/** A surface with one action that needs git, as the welcome's doors and the steps mount it. */
function Surface({ onClone }: { onClone: () => void }) {
  const gate = useGitPresence();
  return (
    <>
      <button type="button" onClick={() => void gate.withGit(onClone)}>
        Clone
      </button>
      <GitMissing gate={gate} />
    </>
  );
}

describe("an action that needs git", () => {
  it("runs at once where git is installed, and no screen is shown", async () => {
    machine(() => PRESENT);
    const clone = vi.fn();
    render(<Surface onClone={clone} />);

    fireEvent.click(screen.getByRole("button", { name: "Clone" }));

    await vi.waitFor(() => expect(clone).toHaveBeenCalledTimes(1));
    expect(screen.queryByTestId("git-missing")).toBeNull();
  });

  it("waits behind the guided screen on a Mac without git, and carries on after Check again", async () => {
    let has: GitPresence = NO_TOOLS;
    const asked = machine(() => has);
    const clone = vi.fn();
    render(<Surface onClone={clone} />);

    fireEvent.click(screen.getByRole("button", { name: "Clone" }));

    expect(await screen.findByRole("heading", { name: "Git is not installed" })).toBeVisible();
    expect(clone).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Install" }));
    await vi.waitFor(() => expect(asked).toContain("install_developer_tools"));
    expect(await screen.findByText(/Apple's installer is open/)).toBeVisible();
    expect(screen.queryByRole("button", { name: "Install" })).toBeNull();

    has = PRESENT;
    fireEvent.click(screen.getByRole("button", { name: "Check again" }));

    await vi.waitFor(() => expect(clone).toHaveBeenCalledTimes(1));
    expect(screen.queryByTestId("git-missing")).toBeNull();
  });

  it("carries on by itself once Apple's installer has put git there", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    let has: GitPresence = NO_TOOLS;
    machine(() => has);
    const clone = vi.fn();
    render(<Surface onClone={clone} />);
    fireEvent.click(screen.getByRole("button", { name: "Clone" }));
    fireEvent.click(await screen.findByRole("button", { name: "Install" }));
    await screen.findByText(/Apple's installer is open/);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(WATCH_MS);
    });
    expect(clone).not.toHaveBeenCalled();

    has = PRESENT;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(WATCH_MS);
    });
    await vi.waitFor(() => expect(clone).toHaveBeenCalledTimes(1));
    expect(screen.queryByTestId("git-missing")).toBeNull();
  });

  it("shows the distribution's package manager line on Linux, with no Install to press", async () => {
    machine(() => NO_PACKAGE);
    render(<Surface onClone={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "Clone" }));

    expect(await screen.findByText("sudo apt install git")).toBeVisible();
    expect(screen.queryByRole("button", { name: "Install" })).toBeNull();
    expect(screen.getByRole("button", { name: "Check again" })).toBeVisible();
  });

  it("is dropped by Not now, and a later look runs nothing", async () => {
    let has: GitPresence = NO_TOOLS;
    machine(() => has);
    const clone = vi.fn();
    render(<Surface onClone={clone} />);
    fireEvent.click(screen.getByRole("button", { name: "Clone" }));

    fireEvent.click(await screen.findByRole("button", { name: "Not now" }));

    expect(screen.queryByTestId("git-missing")).toBeNull();
    has = PRESENT;
    fireEvent.click(screen.getByRole("button", { name: "Clone" }));
    await vi.waitFor(() => expect(clone).toHaveBeenCalledTimes(1));
  });

  it("says the machine could not be looked at, never that git is missing", async () => {
    mockIPC((cmd) => {
      if (cmd === "git_presence") throw new Error("the look failed");
      return null;
    });
    const clone = vi.fn();
    render(<Surface onClone={clone} />);

    fireEvent.click(screen.getByRole("button", { name: "Clone" }));

    expect(
      await screen.findByRole("heading", { name: "purlis could not look for git" }),
    ).toBeVisible();
    expect(screen.getByRole("alert")).toHaveTextContent("the look failed");
    expect(clone).not.toHaveBeenCalled();
  });
});
