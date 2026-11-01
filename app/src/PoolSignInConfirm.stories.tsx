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
          ? { id: 1, profile: "work", pool: "team", code: "A1B2C3" }
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
    await waitFor(() => expect(signIn).toBeEnabled(), { timeout: 4000 });
    await untilFocused(() => signIn, "Tab reaches Sign in");
    await userEvent.keyboard("{Shift>}{Tab}{/Shift}");
    await untilFocused(() => cancel, "Shift+Tab goes back to Cancel");
  },
};
