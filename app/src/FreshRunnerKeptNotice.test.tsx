// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { FreshKept, FreshRunnersKept } from "./bindings";
import { FRESH_RUNNERS_KEPT, FreshRunnerKeptNotice } from "./FreshRunnerKeptNotice";

/**
 * **A task's runner kept, since its work is not all here, is never silent** (#4569): the window
 * says what is missing and offers Fetch again and Discard, which asks once more first. A runner
 * its cluster says is gone is said too.
 */

const PLANE = "/planes/one";

afterEach(() => {
  cleanup();
  clearMocks();
});

const KEPT: FreshKept = {
  chat: "01CHAT",
  runner: "team/fresh-0123456789abcdef",
  workspace: "app",
  why: [
    "HEAD of the repository workspaces/app/web on the runner is at a commit no ref here reaches",
  ],
  gone: false,
};

const told = (kept: FreshKept[]) =>
  act(async () => {
    await emit(FRESH_RUNNERS_KEPT, { plane: PLANE, kept } satisfies FreshRunnersKept);
  });

const notice = (cause = "fresh-runner-kept") =>
  document.querySelector(`[data-cause="${cause}:${PLANE}:01CHAT"]`);

describe("a task's runner kept", () => {
  it("is said from what the core holds, with what is missing", async () => {
    mockIPC((cmd) => (cmd === "fresh_runners_kept" ? [KEPT] : null), { shouldMockEvents: true });
    render(<FreshRunnerKeptNotice plane={PLANE} />);

    await waitFor(() => expect(notice()).not.toBeNull());
    expect(screen.getByText(/workspace app is kept/)).toBeTruthy();
    expect(screen.getByText(/HEAD of the repository .* no ref here reaches\./)).toBeTruthy();
  });

  it("asks again with Fetch again, and goes once the core says the runner went", async () => {
    const asked: [string, unknown][] = [];
    mockIPC(
      (cmd, args) => {
        asked.push([cmd, args]);
        return cmd === "fresh_runners_kept" ? [] : null;
      },
      { shouldMockEvents: true },
    );
    render(<FreshRunnerKeptNotice plane={PLANE} />);
    await told([KEPT]);
    await waitFor(() => expect(notice()).not.toBeNull());

    await userEvent.click(screen.getByRole("button", { name: "Fetch again" }));
    await waitFor(() =>
      expect(asked).toContainEqual(["fetch_fresh_runner_again", { plane: PLANE, chat: "01CHAT" }]),
    );
    await told([]);
    await waitFor(() => expect(notice()).toBeNull());
  });

  it("discards only on a second press", async () => {
    const asked: string[] = [];
    mockIPC(
      (cmd) => {
        asked.push(cmd);
        return cmd === "fresh_runners_kept" ? [KEPT] : null;
      },
      { shouldMockEvents: true },
    );
    render(<FreshRunnerKeptNotice plane={PLANE} />);
    await waitFor(() => expect(notice()).not.toBeNull());

    await userEvent.click(screen.getByRole("button", { name: "Discard" }));
    expect(asked).not.toContain("discard_fresh_runner");
    expect(screen.getByText(/lost once you discard it/)).toBeTruthy();

    await userEvent.click(screen.getByRole("button", { name: "Discard for good" }));
    await waitFor(() => expect(asked).toContain("discard_fresh_runner"));
    // A check of it may be under way, which takes minutes, and it goes only once that ends (#5312).
    expect(await screen.findByText(/It goes once any check of it has ended/)).toBeTruthy();
  });

  it("says a runner its cluster lost, with Forget the runner", async () => {
    mockIPC((cmd) => (cmd === "fresh_runners_kept" ? [{ ...KEPT, gone: true }] : null), {
      shouldMockEvents: true,
    });
    render(<FreshRunnerKeptNotice plane={PLANE} />);

    await waitFor(() => expect(notice("fresh-runner-gone")).not.toBeNull());
    expect(screen.getByText(/gone from its cluster/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Forget the runner" })).toBeTruthy();
  });

  it("names nothing of another project", async () => {
    mockIPC((cmd) => (cmd === "fresh_runners_kept" ? [] : null), { shouldMockEvents: true });
    render(<FreshRunnerKeptNotice plane={PLANE} />);
    await act(async () => {
      await emit(FRESH_RUNNERS_KEPT, {
        plane: "/planes/two",
        kept: [KEPT],
      } satisfies FreshRunnersKept);
    });
    await act(async () => {});
    expect(notice()).toBeNull();
  });
});
