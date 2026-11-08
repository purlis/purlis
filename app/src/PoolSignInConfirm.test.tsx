// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import {
  POOL_SIGN_IN_CONFIRM,
  PoolSignInConfirm,
  SIGN_IN_ARMS_AFTER_MS,
} from "./PoolSignInConfirm";
import type { PoolSignInAsk } from "./bindings";
import { Dialog } from "./ui/dialog/Dialog";

afterEach(() => {
  cleanup();
  clearMocks();
});

const ASK: PoolSignInAsk = {
  id: 4,
  asks: "sign-in",
  profile: "work",
  pool: "team",
  code: "A1B2C3",
};

function core(first: PoolSignInAsk | null) {
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
  it("asks of a profile taken off in those words, and answers the same way (#5174)", async () => {
    const state = core({ ...ASK, asks: "take-off" });
    render(<PoolSignInConfirm />);
    expect(await screen.findByRole("alertdialog")).toBeTruthy();
    expect(screen.getByText("Take work off team?")).toBeTruthy();
    expect(screen.getByText(/its sign-in there with it/)).toBeTruthy();
    expect(screen.getByText("A1B2C3")).toBeTruthy();
    const yes = screen.getByRole("button", { name: "Take it off" });
    await waitFor(() => expect((yes as HTMLButtonElement).disabled).toBe(false), {
      timeout: SIGN_IN_ARMS_AFTER_MS + 2000,
    });
    await userEvent.click(yes);
    expect(state.answers).toEqual([{ id: 4, yes: true }]);
  });

  it("asks of a commit `purlis signing check` asks GitHub about, and answers the same way (#5625)", async () => {
    const state = core({
      ...ASK,
      asks: "signing-check",
      profile: "octo/hello",
      pool: "0123abc",
    });
    render(<PoolSignInConfirm />);
    expect(await screen.findByRole("alertdialog")).toBeTruthy();
    expect(screen.getByText("Check 0123abc in octo/hello on GitHub?")).toBeTruthy();
    expect(screen.getByText(/whether this commit shows as Verified/)).toBeTruthy();
    expect(screen.getByText("A1B2C3")).toBeTruthy();
    const yes = screen.getByRole("button", { name: "Check it" });
    await waitFor(() => expect((yes as HTMLButtonElement).disabled).toBe(false), {
      timeout: SIGN_IN_ARMS_AFTER_MS + 2000,
    });
    await userEvent.click(yes);
    expect(state.answers).toEqual([{ id: 4, yes: true }]);
  });

  it("names the GitLab a `purlis signing check` asks, never GitHub (#5793)", async () => {
    core({
      ...ASK,
      asks: "signing-check",
      profile: "team/billing",
      pool: "0123abc",
      on: "GitLab on gitlab.example.org",
    });
    render(<PoolSignInConfirm />);
    expect(await screen.findByRole("alertdialog")).toBeTruthy();
    expect(
      screen.getByText("Check 0123abc in team/billing on GitLab on gitlab.example.org?"),
    ).toBeTruthy();
    expect(screen.getByText(/ask GitLab on gitlab.example.org, with your/)).toBeTruthy();
    expect(document.body.textContent).not.toContain("GitHub");
  });

  it("draws nothing while nothing is asked", async () => {
    const state = core(null);
    render(<PoolSignInConfirm />);
    await act(async () => {
      await emit(POOL_SIGN_IN_CONFIRM);
    });
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(state.answers).toEqual([]);
  });

  it("waits for a dialog already up to close (#4504)", async () => {
    core(ASK);
    const window = (isOpen: boolean) => (
      <>
        <Dialog isOpen={isOpen} onClose={() => {}} title="Another" titleId="another">
          <p>Something else.</p>
        </Dialog>
        <PoolSignInConfirm />
      </>
    );
    const { rerender } = render(window(true));
    await act(async () => {
      await emit(POOL_SIGN_IN_CONFIRM);
    });
    expect(screen.queryByRole("alertdialog")).toBeNull();
    // Longer than its yes takes to arm: that counts from when it opens, not from when it was asked.
    await act(() => new Promise((done) => setTimeout(done, SIGN_IN_ARMS_AFTER_MS + 100)));

    rerender(window(false));

    await screen.findByRole("alertdialog", { name: "Sign work in on team?" });
    expect(screen.getByRole("button", { name: "Sign in" })).toBeDisabled();
  });

  it("names the profile and the pool, and starts on Cancel", async () => {
    core(ASK);
    render(<PoolSignInConfirm />);
    expect(
      await screen.findByRole("alertdialog", { name: "Sign work in on team?" }),
    ).toBeInTheDocument();
    expect(screen.getByText("A1B2C3")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus());
  });

  it("says yes only on Sign in pressed", async () => {
    const state = core(ASK);
    render(<PoolSignInConfirm />);
    const signIn = await screen.findByRole("button", { name: "Sign in" });
    await waitFor(() => expect(signIn).toBeEnabled(), { timeout: 4000 });
    await userEvent.click(signIn);
    await waitFor(() => expect(state.answers).toEqual([{ id: 4, yes: true }]));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  });

  it("takes a yes from the keyboard once it can be pressed", async () => {
    const state = core(ASK);
    render(<PoolSignInConfirm />);
    const signIn = await screen.findByRole("button", { name: "Sign in" });
    await waitFor(() => expect(signIn).toBeEnabled(), { timeout: 4000 });
    await userEvent.tab();
    await waitFor(() => expect(signIn).toHaveFocus());
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(state.answers).toEqual([{ id: 4, yes: true }]));
  });

  it("takes no yes from typing meant for somewhere else", async () => {
    const state = core(ASK);
    render(<PoolSignInConfirm />);
    const signIn = await screen.findByRole("button", { name: "Sign in" });
    expect(signIn).toBeDisabled();
    await userEvent.keyboard("{Tab}{Enter}");
    expect(state.answers).not.toContainEqual({ id: 4, yes: true });
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
