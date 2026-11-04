// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from "@storybook/react-vite";
import { SecretFillSite } from "./SecretFillNotice";

const meta = {
  title: "Notices/Secret fill",
  component: SecretFillSite,
} satisfies Meta<typeof SecretFillSite>;

export default meta;
type Story = StoryObj<typeof meta>;

const fill = {
  key: "dash-api-key",
  vault: "qa",
  site: "dash.example.test",
  page: "https://dash.example.test/settings/api",
  label: "API key",
  browser: "b1",
  warnings: [],
  card: false,
  line: null,
  under: null,
  no_always: null,
  vault_too: false,
};

export const Plain: Story = {
  args: { fill },
};

/** A site on this machine says its scheme and port. */
export const OnThisMachine: Story = {
  args: {
    fill: {
      ...fill,
      site: "http://localhost:3000",
      page: "http://localhost:3000/settings",
    },
  },
};

/** A vault of another persona's is said plainly, and a field the page labels nothing is "a field". */
export const Warned: Story = {
  args: {
    fill: {
      ...fill,
      label: "",
      vault: "prod",
      warnings: ["Vault prod is ops's, not qa's."],
    },
  },
};

/** Always allow shows the line it writes before it's pressed (#5116). */
export const WithItsAlwaysLine: Story = {
  args: {
    fill: {
      ...fill,
      line: 'dash-api-key = { vault = "qa", key = "dash-api-key", fill = "https://dash.example.test" }',
      under: "[sandbox.personas.qa] tools",
    },
  },
};

/** A card's key is allowed once each time, and shows no line (#5116, SF-4). */
export const ACard: Story = {
  args: {
    fill: {
      ...fill,
      key: "visa",
      vault: "pay",
      site: "shop.example.test",
      page: "https://shop.example.test/checkout",
      label: "Card number",
      card: true,
    },
  },
};
