// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Decorator, Meta, StoryObj } from "@storybook/react-vite";
import { expect, screen, waitFor } from "storybook/test";
import { SecretMarks } from "./SecretMarks";
import { expectFocusInSight, untilFocusWithin, untilFocused } from "./stories.testkit";

const meta = {
  title: "Browser/Secret marks",
  component: SecretMarks,
} satisfies Meta<typeof SecretMarks>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A view's first row, `width` wide, with its site cut as the view cuts it, before the marks. */
function inARow(width: number): Decorator {
  return (Story) => (
    <div className="browser-head" style={{ inlineSize: width }}>
      <span className="browser-chip">
        <span className="browser-chip-said">console.volaticloud.com</span>
      </span>
      <Story />
    </div>
  );
}

/** Where the row's padding starts at its end: nothing of the row is drawn past it. */
function rowEnd(row: Element): number {
  return row.getBoundingClientRect().right - parseFloat(getComputedStyle(row).paddingInlineEnd);
}

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
  decorators: [inARow(420)],
  play: async ({ canvas, userEvent }) => {
    const mark = canvas.getByRole("img", {
      name: "Secret dash-api-key, filled on dash.example.test",
    });
    // A short key is whole where there is room.
    const said = mark.querySelector<HTMLElement>(".browser-secret-said");
    await expect(said?.scrollWidth ?? 0).toBeLessThanOrEqual(said?.clientWidth ?? 0);
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

/** **Several secrets are one count** (#6937): a key, a 2FA code and a key on a site on this
 *  machine. Enter opens every key with its site and field, never a value; Escape gives the
 *  keyboard back to the count. */
export const Several: Story = {
  args: {
    secrets: [
      dash,
      { ...dash, key: "dash-2fa", label: "Authentication code", code: true },
      { ...dash, key: "dev-token", origin: "http://localhost:3000", page: null, label: "" },
    ],
  },
  play: async ({ canvas, userEvent }) => {
    const count = canvas.getByRole("button", { name: "3 secrets" });
    await userEvent.tab();
    await expect(count).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    const list = await screen.findByRole("dialog", { name: "Secrets purlis filled on this site" });
    await untilFocusWithin(() => list, "Enter opens every secret filled");
    await expect(list).toHaveTextContent("dash-2fa");
    await expect(list).toHaveTextContent("The chat never saw the value.");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await untilFocused(() => count, "Escape gives the keyboard back to the count");
    expectFocusInSight();
  },
};

/** **A key longer than the room** (#6937): cut to an ellipsis inside a narrow row, whole in its
 *  hint, so it never pushes the site or the row past the pane. */
export const ALongKeyInANarrowRow: Story = {
  args: { secrets: [{ ...dash, key: "STAGE_QA_S3RACE_RUNNER_KUBECONFIG_WITH_A_LONG_SUFFIX" }] },
  decorators: [inARow(320)],
  play: async ({ canvas }) => {
    const mark = canvas.getByTestId("secret-mark");
    const row = mark.closest(".browser-head");
    await expect(row?.scrollWidth).toBeLessThanOrEqual(row?.clientWidth ?? 0);
    // Inside the row's padding, its rounded end drawn: the row never clips it.
    await expect(mark.getBoundingClientRect().right).toBeLessThanOrEqual(
      row === null ? 0 : rowEnd(row) + 0.5,
    );
    // The chip shrinks within its group, and its words, in a box of their own, are what is cut.
    const group = mark.parentElement?.getBoundingClientRect();
    await expect(mark.getBoundingClientRect().right).toBeLessThanOrEqual(group?.right ?? 0);
    const said = mark.querySelector<HTMLElement>(".browser-secret-said");
    await expect(said?.scrollWidth ?? 0).toBeGreaterThan(said?.clientWidth ?? 0);
  },
};
