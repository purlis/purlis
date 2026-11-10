// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** PROTOTYPE, throwaway (branch prototype/studio-whose-personas, #2935). Never merges to main. */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { PrototypeWhose } from "./prototype-studio-whose-personas/PrototypeWhose";

const meta = { title: "PROTOTYPE/Whose chats, personas", component: PrototypeWhose } satisfies Meta<typeof PrototypeWhose>;
export default meta;
export const EveryVariant: StoryObj<typeof meta> = { render: () => <PrototypeWhose /> };
