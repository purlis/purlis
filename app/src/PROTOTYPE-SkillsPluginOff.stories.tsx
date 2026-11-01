// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** PROTOTYPE, throwaway (branch prototype/skills-plugin-off, #4477). Never merges to main. */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { PrototypeSkills } from "./prototype-skills-plugin-off/PrototypeSkills";

const meta = { title: "PROTOTYPE/Skills plugin off", component: PrototypeSkills } satisfies Meta<typeof PrototypeSkills>;
export default meta;
export const EveryState: StoryObj<typeof meta> = { render: () => <PrototypeSkills /> };
