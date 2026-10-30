// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A chat's move waiting on its profile's sign-in on the runner** (#3477, spec #4023, prototype
 * Variant A): the move's line on the chat's pane, and the sign-in strip under it, at its first
 * step. Drawn in every cell of the theme matrix, light and dark among them. The core is faked at
 * its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ChatMove } from "./bindings";
import { forgetMove, heardMove } from "./chatMoves";
import { MoveNotice } from "./MoveNotice";

const PLANE = "/home/op/project";

const WAITING: ChatMove = {
  session: 3,
  from: null,
  to: "builds",
  stage: "signing-in",
  why: null,
  with: [],
  sign_in: { profile: "work", kind: "claude", harness: "Claude Code" },
};

function Pane() {
  return (
    <div style={{ inlineSize: 720 }}>
      <MoveNotice plane={PLANE} session={3} />
    </div>
  );
}

const meta = {
  component: Pane,
  beforeEach: () => {
    mockIPC((cmd) => {
      if (cmd === "pool_sign_in_start") return 1;
      // The login on the runner has not said its page yet.
      if (cmd === "pool_sign_in_next") return new Promise(() => undefined);
      return null;
    });
    heardMove(PLANE, WAITING);
    return () => {
      forgetMove(PLANE, 3);
      clearMocks();
    };
  },
} satisfies Meta<typeof Pane>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **The move waits on work's sign-in on builds**: its line, Cancel move, and the strip. */
export const WaitsOnASignIn: Story = {
  play: async ({ canvas }) => {
    await canvas.findByText(/Moves to pool builds once work is signed in there/);
    await expect(
      await canvas.findByRole("group", { name: "Signing work in on builds" }),
    ).toBeInTheDocument();
    await expect(canvas.getByRole("button", { name: "Cancel move" })).toBeVisible();
  },
};
