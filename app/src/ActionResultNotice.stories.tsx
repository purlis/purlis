// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What an action answered** (#3344, the sidebar-views prototype's Q3 A): a workspace made, as
 * the closable notice at the top of the content draws it, with its title, its one plain
 * sentence, Open, Details holding the core's lines (never the one written for an agent) and ×.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor, within } from "storybook/test";
import { ActionResultNotice } from "./ActionResultNotice";
import { workspaceMade } from "./actionResult";

const LINES = [
  "Workspace 'git-ci-cd' ready (LIVE — charter + manifest + memory committed + shared + auto-saved) → workspaces/git-ci-cd/",
  '⬢ No vision yet — ask the developer what this workspace is for, then record it: purlis workspace vision "<the goal>"',
];

function Result() {
  const [said, setSaid] = useState("Nothing pressed");
  const [closed, setClosed] = useState(false);
  const made = workspaceMade("git-ci-cd", LINES, { onOpen: () => setSaid("Opened git-ci-cd") });
  return (
    <div
      className="window-work"
      style={{ display: "grid", gap: "var(--space-3)", inlineSize: 760 }}
    >
      {!closed && (
        <ActionResultNotice
          cause="result:workspace.create"
          said={made}
          onClose={() => setClosed(true)}
          onSettings={() => undefined}
        />
      )}
      <p style={{ margin: 0 }} data-testid="said">
        {closed ? "Closed" : said}
      </p>
    </div>
  );
}

const meta = { component: Result } satisfies Meta<typeof Result>;
export default meta;
type Story = StoryObj<typeof meta>;

/** Its details open: the core's line, and nothing written for an agent. */
export const WorkspaceMade: Story = {
  play: async ({ canvas, userEvent }) => {
    await expect(canvas.getByText("Workspace git-ci-cd is ready")).toBeVisible();
    await expect(
      canvas.getByText("It has no vision yet. Add one so its chats know what it is for."),
    ).toBeVisible();
    await userEvent.click(canvas.getByRole("button", { name: "Details" }));
    await expect(canvas.getByText(/ready \(LIVE/)).toBeVisible();
    await expect(canvas.queryByText(/ask the developer/)).toBeNull();
    await userEvent.click(canvas.getByRole("button", { name: "Open git-ci-cd" }));
    await expect(canvas.getByTestId("said")).toHaveTextContent("Opened git-ci-cd");
  },
};

/** × and Esc close it. */
export const Closes: Story = {
  play: async ({ canvas, userEvent }) => {
    const notice = canvas.getByRole("status");
    within(notice).getByRole("button", { name: "Open git-ci-cd" }).focus();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(canvas.getByTestId("said")).toHaveTextContent("Closed"));
  },
};
