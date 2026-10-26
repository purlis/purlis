// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The card family** (#2912): a card with a title and its words, and an ask that holds Allow
 * beside Keep blocked, walked by the keyboard, on the panel's ground it stands on. Each held to
 * axe and compared with its baseline (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor } from "storybook/test";
import { expectFocusInSight } from "../../stories.testkit";
import { Button, ButtonRow } from "../button/Button";
import { Card, CardTitle } from "./Card";

/** A side panel's ground, which a card stands one step off. */
function OnAPanel({ children }: { children: React.ReactNode }) {
  return (
    <div
      style={{
        inlineSize: "22rem",
        padding: "var(--space-5)",
        background: "var(--surface-raised)",
      }}
    >
      {children}
    </div>
  );
}

/** An ask: its words, then Allow and Keep blocked, which say what was answered. */
function AnAsk() {
  const [said, setSaid] = useState("Nothing answered yet");
  return (
    <OnAPanel>
      <Card variant="ask" aria-label="steward 4 asks">
        <CardTitle>steward 4 wants to reach api.github.com</CardTitle>
        <p style={{ margin: 0 }}>{said}</p>
        <ButtonRow size="sm">
          <Button variant="grants" onPress={() => setSaid("Allowed for this chat")}>
            Allow for this chat
          </Button>
          <Button onPress={() => setSaid("Kept blocked")}>Keep blocked</Button>
        </ButtonRow>
      </Card>
    </OnAPanel>
  );
}

const meta = {
  component: AnAsk,
} satisfies Meta<typeof AnAsk>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Tab reaches Allow and then Keep blocked, each in sight; a press says what was answered. */
export const Ask: Story = {
  play: async ({ canvas, userEvent }) => {
    const allow = canvas.getByRole("button", { name: "Allow for this chat" });
    const keep = canvas.getByRole("button", { name: "Keep blocked" });
    await userEvent.tab();
    await expect(allow).toHaveFocus();
    expectFocusInSight();
    await userEvent.tab();
    await expect(keep).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByText("Kept blocked")).toBeInTheDocument());
  },
};

/** A card that only says something: its title and its words, no edge of the ask's. */
export const Raised: Story = {
  render: () => (
    <OnAPanel>
      <Card aria-label="A report">
        <CardTitle>The sample project saved</CardTitle>
        <p style={{ margin: 0 }}>3 files on purlis/save/this-machine, pushed a minute ago.</p>
      </Card>
    </OnAPanel>
  ),
  play: async ({ canvas }) => {
    await expect(canvas.getByText("The sample project saved")).toBeInTheDocument();
    await expect(canvas.queryByRole("button")).toBeNull();
  },
};
