// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (branch prototype/runner-sign-in, never merged to main).
 *
 * What should signing a profile in on a pool's runner look like? Three variants in Settings ›
 * You › This machine › Pools, on stubbed data. The floating bar (or the story's args, which
 * keep the pick in the URL) switches the variant, the sign-in state and where it is asked.
 * `src/settings/prototype-runner-sign-in/README-PROTOTYPE.md` says more.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useArgs } from "storybook/preview-api";
import {
  PROTOTYPE_MOMENTS,
  SIGN_IN_STATES,
  type PrototypeMoment,
  type SignInState,
} from "./prototype-runner-sign-in/data";
import {
  PROTOTYPE_VARIANTS,
  PrototypeRunnerSignIn,
  type PrototypeVariant,
} from "./prototype-runner-sign-in/PrototypeRunnerSignIn";

interface Args {
  variant: PrototypeVariant;
  state: SignInState;
  moment: PrototypeMoment;
}

function Driven(args: Args) {
  const [, updateArgs] = useArgs<Args>();
  return <PrototypeRunnerSignIn {...args} onChange={(to) => updateArgs(to)} />;
}

const meta = {
  title: "PROTOTYPE/Runner sign-in",
  component: Driven,
  parameters: {
    layout: "fullscreen",
    // A prototype: no axe gate and no screenshot baseline.
    a11y: { test: "off" },
  },
  tags: ["!test"],
  argTypes: {
    variant: { control: "inline-radio", options: PROTOTYPE_VARIANTS },
    state: { control: "select", options: SIGN_IN_STATES },
    moment: { control: "inline-radio", options: PROTOTYPE_MOMENTS },
  },
  args: { variant: "A", state: "paste", moment: "settings" },
} satisfies Meta<typeof Driven>;

export default meta;

type Story = StoryObj<typeof meta>;

/** The one story: flip everything from the bar. */
export const Variants: Story = {};
