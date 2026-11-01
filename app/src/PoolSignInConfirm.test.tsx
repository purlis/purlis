// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { POOL_SIGN_IN_CONFIRM, PoolSignInConfirm } from "./PoolSignInConfirm";

afterEach(() => {
  cleanup();
  clearMocks();
});

const ASK = { id: 4, profile: "work", pool: "team" };

function core(first: typeof ASK | null) {
  const state = { asked: first, answers: [] as unknown[] };
  mockIPC(
    (cmd, args) => {
      if (cmd === "pool_sign_in_asked") return state.asked;
      if (cmd === "answer_pool_sign_in") {
        state.answers.push(args);
        return null;
      }
      return null;
    },
    { shouldMockEvents: true },
  );
  return state;
}

describe("a pool sign-in confirmed in the window", () => {
  it("draws nothing while nothing is asked", async () => {
    const state = core(null);
    render(<PoolSignInConfirm />);
    await act(async () => {
      await emit(POOL_SIGN_IN_CONFIRM);
    });
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(state.answers).toEqual([]);
  });

  it("names the profile and the pool, and starts on Cancel", async () => {
    core(ASK);
    render(<PoolSignInConfirm />);
    expect(
      await screen.findByRole("alertdialog", { name: "Sign work in on team?" }),
    ).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus());
  });

  it("says yes only on Sign in pressed", async () => {
    const state = core(ASK);
    render(<PoolSignInConfirm />);
    await userEvent.click(await screen.findByRole("button", { name: "Sign in" }));
    await waitFor(() => expect(state.answers).toEqual([{ id: 4, yes: true }]));
  });

  it("says no on Cancel and on Escape", async () => {
    const state = core(ASK);
    render(<PoolSignInConfirm />);
    await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(state.answers).toEqual([{ id: 4, yes: false }]));

    cleanup();
    state.answers = [];
    render(<PoolSignInConfirm />);
    await screen.findByRole("alertdialog");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(state.answers).toEqual([{ id: 4, yes: false }]));
  });

  it("goes when the core takes the question back", async () => {
    const state = core(ASK);
    render(<PoolSignInConfirm />);
    await screen.findByRole("alertdialog");
    state.asked = null;
    await act(async () => {
      await emit(POOL_SIGN_IN_CONFIRM);
    });
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  });
});
