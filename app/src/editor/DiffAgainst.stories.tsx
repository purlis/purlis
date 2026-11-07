// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a file's diff is compared against** (#5405), **and which turn** (#5408), with their
 * keyboard paths: the segments by the arrows, then Tab to the picker, which opens on the latest
 * turn and steps to an earlier one.
 */
import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, screen, userEvent, waitFor, within } from "storybook/test";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { DiffAgainst, TurnPicker, type Against } from "./DiffAgainst";

/** The chat's turns, newest first, as the core lists them. */
const TURNS = [4, 3, 2, 1];

function Live({ from }: { from: Against }) {
  const [value, setValue] = useState(from);
  const [number, setNumber] = useState(TURNS[0]);
  return (
    <div style={{ display: "flex", gap: "var(--space-2)", alignItems: "center" }}>
      <DiffAgainst value={value} onChange={setValue} />
      {value === "turn" && <TurnPicker turns={TURNS} number={number} onPick={setNumber} />}
    </div>
  );
}

const meta = {
  component: Live,
  args: { from: "base" },
} satisfies Meta<typeof Live>;

export default meta;
type Story = StoryObj<typeof meta>;

/** How long a list may take to go: WebKit draws the next frame late in a busy run. */
const CLOSES = { timeout: 3_000 };

export const AgainstBase: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const base = canvas.getByRole("radio", { name: "Against base" });
    const turn = canvas.getByRole("radio", { name: "One turn" });
    await expect(base).toBeChecked();

    await userEvent.tab();
    await untilFocused(() => base, "Tab reaches the chosen segment");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(() => turn, "the arrow moves to one turn");
    await expect(turn).toBeChecked();
    expectFocusInSight();
    await userEvent.keyboard("{ArrowLeft}");
    await untilFocused(() => base, "the arrow moves back to against base");
    await expect(base).toBeChecked();
  },
};

/** One turn shown: the picker beside it, on the latest. */
export const LatestTurn: Story = { args: { from: "turn" } };

/**
 * **Stepping back a turn by the keyboard**: Tab from the segments reaches the picker, named
 * Turn and holding the latest; Enter opens it on that turn with every turn listed newest first;
 * ↓ and Enter pick the one before, and the keyboard is back on the picker.
 */
export const StepsBackATurn: Story = {
  args: { from: "turn" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const picker = () => canvas.getByRole("button", { name: /Turn$/ });
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("radio", { name: "One turn" }),
      "Tab reaches the chosen segment",
    );
    await userEvent.tab();
    await untilFocused(picker, "Tab reaches the turn picker");
    expectFocusInSight();
    await expect(picker()).toHaveTextContent("Turn 4, the latest");

    await userEvent.keyboard("{Enter}");
    const list = await screen.findByRole("listbox");
    await expect(
      within(list)
        .getAllByRole("option")
        .map((option) => option.textContent),
    ).toEqual(["Turn 4, the latest", "Turn 3", "Turn 2", "Turn 1"]);
    await userEvent.keyboard("{ArrowDown}{Enter}");
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull(), CLOSES);
    await untilFocused(picker, "the keyboard is back on the picker");
    expectFocusInSight();
    await expect(picker()).toHaveTextContent("Turn 3");
  },
};
