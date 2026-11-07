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
  form: "shop.example.test",
  registrable: "example.test",
  form_registrable: "example.test",
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

/** The person's ruling of 2026-11-05: a page the chat chose, not the front one, is shown as large as
 * the site, and said. */
export const OnAPageTheChatChose: Story = {
  args: {
    login: {
      ...login,
      path: "/~mallory/notes.html",
      warnings: [
        "The chat chose the page /~mallory/notes.html, not the site's front page. Opening a page can sign the browser in to someone else's account, on a shared site a page can be anyone's, and a page sees what's filled on it. Allow only if you know /~mallory/notes.html as this site's sign-in page.",
      ],
      line: 'shop-login = { vault = "qa", browser = "https://shop.example.test/~mallory/notes.html", user = "username", key = "password" }',
    },
    typed: undefined,
    onTyped: () => {},
  },
};

/** #4995: the page sends the browser to sign in on another host, and the password goes there. */
export const SignsInElsewhere: Story = {
  args: {
    login: {
      ...login,
      domain: "app.example.test",
      path: "/",
      form: "auth.example.test",
      warnings: [
        "The password goes to auth.example.test, not app.example.test: app.example.test sent the browser there to sign in. Allow only if you know auth.example.test as this site's sign-in.",
      ],
      line: 'shop-login = { vault = "qa", browser = "https://app.example.test/", form = "https://auth.example.test", user = "username", key = "password" }',
    },
    typed: undefined,
    onTyped: () => {},
  },
};

/** #4996: a chat names another site for a login already recorded, and asks to replace it. */
export const ReplacesTheRecordedSite: Story = {
  args: {
    login: {
      ...login,
      domain: "app.example.test",
      path: "/",
      form: "auth.example.test",
      replaces: "https://app.example.test/",
      replaces_form: "app.example.test",
      warnings: [
        "The password goes to auth.example.test, not app.example.test: app.example.test sent the browser there to sign in. Allow only if you know auth.example.test as this site's sign-in.",
      ],
      line: 'shop-login = { vault = "qa", browser = "https://app.example.test/", form = "https://auth.example.test", user = "username", key = "password" }',
    },
    typed: undefined,
    onTyped: () => {},
  },
};

/** The person's ruling of 2026-11-07: the part of each host its owner registered is marked, so a
 * known name borrowed on its left, or a swapped letter, stands out. */
export const ABorrowedName: Story = {
  args: {
    login: {
      ...login,
      domain: "paypal.com.evil.example",
      registrable: "evil.example",
      path: "/",
      form: "login.paypa1.com",
      form_registrable: "paypa1.com",
      warnings: [
        "The password goes to login.paypa1.com, not paypal.com.evil.example: paypal.com.evil.example sent the browser there to sign in. Allow only if you know login.paypa1.com as this site's sign-in.",
      ],
      line: 'shop-login = { vault = "qa", browser = "https://paypal.com.evil.example/", form = "https://login.paypa1.com", user = "username", key = "password" }',
    },
    typed: undefined,
    onTyped: () => {},
  },
};
