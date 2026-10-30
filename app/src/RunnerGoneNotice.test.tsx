// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ChatsReconnecting } from "./bindings";
import { RECONNECTING } from "./reconnecting";
import { RunnerGoneNotice } from "./RunnerGoneNotice";

/**
 * **A refused start on a runner removed outside purlis offers Make the runner again** (#4149):
 * once for the project, naming the pool, saying whether its home went too, and gone once the
 * runner is made again.
 */

const PLANE = "/planes/one";

afterEach(() => {
  cleanup();
  clearMocks();
});

const told = (said: Partial<ChatsReconnecting>) =>
  act(async () => {
    await emit(RECONNECTING, {
      plane: PLANE,
      sessions: [1],
      reconnection: "runner_gone",
      ...said,
    } satisfies ChatsReconnecting);
  });

const notice = () => document.querySelector(`[data-cause="runner-gone:${PLANE}:first"]`);

describe("a runner removed outside purlis", () => {
  it("is said with Make the runner again, which makes it and puts the Notice away", async () => {
    const asked: string[] = [];
    mockIPC(
      (cmd, args) => {
        asked.push(cmd);
        if (cmd === "docker_pools_removed") return [{ name: "first", home_kept: false }];
        if (cmd === "docker_pool_make_again") {
          expect(args).toEqual({ name: "first" });
          return { sign_ins_kept: false };
        }
        return null;
      },
      { shouldMockEvents: true },
    );
    render(<RunnerGoneNotice plane={PLANE} onOpenPools={() => {}} />);
    // A link that is only down says nothing here: its panes say Reconnecting….
    await told({ reconnection: "reconnecting" });
    expect(notice()).toBeNull();

    await told({});
    await waitFor(() => expect(notice()).not.toBeNull());
    expect(screen.getByText(/removed outside purlis/)).toBeTruthy();
    expect(screen.getByText(/sign-ins kept there/)).toBeTruthy();

    await userEvent.click(screen.getByRole("button", { name: "Make the runner again" }));
    await waitFor(() => expect(notice()).toBeNull());
    expect(asked).toContain("docker_pool_make_again");
  });

  it("names nothing where no pool's runner is gone", async () => {
    mockIPC((cmd) => (cmd === "docker_pools_removed" ? [] : null), { shouldMockEvents: true });
    render(<RunnerGoneNotice plane={PLANE} onOpenPools={() => {}} />);
    await told({});
    await act(async () => {});
    expect(notice()).toBeNull();
  });
});
