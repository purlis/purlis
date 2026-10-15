// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { TaskQuietNotice } from "./TaskQuietNotice";

/**
 * **A task gone quiet says so on its tab** (#1981, D-6): purlis's note, which ends nothing, with
 * Dismiss. It comes down by itself when the task is heard from again.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const SAID = "task 'check prod' (chat 7) has shown no activity for 15 minutes";

/** A core holding `said` for chat 7. */
function core(said: string | null) {
  const asked: string[] = [];
  let now = said;
  mockIPC(
    (cmd) => {
      asked.push(cmd);
      if (cmd === "task_quiet") return now;
      if (cmd === "put_away_task_quiet") {
        now = null;
        return null;
      }
      return null;
    },
    { shouldMockEvents: true },
  );
  return {
    pressed: () => asked.filter((one) => one !== "task_quiet"),
    say: (next: string | null) => {
      now = next;
    },
  };
}

const notice = () => screen.findByRole("status", { name: "Quiet" });

describe("a task's quiet Notice", () => {
  it("says purlis's note and that it ends nothing", async () => {
    core(SAID);
    render(<TaskQuietNotice plane={PLANE} session={7} />);
    expect(await notice()).toHaveTextContent(
      "purlis notes that task 'check prod' (chat 7) has shown no activity for 15 minutes. It is still running: purlis ends nothing for it, and the chat that asked was told too.",
    );
  });

  it("shows nothing for a task that has not gone quiet", async () => {
    const { pressed } = core(null);
    render(<TaskQuietNotice plane={PLANE} session={7} />);
    await waitFor(() => expect(pressed()).toEqual([]));
    expect(screen.queryByRole("status", { name: "Quiet" })).toBeNull();
  });

  it("is put away on Dismiss", async () => {
    const { pressed } = core(SAID);
    render(<TaskQuietNotice plane={PLANE} session={7} />);
    await notice();
    await userEvent.click(screen.getByRole("button", { name: /dismiss/i }));
    await waitFor(() => expect(pressed()).toEqual(["put_away_task_quiet"]));
    await waitFor(() => expect(screen.queryByRole("status", { name: "Quiet" })).toBeNull());
  });

  it("comes and goes as the core says, for its own chat only", async () => {
    const { say } = core(null);
    render(<TaskQuietNotice plane={PLANE} session={7} />);
    say(SAID);
    await act(async () => {
      await emit("task-quiet", { plane: PLANE, session: 8, said: SAID });
    });
    expect(screen.queryByRole("status", { name: "Quiet" })).toBeNull();
    await act(async () => {
      await emit("task-quiet", { plane: PLANE, session: 7, said: SAID });
    });
    expect(await notice()).toBeInTheDocument();
    say(null);
    await act(async () => {
      await emit("task-quiet", { plane: PLANE, session: 7, said: null });
    });
    await waitFor(() => expect(screen.queryByRole("status", { name: "Quiet" })).toBeNull());
  });
});
