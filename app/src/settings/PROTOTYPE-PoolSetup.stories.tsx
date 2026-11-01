// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** PROTOTYPE, throwaway (branch prototype/pool-setup, spec #4469). Never merges to main. */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { SCENARIOS } from "./prototype-pool-setup/data";
import { PrototypePoolSetup } from "./prototype-pool-setup/PrototypePoolSetup";

const meta = {
  title: "PROTOTYPE/Pool setup",
  component: PrototypePoolSetup,
  args: { start: "where-docker" },
  argTypes: { start: { control: "select", options: SCENARIOS.map((one) => one.id) } },
} satisfies Meta<typeof PrototypePoolSetup>;

export default meta;

/** Press [ and ] for the previous and next state, or \ for the list of all of them. */
export const EveryState: StoryObj<typeof meta> = {
  render: ({ start }) => <PrototypePoolSetup key={start} start={start} />,
};
