// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from "@storybook/react-vite";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { PoolSignInConfirm } from "./PoolSignInConfirm";
import { untilFocused } from "./stories.testkit";

const meta: Meta<typeof PoolSignInConfirm> = {
  title: "PoolSignInConfirm",
  component: PoolSignInConfirm,
  // The dialog draws over the page, outside the story's frame.
  parameters: { screenshot: "page" },
};
export default meta;

export const Asked: StoryObj<typeof PoolSignInConfirm> = {
  beforeEach: () => {
    mockIPC(
      (cmd) =>
        cmd === "pool_sign_in_asked"
          ? { id: 1, asks: "sign-in", profile: "work", pool: "team", code: "A1B2C3" }
          : null,
      { shouldMockEvents: true },
    );
    return () => clearMocks();
  },
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    const cancel = await page.findByRole("button", { name: "Cancel" });
    await untilFocused(() => cancel, "It starts on Cancel");
    const signIn = page.getByRole("button", { name: "Sign in" });
    await waitFor(() => expect(signIn).toBeEnabled(), { timeout: 4000 });
    await userEvent.tab();
    await untilFocused(() => signIn, "Tab reaches Sign in");
    await userEvent.keyboard("{Shift>}{Tab}{/Shift}");
    await untilFocused(() => cancel, "Shift+Tab goes back to Cancel");
  },
};

/** `purlis pool remove-profile` (#5174): the runner forgets it, and its sign-in with it. */
export const TakingOff: StoryObj<typeof PoolSignInConfirm> = {
  beforeEach: () => {
    mockIPC(
      (cmd) =>
        cmd === "pool_sign_in_asked"
          ? { id: 1, asks: "take-off", profile: "work", pool: "team", code: "A1B2C3" }
          : null,
      { shouldMockEvents: true },
    );
    return () => clearMocks();
  },
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    const cancel = await page.findByRole("button", { name: "Cancel" });
    await untilFocused(() => cancel, "It starts on Cancel");
    const takeOff = page.getByRole("button", { name: "Take it off" });
    await waitFor(() => expect(takeOff).toBeEnabled(), { timeout: 4000 });
    await userEvent.tab();
    await untilFocused(() => takeOff, "Tab reaches Take it off");
    await userEvent.keyboard("{Shift>}{Tab}{/Shift}");
    await untilFocused(() => cancel, "Shift+Tab goes back to Cancel");
  },
};

/** `purlis signing check` (#5625): the window asks GitHub how a pushed commit shows, on a yes. */
export const SigningCheck: StoryObj<typeof PoolSignInConfirm> = {
  beforeEach: () => {
    mockIPC(
      (cmd) =>
        cmd === "pool_sign_in_asked"
          ? {
              id: 1,
              asks: "signing-check",
              profile: "octo/hello",
              pool: "0123abc",
              code: "A1B2C3",
            }
          : null,
      { shouldMockEvents: true },
    );
    return () => clearMocks();
  },
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    const cancel = await page.findByRole("button", { name: "Cancel" });
    await untilFocused(() => cancel, "It starts on Cancel");
    const check = page.getByRole("button", { name: "Check it" });
    await waitFor(() => expect(check).toBeEnabled(), { timeout: 4000 });
    await userEvent.tab();
    await untilFocused(() => check, "Tab reaches Check it");
    await userEvent.keyboard("{Shift>}{Tab}{/Shift}");
    await untilFocused(() => cancel, "Shift+Tab goes back to Cancel");
  },
};

/** `purlis signing check` on GitLab (#5793): the window names the GitLab it asks. */
export const SigningCheckOnGitLab: StoryObj<typeof PoolSignInConfirm> = {
  beforeEach: () => {
    mockIPC(
      (cmd) =>
        cmd === "pool_sign_in_asked"
          ? {
              id: 1,
              asks: "signing-check",
              profile: "team/billing",
              pool: "0123abc",
              code: "A1B2C3",
              on: "GitLab on gitlab.example.org",
            }
          : null,
      { shouldMockEvents: true },
    );
    return () => clearMocks();
  },
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    const cancel = await page.findByRole("button", { name: "Cancel" });
    await untilFocused(() => cancel, "It starts on Cancel");
    // Its title names the GitLab it asks, never GitHub.
    await page.findByRole("alertdialog", {
      name: "Check 0123abc in team/billing on GitLab on gitlab.example.org?",
    });
  },
};
