// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (branch prototype/pool-limits, never merged to main).
 *
 * What should a pool's limits page look like? Three variants in Settings › You › This machine ›
 * Pools, on fake data. The floating bar (or the story's args, which keep the pick in the URL)
 * switches the variant, the pool and the error's suggestion; the toolbar's Appearance, or the
 * two stories, switch light and dark.
 * `src/settings/prototype-pool-limits/README-PROTOTYPE.md` says more.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useArgs } from "storybook/preview-api";
import type { PoolId, Suggestion } from "./prototype-pool-limits/data";
import {
  PROTOTYPE_POOLS,
  PROTOTYPE_VARIANTS,
  PrototypePoolLimits,
  SUGGESTIONS,
  type PrototypeVariant,
} from "./prototype-pool-limits/PrototypePoolLimits";

interface Args {
  variant: PrototypeVariant;
  pool: PoolId;
  suggestion: Suggestion;
}

function Driven(args: Args) {
  const [, updateArgs] = useArgs<Args>();
  return <PrototypePoolLimits {...args} onChange={(to) => updateArgs(to)} />;
}

const meta = {
  title: "PROTOTYPE/Pool limits",
  component: Driven,
  parameters: {
    layout: "fullscreen",
    // A prototype: no axe gate and no screenshot baseline.
    a11y: { test: "off" },
  },
  tags: ["!test"],
  argTypes: {
    variant: { control: "inline-radio", options: PROTOTYPE_VARIANTS },
    pool: { control: "inline-radio", options: PROTOTYPE_POOLS },
    suggestion: { control: "select", options: SUGGESTIONS },
  },
  args: { variant: "B", pool: "docker", suggestion: "none" },
} satisfies Meta<typeof Driven>;

export default meta;

type Story = StoryObj<typeof meta>;

/** Flip everything from the bar, in the toolbar's appearance. */
export const Variants: Story = {};

export const Light: Story = { globals: { appearance: "light" } };

export const Dark: Story = { globals: { appearance: "dark" } };
