// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from "@storybook/react-vite";
import { LoginSite } from "./LoginSite";

const meta = {
  title: "Notices/Browser login",
  component: LoginSite,
} satisfies Meta<typeof LoginSite>;

export default meta;
type Story = StoryObj<typeof meta>;

const login = {
  domain: "shop.example.test",
  path: "/login",
  vault: "qa",
  user_key: "username",
  key: "password",
  warnings: [],
  line: 'shop-login = { vault = "qa", browser = "https://shop.example.test/login", user = "username", key = "password" }',
  under: "[sandbox.personas.qa] tools",
  no_always: null,
  vault_too: false,
};

export const Plain: Story = {
  args: { login, typed: undefined, onTyped: () => {} },
};

export const Warned: Story = {
  args: {
    login: {
      ...login,
      warnings: [
        "This password is also used on https://old.example.test. Filling it here gives it to a different site.",
        "This name uses letters from another alphabet. It can look like pаypal.example.test, and it may not be the site you know.",
      ],
      no_always: "Always allow isn't offered while qa's hosts and tools wait for your Allow",
    },
    typed: undefined,
    onTyped: () => {},
  },
};
