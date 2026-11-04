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
  code: false,
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

/** A 2FA code says it is made from the seed under the key, and that the seed goes nowhere. */
export const TwoFactorCode: Story = {
  args: { fill: { ...fill, key: "dash-2fa", label: "Authentication code", code: true } },
};
