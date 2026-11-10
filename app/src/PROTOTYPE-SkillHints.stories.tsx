// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** PROTOTYPE, throwaway (branch prototype/skill-hints, #5874). Never merges to main. */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { PrototypeHints } from "./prototype-skill-hints/PrototypeHints";

const meta = { title: "PROTOTYPE/Skill hints", component: PrototypeHints } satisfies Meta<typeof PrototypeHints>;
export default meta;
export const EveryVariant: StoryObj<typeof meta> = { render: () => <PrototypeHints /> };
