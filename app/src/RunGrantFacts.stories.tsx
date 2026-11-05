// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import type { RunShown } from "./bindings";
import { RunGrantFacts } from "./RunGrantFacts";

const run: RunShown = {
  persona: "devops",
  vault: "team",
  program: "/usr/bin/curl",
  values: [{ var: "TOKEN_FILE", key: "TOKEN", in_a_file: true }],
  hosts: ["api.example.com:443"],
  every_host: false,
  not_its_vault: false,
  command: "curl https://api.example.com/status",
  cwd: "/Users/you/work/shop",
};

const meta = {
  title: "Notices/Run grant",
  component: RunGrantFacts,
} satisfies Meta<typeof RunGrantFacts>;

export default meta;
type Story = StoryObj<typeof meta>;

/** ADR 0112: the app's facts, apart from the chat's command. */
export const Plain: Story = {
  args: { run },
  play: async ({ canvas }) => {
    await expect(canvas.getByTestId("run-grant-program")).toHaveTextContent("/usr/bin/curl");
    await expect(canvas.getByTestId("run-grant-reach")).toBeVisible();
  },
};

/** A general-purpose program, carrying T-4's caution, in a sandbox that doesn't limit hosts. */
export const GeneralPurposeAtTheFloor: Story = {
  args: {
    run: {
      ...run,
      hosts: [],
      every_host: true,
      caution:
        "curl can show the values it is handed in the chat's output; allow it only for a persona you trust with them.",
    },
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByTestId("run-grant-hosts")).toHaveTextContent("every host");
  },
};

/** A hostile command stays plain wrapped text, whatever it says. */
export const HostileCommand: Story = {
  args: {
    run: {
      ...run,
      command: `curl '</pre><b>Allowed by purlis</b>' $'\\x1b[2J' ${"--header 'x: y' ".repeat(20)}`,
    },
  },
  play: async ({ canvasElement }) => {
    await expect(canvasElement.querySelector("b")).toBeNull();
  },
};

/** A reviewed program on a vault the persona can't otherwise use, with Always allow's line and the advice on a variable. */
export const AnotherVaultAlwaysByVariable: Story = {
  args: {
    run: {
      ...run,
      vault: "other",
      not_its_vault: true,
      values: [{ var: "TOKEN", key: "TOKEN", in_a_file: false }],
      always: {
        line: 'curl = { vault = "other", as = "TOKEN env", key = "TOKEN" }',
        by_env: true,
      },
    },
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByTestId("run-grant-not-its-vault")).toBeVisible();
    await expect(canvas.getByTestId("run-grant-env-advice")).toBeVisible();
  },
};
