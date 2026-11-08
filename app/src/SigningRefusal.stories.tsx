// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A forge's refusal of unsigned commits, where it happened** (#5630, `prototype/commit-signing`
 * moments "Lane refused", "Chat refused" and "Sign and retry"): the prompt on a lane and in a
 * chat, before setup, with signing on, and once signed and sent. Drawn with the core faked at
 * its command boundary, held to axe, walked by the keyboard, and pictured in every pictured
 * theme cell (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SigningWaiting } from "./bindings";
import { SigningRefusal } from "./SigningRefusal";
import { expectFocusInSight } from "./stories.testkit";

const LANE: SigningWaiting = {
  ws: "smart-ide",
  repo: "purlis",
  piece: "tab-names",
  branch: "tab-names",
  kind: "github",
  forge: "GitHub",
  host: "github.com",
  path: "purlis/purlis",
  commit: "0123abc0000000000000000000000000000000000",
  chat: 7,
  sent: false,
};

function Shown({
  on = false,
  where = "lane",
  sent = false,
}: {
  on?: boolean;
  where?: "lane" | "chat";
  sent?: boolean;
}) {
  return <SigningRefusal plane="p1" lane={{ ...LANE, sent }} on={on} where={where} />;
}

const meta = {
  component: Shown,
  beforeEach: () => {
    mockIPC((cmd) =>
      cmd === "signing_sign_and_retry"
        ? [{ ws: LANE.ws, repo: LANE.repo, piece: LANE.piece, signed: 2 }]
        : null,
    );
    return () => clearMocks();
  },
} satisfies Meta<typeof Shown>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **On a lane, before setup**: the refusal named, and Set up signing. */
export const LaneRefused: Story = {
  play: async ({ canvasElement }) => {
    const set = await waitFor(() =>
      within(canvasElement).getByRole("button", { name: "Set up signing" }),
    );
    await userEvent.tab();
    await expect(set).toHaveFocus();
    expectFocusInSight();
  },
};

/** **In the chat, before setup**: and that the chat can't set signing up itself. */
export const ChatRefused: Story = {
  args: { where: "chat" },
  play: async ({ canvasElement }) => {
    await waitFor(() =>
      expect(canvasElement).toHaveTextContent("The chat can't set signing up itself"),
    );
  },
};

/** **With signing on**: Sign and retry at once. */
export const SignAndRetry: Story = {
  args: { on: true },
  play: async ({ canvasElement }) => {
    const retry = await waitFor(() =>
      within(canvasElement).getByRole("button", { name: "Sign and retry" }),
    );
    await userEvent.tab();
    await expect(retry).toHaveFocus();
    expectFocusInSight();
  },
};

/** **Signed and sent ✓**: the lane went again. */
export const SignedAndSent: Story = {
  args: { sent: true },
  play: async ({ canvasElement }) => {
    await waitFor(() =>
      expect(within(canvasElement).getByLabelText("Signed and sent ✓")).toBeVisible(),
    );
  },
};
