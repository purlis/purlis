// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Who can sign in to a pool's other machine** (#6271, accepted in #6265, choice 9): not sure,
 * recommended and the default, and only the person with people they trust with their logins; as
 * Connect asks it and as the pool's row in Settings › Pools asks it again. Held to axe, walked by
 * the keyboard and pictured in every pictured theme cell.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, within } from "storybook/test";
import { expectFocusInSight } from "../stories.testkit";
import { WhoCanSignIn } from "./WhoCanSignIn";

/** The question as the window holds it: the answer is kept on Save. */
function Held({
  owned: first,
  onARow,
  over = "ssh",
}: {
  owned: boolean;
  onARow?: boolean;
  over?: "ssh" | "tls";
}) {
  const [owned, setOwned] = useState(first);
  return (
    <WhoCanSignIn
      host="build-box"
      owned={owned}
      over={over}
      onSave={(to) => {
        setOwned(to);
        return Promise.resolve();
      }}
      forgetsLoginsOf={onARow === true ? "builds" : undefined}
      onARow={onARow}
    />
  );
}

const meta = {
  component: Held,
  args: { owned: false },
  decorators: [
    (Story) => (
      <div style={{ maxWidth: 720 }} className="pool-setup-page">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof Held>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Not sure, recommended and chosen: the keyboard picks the other answer, and Save keeps it. */
export const NotSure: Story = {
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    const notSure = canvas.getByRole("radio", { name: /Others might, or I.m not sure/ });
    await expect(notSure).toBeChecked();
    notSure.focus();
    await userEvent.keyboard("{ArrowDown}");
    const mine = canvas.getByRole("radio", { name: "Only me, and people I trust with my logins" });
    await expect(mine).toHaveFocus();
    await expect(mine).toBeChecked();
    await userEvent.tab();
    const save = canvas.getByRole("button", { name: "Save who can sign in to build-box" });
    await expect(save).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    await expect(save).toHaveAttribute("aria-disabled", "true");
    await expectFocusInSight();
  },
};

/** Only the person, and people they trust with their logins. */
export const OnlyYou: Story = {
  args: { owned: true },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      canvas.getByRole("radio", { name: "Only me, and people I trust with my logins" }),
    ).toBeChecked();
  },
};

/** Taking the mark back on the pool's row asks first, since it forgets the pool's logins. */
export const TakingItBackAsks: Story = {
  args: { owned: true, onARow: true },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    canvas.getByRole("radio", { name: "Only me, and people I trust with my logins" }).focus();
    await userEvent.keyboard("{ArrowUp}");
    await userEvent.tab();
    await userEvent.keyboard("{Enter}");
    await expect(canvas.getByText(/forgets the logins you allowed on pool builds/)).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Forget its logins" })).toBeVisible();
  },
};

/** Over Docker's TLS API, the CA is what is pinned. */
export const OverTls: Story = {
  args: { owned: false, over: "tls" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText(/If build-box's CA ever changes/)).toBeVisible();
  },
};

/** On the pool's row in Settings › Pools, under no step's heading. */
export const OnThePoolsRow: Story = {
  args: { owned: false, onARow: true },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    await expect(
      canvas.getByRole("radiogroup", { name: "Who can sign in to build-box?" }),
    ).toBeVisible();
    await expect(canvas.queryByRole("heading")).toBeNull();
    canvas.getByRole("radio", { name: /Others might, or I.m not sure/ }).focus();
    await userEvent.keyboard("{ArrowDown}");
    await expect(
      canvas.getByRole("radio", { name: "Only me, and people I trust with my logins" }),
    ).toBeChecked();
    await expectFocusInSight();
  },
};
