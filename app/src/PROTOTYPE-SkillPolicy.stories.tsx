// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** PROTOTYPE, throwaway (branch prototype/skill-policy, #6031). Never merges to main. */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { PrototypePolicy } from "./prototype-skill-policy/PrototypePolicy";

const meta = { title: "PROTOTYPE/Skill controls", component: PrototypePolicy } satisfies Meta<typeof PrototypePolicy>;
export default meta;
export const EveryVariant: StoryObj<typeof meta> = { render: () => <PrototypePolicy /> };
