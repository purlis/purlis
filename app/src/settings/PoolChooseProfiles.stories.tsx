// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What adding a pool ends with: the profiles that sign in on it** (#4048, spec #4023, prototype
 * Variant A): the strip that chooses them, the project's default ticked, and the first of the
 * chosen signing in, in the same strip. Each is drawn in every cell of the theme matrix, light and
 * dark among them. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PoolProfileRow } from "../bindings";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { ChooseProfiles } from "./PoolChooseProfiles";

const a_profile = (name: string, kind = "claude"): PoolProfileRow => ({
  name,
  kind,
  harness: kind === "claude" ? "Claude Code" : "Codex",
  standing: "signed-out",
  not_kept: null,
  plans: [],
});

const ALL = [a_profile("work"), a_profile("personal"), a_profile("oss", "codex")];

/** The core, faked: the new pool keeps `work`, the project's default, until more are added. */
function theCore() {
  let chosen = ["work"];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    if (cmd === "pool_profiles")
      return {
        kept: chosen.flatMap((name) => ALL.filter((one) => one.name === name)),
        addable: ALL.filter((one) => !chosen.includes(one.name)).map((one) => ({
          name: one.name,
          harness: one.harness,
        })),
      };
    if (cmd === "pool_profile_add") {
      chosen = [...chosen, a.name as string];
      return null;
    }
    if (cmd === "pool_sign_in_start") return 1;
    // The login on the runner has not said its page yet.
    if (cmd === "pool_sign_in_next") return new Promise(() => undefined);
    return null;
  });
}

function Choose() {
  return (
    <div style={{ inlineSize: 640 }}>
      <ChooseProfiles pool="builds" plane="/home/op/project" />
    </div>
  );
}

const meta = {
  component: Choose,
  beforeEach: () => {
    theCore();
    return () => clearMocks();
  },
} satisfies Meta<typeof Choose>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Choosing**: this machine's profiles, the project's default ticked; Tab walks each box. */
export const Choosing: Story = {
  play: async ({ canvas, userEvent }) => {
    const group = await canvas.findByRole("group", {
      name: "Profiles that sign in on pool builds",
    });
    await expect(within(group).getByRole("checkbox", { name: "work" })).toBeChecked();
    await expect(within(group).getByRole("checkbox", { name: "personal" })).not.toBeChecked();
    await userEvent.tab();
    await untilFocused(
      () => within(group).getByRole("checkbox", { name: "work" }),
      "Tab reaches the first profile",
    );
    expectFocusInSight();
  },
};

/** **Signing the chosen in**: two ticked, the first one's strip in the same place. */
export const SigningTheChosenIn: Story = {
  play: async ({ canvas, userEvent }) => {
    const group = await canvas.findByRole("group", {
      name: "Profiles that sign in on pool builds",
    });
    await userEvent.click(within(group).getByRole("checkbox", { name: "personal" }));
    await userEvent.click(canvas.getByRole("button", { name: "Sign in 2 profiles" }));
    await canvas.findByText("Profile 1 of 2: work");
    await canvas.findByRole("group", { name: "Signing work in on builds" });
  },
};
