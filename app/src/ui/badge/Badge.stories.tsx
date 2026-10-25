// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The badge and count family** (#2912): every badge and every count, the two statuses Your
 * hosts says (R66), counts drawn as one pill, and a badge with a hint reached by the keyboard.
 * Each held to axe and compared with its baseline (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor } from "storybook/test";
import { expectFocusInSight } from "../../stories.testkit";
import { Tooltip } from "../tooltip/Tooltip";
import { BADGE_VARIANTS, COUNT_VARIANTS, Badge, Count, CountGroup, HostStatus } from "./Badge";

/** Every badge and every count, on the panel's ground and on the window's. */
function Every() {
  const row = { display: "flex", gap: "var(--space-3)", alignItems: "center", flexWrap: "wrap" };
  return (
    <div style={{ display: "grid", gap: "var(--space-5)", inlineSize: "28rem" }}>
      {(["--surface-base", "--surface-raised"] as const).map((ground) => (
        <div
          key={ground}
          style={{
            display: "grid",
            gap: "var(--space-4)",
            padding: "var(--space-4)",
            background: `var(${ground})`,
          }}
        >
          <div style={row as React.CSSProperties}>
            {BADGE_VARIANTS.map((variant) => (
              <Badge key={variant} variant={variant}>
                {variant}
              </Badge>
            ))}
          </div>
          <div style={row as React.CSSProperties}>
            {COUNT_VARIANTS.map((variant, at) => (
              <Count key={variant} variant={variant} said={`${at + 2} ${variant}`}>
                {at + 2}
              </Count>
            ))}
            <CountGroup role="img" aria-label="12 chats, 3 need you">
              <Count aria-hidden="true">12</Count>
              <Count variant="hot" aria-hidden="true">
                3
              </Count>
            </CountGroup>
          </div>
        </div>
      ))}
    </div>
  );
}

const meta = {
  component: Every,
} satisfies Meta<typeof Every>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Variants: Story = {
  play: async ({ canvas }) => {
    await expect(canvas.getAllByText("waiting")).toHaveLength(2);
    // A count says what it counts; its digits are its drawing.
    await expect(canvas.getAllByText("2 neutral")).toHaveLength(2);
    await expect(canvas.getAllByRole("img", { name: "12 chats, 3 need you" })).toHaveLength(2);
  },
};

/** Your hosts' two statuses (R66): "Waiting for you" on the amber wash, "Confirmed" in the ok
 *  words. */
export const HostStatuses: Story = {
  render: () => (
    <ul
      style={{ display: "grid", gap: "var(--space-3)", margin: 0, padding: 0, listStyle: "none" }}
    >
      <li style={{ display: "flex", gap: "var(--space-4)" }}>
        <code>api.example.com:8443</code>
        <HostStatus waiting />
      </li>
      <li style={{ display: "flex", gap: "var(--space-4)" }}>
        <code>docs.github.com</code>
        <HostStatus waiting={false} />
      </li>
    </ul>
  ),
  play: async ({ canvas }) => {
    await expect(canvas.getByText("Waiting for you")).toHaveAttribute("data-variant", "waiting");
    await expect(canvas.getByText("Confirmed")).toHaveAttribute("data-variant", "ok");
  },
};

/** A badge with a hint is a Tab stop: the keyboard reads its hint, and Escape takes it down. */
export const WithAHint: Story = {
  render: () => (
    <p style={{ display: "flex", gap: "var(--space-3)", alignItems: "center" }}>
      Mode
      <Tooltip tip="Set in purlis.local.toml, on this machine only.">
        <Badge variant="accent">This machine</Badge>
      </Tooltip>
    </p>
  ),
  play: async ({ canvas, userEvent }) => {
    const badge = canvas.getByText("This machine");
    await userEvent.tab();
    await expect(badge).toHaveFocus();
    expectFocusInSight();
    await waitFor(() =>
      expect(document.querySelector('[role="tooltip"]')).toHaveTextContent(
        "Set in purlis.local.toml, on this machine only.",
      ),
    );
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(document.querySelector('[role="tooltip"]')).toBeNull());
    await userEvent.tab();
    await expect(badge).not.toHaveFocus();
  },
  parameters: { screenshot: "page" },
};
