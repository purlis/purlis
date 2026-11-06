// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent } from "storybook/test";
import type { RunShown } from "./bindings";
import { RunFills } from "./RunFills";

const run: RunShown = {
  persona: "devops",
  vault: "team",
  program: "/usr/bin/curl",
  values: [
    { var: "TOKEN", key: "TOKEN", in_a_file: false },
    { var: "SECRET_FILE", key: "CLIENT_SECRET", in_a_file: true },
  ],
  hosts: ["api.example.com:443"],
  every_host: false,
  not_its_vault: false,
  command: "curl https://api.example.com/status",
  cwd: "/Users/you/work/shop",
  fills: [{ key: "TOKEN" }, { key: "CLIENT_SECRET", declared: "signing in to the shop's API" }],
  offer: "program",
};

const meta = {
  title: "Notices/Run grant/Value fields",
  component: RunFills,
  args: { run, boxes: { current: new Map<string, HTMLInputElement>() } },
} satisfies Meta<typeof RunFills>;

export default meta;
type Story = StoryObj<typeof meta>;

/** #5242: two keys the run lacks, two masked boxes, beside the program's Allow. */
export const TwoKeysBesideTheAllow: Story = {
  play: async ({ canvas }) => {
    const token = canvas.getByLabelText("Value of TOKEN");
    await expect(token).toHaveAttribute("type", "password");
    // By keyboard alone: one box, then the next.
    await userEvent.click(token);
    await userEvent.keyboard("t0ken");
    await userEvent.tab();
    await expect(canvas.getByLabelText("Value of CLIENT_SECRET")).toHaveFocus();
  },
};

/** A vault the persona can't use: what is typed is stored only on an Allow. */
export const OnAVaultThePersonaCantUse: Story = {
  args: { run: { ...run, not_its_vault: true } },
  play: async ({ canvas }) => {
    await expect(canvas.getByTestId("run-fills")).toHaveTextContent("only if you allow");
  },
};

/** No program Allow (a declared tool): the vault's line has its own Allow. */
export const TheVaultsOwnAllow: Story = {
  args: { run: { ...run, offer: "vault", not_its_vault: true, fills: [{ key: "TOKEN" }] } },
  play: async ({ canvas }) => {
    await expect(canvas.getByTestId("run-fills")).toHaveTextContent("every chat opened as devops");
  },
};

/** Policy offers no Allow: Fill still stores, and the notice says why. */
export const LockedByPolicy: Story = {
  args: {
    run: {
      ...run,
      offer: "fill",
      fills: [{ key: "TOKEN" }],
      no_allow:
        "Policy turns off allowing a program with a vault's keys, so curl needs to be declared as a tool of devops's.",
    },
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByTestId("run-fills-no-allow")).toBeVisible();
  },
};
