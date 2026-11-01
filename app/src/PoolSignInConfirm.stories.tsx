// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from "@storybook/react-vite";
import { mockIPC } from "@tauri-apps/api/mocks";
import { PoolSignInConfirm } from "./PoolSignInConfirm";

const meta: Meta<typeof PoolSignInConfirm> = {
  title: "Pools/PoolSignInConfirm",
  component: PoolSignInConfirm,
};
export default meta;

export const Asked: StoryObj<typeof PoolSignInConfirm> = {
  beforeEach: () => {
    mockIPC(
      (cmd) =>
        cmd === "pool_sign_in_asked" ? { id: 1, profile: "work", pool: "team" } : null,
      { shouldMockEvents: true },
    );
  },
};
