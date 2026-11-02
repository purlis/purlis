// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Raising a limit where it was hit** (spec #4275, #4285): as Start a chat draws it, on its
 * ground, with one limit, with two, and refusing what is typed.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PoolLimitHitRow } from "./bindings";
import { RaiseLimit } from "./RaiseLimit";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const SIZE: PoolLimitHitRow = {
  key: "profileSize",
  plain: "Most a profile may send: size",
  unit: "MiB",
  in_force: 128,
  team_suggests: false,
  needed: 150,
  suggested: 192,
};
const FILES: PoolLimitHitRow = {
  key: "profileFiles",
  plain: "Most a profile may send: files",
  unit: "files",
  in_force: 8192,
  team_suggests: false,
  needed: 9000,
  suggested: 11264,
};

const meta = {
  component: RaiseLimit,
  args: {
    offer: { pool: "builds", hits: [SIZE] },
    action: "Save to the pool and start",
    onApply: fn(),
  },
  beforeEach: () => {
    mockIPC((cmd, args) => {
      if (cmd !== "pool_limit_check") return null;
      const { typed } = args as { typed: string };
      if (!/^\d+$/.test(typed.trim())) throw "Most a profile may send: size is a whole number";
      if (Number(typed) > 1024) throw "Most a profile may send: size is at most 1024 MiB";
      return Number(typed);
    });
    return () => clearMocks();
  },
  decorators: [
    (Story) => (
      <div
        style={{ inlineSize: 480, padding: "var(--space-6)", background: "var(--surface-overlay)" }}
      >
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof RaiseLimit>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **One limit**: the suggestion, what is needed, and the press, by the keyboard. */
export const OneLimit: Story = {
  play: async ({ canvas, userEvent, args }) => {
    const box = canvas.getByRole("textbox", { name: /Most a profile may send: size/ });
    await expect(box).toHaveValue("192");
    await userEvent.tab();
    await untilFocused(() => box, "the suggested value");
    expectFocusInSight();
    await userEvent.tab();
    const press = canvas.getByRole("button", { name: "Save to the pool and start" });
    await untilFocused(() => press, "Tab goes on to the press");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onApply).toHaveBeenCalledWith([["profileSize", 192]]);
  },
};

/** **Two limits at once**: a box each, and one press. */
export const TwoLimits: Story = {
  args: { offer: { pool: "builds", hits: [SIZE, FILES] } },
  play: async ({ canvas, userEvent }) => {
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("textbox", { name: /send: size/ }),
      "the first limit's box",
    );
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("textbox", { name: /send: files/ }),
      "Tab goes on to the second",
    );
    expectFocusInSight();
  },
};

/** **While it saves**: the press says so, keeps the keyboard and does nothing. */
export const Saving: Story = {
  args: { working: "Saving…" },
  play: async ({ canvas, userEvent, args }) => {
    const press = canvas.getByRole("button", { name: "Saving…" });
    await expect(press).toHaveAttribute("aria-disabled", "true");
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(() => press, "the press keeps the keyboard");
    await userEvent.keyboard("{Enter}");
    await expect(args.onApply).not.toHaveBeenCalled();
  },
};

/** **A value the core refuses**: why, and no press. */
export const RefusesWhatIsTyped: Story = {
  play: async ({ canvas, userEvent }) => {
    const box = canvas.getByRole("textbox", { name: /Most a profile may send: size/ });
    await userEvent.clear(box);
    await userEvent.type(box, "5000");
    await canvas.findByText(/is at most 1024 MiB/);
    await expect(box).toHaveAttribute("aria-invalid", "true");
  },
};

/** **A value too small to get through**: what it needs, and no press. */
export const TooSmall: Story = {
  play: async ({ canvas, userEvent }) => {
    const box = canvas.getByRole("textbox", { name: /Most a profile may send: size/ });
    await userEvent.clear(box);
    await userEvent.type(box, "130");
    await canvas.findByText(/It needs 150 MiB, so this would be refused again/);
  },
};

/** **The save refused**: the core's words, beside the form. */
export const SaveRefused: Story = {
  args: { refused: "there is no pool named builds on this machine" },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("alert")).toHaveTextContent(/no pool named builds/);
  },
};
