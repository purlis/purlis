// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Regenerate in a pinned artifact's bar** (#5962, ADR 0122 §6): ready, asking first with the
 * code it approves named, failed with the run's output, and off where its command cannot run.
 * The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { ArtifactRegenerate as About, ArtifactRegenerated } from "./bindings";
import { ArtifactRegenerate } from "./ArtifactRegenerate";
import { expectFocusInSight } from "./stories.testkit";

const READY: About = {
  has_command: true,
  refused: null,
  standing: "approved",
  code: "/plane/skills/backlog-sync",
  fingerprint: "ab".repeat(32),
  last_good: "2026-11-08 09:00 UTC",
};

const FAILED: ArtifactRegenerated = {
  written: [],
  failed:
    "Regenerate failed: its command exited with code 1. The data shown is unchanged; last good: 2026-11-08 09:00 UTC",
  output: "TypeError: rows is undefined\n    at report (scripts/state.mjs:12:3)",
};

function answers(about: About, ran: ArtifactRegenerated = FAILED) {
  mockIPC((cmd) => {
    if (cmd === "artifact_regenerate_about") return about;
    if (cmd === "artifact_regenerate") return ran;
    return null;
  });
}

const meta: Meta<typeof ArtifactRegenerate> = {
  title: "Views/Pinned artifact/Regenerate",
  component: ArtifactRegenerate,
  args: { plane: "p", workspace: "alpha", folder: "backlog-state", name: "Backlog state" },
  decorators: [
    (Story) => (
      <header className="artifact-bar" style={{ inlineSize: "40rem" }}>
        <Story />
      </header>
    ),
  ],
};
export default meta;
type Story = StoryObj<typeof ArtifactRegenerate>;

/** Approved here: a press runs it without asking. */
export const Ready: Story = {
  beforeEach: () => answers(READY),
  play: async ({ canvasElement }) => {
    const bar = within(canvasElement);
    const button = await bar.findByRole("button", { name: "Regenerate" });
    await userEvent.tab();
    await expect(button).toHaveFocus();
    expectFocusInSight();
  },
};

/** Its code changed since it was approved: a press asks, naming the code, and Esc runs nothing. */
export const AsksAgain: Story = {
  beforeEach: () => answers({ ...READY, standing: "changed" }),
  play: async ({ canvasElement }) => {
    const bar = within(canvasElement);
    await userEvent.click(await bar.findByRole("button", { name: "Regenerate" }));
    const page = within(canvasElement.ownerDocument.body);
    const ask = await page.findByRole("alertdialog");
    await expect(ask).toHaveTextContent("/plane/skills/backlog-sync");
    await expect(page.getByRole("button", { name: "Cancel" })).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Escape}");
    // Esc can come while the ask still fades in, and React Aria lets that finish before it goes.
    await waitFor(() => expect(page.queryByRole("alertdialog")).toBeNull());
    await expect(page.queryByTestId("artifact-regenerate-failed")).toBeNull();
  },
};

/** A run that failed: a Notice with the reason, the date the data shown was last good, its
 *  output, and Regenerate again. */
export const Failed: Story = {
  beforeEach: () => answers(READY),
  play: async ({ canvasElement }) => {
    const bar = within(canvasElement);
    const regenerate = await bar.findByRole("button", { name: "Regenerate" });
    await userEvent.click(regenerate);
    const failed = await bar.findByRole("status", { name: "Backlog state was not regenerated" });
    await expect(failed).toHaveTextContent("last good: 2026-11-08");
    await expect(bar.getByTestId("artifact-regenerate-output")).toHaveTextContent(
      "TypeError: rows is undefined",
    );
    await expect(within(failed).getByRole("button", { name: "Regenerate again" })).toBeEnabled();
    // The picture is of the failure, with Regenerate at rest. The click left the story's pointer
    // on it and the focus in it, and the press turns it off and on again while it runs: whether
    // it was still drawn under the pointer, or with its focus ring, hung on the order the engine
    // took those in, and in WebKit its ring came and went from run to run (stories run
    // 42810312973).
    await userEvent.unhover(regenerate);
    regenerate.blur();
    await waitFor(() => {
      expect(regenerate).not.toHaveAttribute("data-hovered");
      expect(regenerate).not.toHaveFocus();
    });
  },
};

/** A command that names no script: off, with the reason. */
export const CannotRun: Story = {
  beforeEach: () =>
    answers({
      ...READY,
      standing: null,
      fingerprint: null,
      refused: "Regenerate approves the script a command runs, and `gh issue list` names none",
    }),
  play: async ({ canvasElement }) => {
    const bar = within(canvasElement);
    await expect(await bar.findByRole("button", { name: "Regenerate" })).toBeDisabled();
  },
};
