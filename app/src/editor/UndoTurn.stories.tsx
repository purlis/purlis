// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Undo a turn** (#5411), with its keyboard path: the button opens one question listing the files
 * the undo changes, which starts on Cancel, so ⏎ keeps the turn and only a press undoes it.
 */
import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { UndoTurn } from "./UndoTurn";

/** The paths a turn changed, sorted, as the core lists them. */
const FILES = ["src/lib.rs", "src/new.rs", "src/old.rs"];

function Live({ files }: { files: string[] }) {
  const [said, setSaid] = useState("Not undone");
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <UndoTurn number={4} files={files} undoing={false} onUndo={() => setSaid("Undone")} />
      <p role="status" style={{ margin: 0 }}>
        {said}
      </p>
    </div>
  );
}

const meta = {
  component: Live,
  args: { files: FILES },
} satisfies Meta<typeof Live>;

export default meta;
type Story = StoryObj<typeof meta>;

/** How long a dialog may take to go: WebKit draws the next frame late in a busy run. */
const CLOSES = { timeout: 3_000 };

/** The turn offered: one button, which asks first. */
export const Offered: Story = {};

/**
 * **Asked once, and kept by the keyboard**: Tab reaches the button, Enter opens the question,
 * which names the turn and lists every file; it starts on Cancel, so Enter keeps the turn, and the
 * keyboard is back on the button. Opened again, Esc keeps it too.
 */
export const AskedAndKept: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const page = within(document.body);
    const button = () => canvas.getByRole("button", { name: "Undo turn 4…" });
    await userEvent.tab();
    await untilFocused(button, "Tab reaches Undo turn");
    expectFocusInSight();

    await userEvent.keyboard("{Enter}");
    const alert = await page.findByRole("alertdialog", { name: "Undo turn 4?" });
    const listed = within(alert).getByRole("list", { name: "Files turn 4 changed" });
    await expect(
      within(listed)
        .getAllByRole("listitem")
        .map((item) => item.textContent),
    ).toEqual(FILES);
    await untilFocused(
      () => page.getByRole("button", { name: "Cancel" }),
      "the question starts on Cancel",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(page.queryByRole("alertdialog")).toBeNull(), CLOSES);
    await untilFocused(button, "the keyboard is back on Undo turn");
    await expect(canvas.getByRole("status")).toHaveTextContent("Not undone");

    await userEvent.keyboard("{Enter}");
    await page.findByRole("alertdialog", { name: "Undo turn 4?" });
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(page.queryByRole("alertdialog")).toBeNull(), CLOSES);
    await expect(canvas.getByRole("status")).toHaveTextContent("Not undone");
  },
};

/** **Undone by its own answer**: Tab from Cancel reaches Undo turn 4, and Enter undoes. */
export const Undone: Story = {
  args: { files: ["README.md"] },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const page = within(document.body);
    await userEvent.click(canvas.getByRole("button", { name: "Undo turn 4…" }));
    const alert = await page.findByRole("alertdialog", { name: "Undo turn 4?" });
    await expect(alert).toHaveTextContent("This file goes back as it was before the turn.");
    const undo = () => within(alert).getByRole("button", { name: "Undo turn 4" });
    await untilFocused(
      () => page.getByRole("button", { name: "Cancel" }),
      "the question starts on Cancel",
    );
    await userEvent.tab();
    await untilFocused(undo, "Tab reaches Undo turn 4");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Undone"), CLOSES);
  },
};
