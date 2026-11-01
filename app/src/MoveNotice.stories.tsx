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
import { dismissMovedAfterSignIn, forgetMove, heardMove, movedAfterSignIn } from "./chatMoves";
import { MoveNotice } from "./MoveNotice";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const PLANE = "/home/op/project";

const WAITING: ChatMove = {
  session: 3,
  from: null,
  to: "builds",
  stage: "signing-in",
  why: null,
  with: [],
  sign_in: { profile: "work", kind: "claude", harness: "Claude Code", plans: [] },
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
    mockIPC((cmd, args) => {
      if (cmd === "pool_limit_offer_for_move")
        return {
          pool: "builds",
          hits: [
            {
              key: "profileSize",
              plain: "Most a profile may send: size",
              unit: "MiB",
              in_force: 128,
              needed: 150,
              suggested: 192,
            },
          ],
        };
      if (cmd === "pool_limit_check") return Number((args as { typed: string }).typed);
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

/** **The move went ahead after its sign-in** (#4135): its line, until it is dismissed. */
export const MovedAfterASignIn: Story = {
  beforeEach: () => {
    heardMove(PLANE, { ...WAITING, stage: "switching", sign_in: null });
    heardMove(PLANE, { ...WAITING, stage: "over", sign_in: null });
    movedAfterSignIn(PLANE, 3, 3);
    return () => dismissMovedAfterSignIn(PLANE, 3);
  },
  play: async ({ canvas }) => {
    await canvas.findByText("Moved to pool builds.");
  },
};

/** **The move waits on work's sign-in on builds**: its line, with Sign in and Cancel move. */
export const WaitsOnASignIn: Story = {
  play: async ({ canvas }) => {
    await canvas.findByText(/Moves to pool builds once work is signed in there/);
    await expect(canvas.getByRole("button", { name: "Sign in" })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Cancel move" })).toBeVisible();
  },
};

/** **A harness that serves several plans** (#4181): which one is asked first, as on a pool's line. */
export const WaitsOnAPlan: Story = {
  beforeEach: () => {
    heardMove(PLANE, {
      ...WAITING,
      sign_in: {
        profile: "work",
        kind: "opencode",
        harness: "opencode",
        plans: [
          { id: "openai", title: "ChatGPT" },
          { id: "github-copilot", title: "GitHub Copilot" },
        ],
      },
    });
  },
  play: async ({ canvas }) => {
    await expect(await canvas.findByRole("button", { name: "Sign in to ChatGPT" })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Sign in to GitHub Copilot" })).toBeVisible();
  },
};

/** **Sign in pressed**: the strip under the move's line, at its first step. */
export const SigningIn: Story = {
  play: async ({ canvas, userEvent }) => {
    await userEvent.click(await canvas.findByRole("button", { name: "Sign in" }));
    await expect(
      await canvas.findByRole("group", { name: "Signing work in on builds" }),
    ).toBeInTheDocument();
  },
};

/** **A move refused by a pool's limit** (#4313): the suggestion to edit, and the press that saves
 *  it and asks for the move again, by the keyboard. */
export const RefusedByALimit: Story = {
  beforeEach: () => {
    heardMove(PLANE, {
      ...WAITING,
      stage: "failed",
      sign_in: null,
      why: "it needs 150 MiB, over this pool's limit of 128 MiB (Most a profile may send: size): raise it to 192 MiB",
    });
  },
  play: async ({ canvas, userEvent }) => {
    const box = await canvas.findByRole("textbox", { name: /Most a profile may send: size/ });
    await expect(box).toHaveValue("192");
    await expect(box).toHaveAccessibleDescription(/It needs 150 MiB/);
    box.focus();
    await untilFocused(() => box, "the suggested value");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Save to the pool and move" }),
      "Tab goes on to the press",
    );
    expectFocusInSight();
  },
};

/** **A value too small, under the move's line**: the refusal read on the form's own ground. */
export const RefusedByALimitTooSmall: Story = {
  beforeEach: RefusedByALimit.beforeEach,
  play: async ({ canvas, userEvent }) => {
    const box = await canvas.findByRole("textbox", { name: /Most a profile may send: size/ });
    await userEvent.clear(box);
    await userEvent.type(box, "130");
    await canvas.findByText(/It needs 150 MiB, so this would be refused again/);
  },
};
