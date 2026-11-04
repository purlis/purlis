// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor } from "storybook/test";
import { SecretMarks } from "./SecretMarks";
import { expectFocusInSight } from "./stories.testkit";

const meta = {
  title: "Browser/Secret marks",
  component: SecretMarks,
} satisfies Meta<typeof SecretMarks>;

export default meta;
type Story = StoryObj<typeof meta>;

const dash = {
  key: "dash-api-key",
  origin: "https://dash.example.test",
  page: "https://dash.example.test",
  label: "API key",
  code: false,
};

/** One secret filled on the page's site: a Tab stop whose hint says the field and the site, and
 *  Escape takes the hint down. */
export const One: Story = {
  args: { secrets: [dash] },
  play: async ({ canvas, userEvent }) => {
    const mark = canvas.getByRole("img", {
      name: "Secret dash-api-key, filled on dash.example.test",
    });
    await userEvent.tab();
    await expect(mark).toHaveFocus();
    expectFocusInSight();
    await waitFor(() =>
      expect(document.querySelector('[role="tooltip"]')).toHaveTextContent(
        'the field the page labels "API key" on dash.example.test',
      ),
    );
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(document.querySelector('[role="tooltip"]')).toBeNull());
  },
};

/** A key and a 2FA code on one site, and a key on a site on this machine. */
export const Several: Story = {
  args: {
    secrets: [
      dash,
      { ...dash, key: "dash-2fa", label: "Authentication code", code: true },
      { ...dash, key: "dev-token", origin: "http://localhost:3000", page: null, label: "" },
    ],
  },
  play: async ({ canvas, userEvent }) => {
    for (const name of [
      "Secret dash-api-key, filled on dash.example.test",
      "Secret dash-2fa, filled on dash.example.test",
      "Secret dev-token, filled on http://localhost:3000",
    ]) {
      await userEvent.tab();
      await expect(canvas.getByRole("img", { name })).toHaveFocus();
      expectFocusInSight();
    }
  },
};
