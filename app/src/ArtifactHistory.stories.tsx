// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **History in a pinned artifact's bar** (#5966, spec #5948 story 47): its versions with how many
 * Regenerates are kept, a comparison with now, none yet, and the bar of a tab shown as of a
 * version. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { ArtifactCompared, ArtifactHistory as History } from "./bindings";
import { ArtifactHistory, AsOfStanding } from "./ArtifactHistory";
import { expectFocusInSight, untilSeenInDialog } from "./stories.testkit";

const HISTORY: History = {
  kept_most: 20,
  versions: [
    {
      id: "data:001762682400-00",
      at: "2026-11-09 10:00 UTC",
      kind: "regenerated",
      by: null,
      said: null,
    },
    {
      id: "template:" + "a".repeat(40),
      at: "2026-11-08 09:00 UTC",
      kind: "template-changed",
      by: "Ada",
      said: "Show the milestones' finish dates",
    },
    {
      id: "data:001762509600-00",
      at: "2026-11-07 10:00 UTC",
      kind: "regenerated",
      by: null,
      said: null,
    },
  ],
};

const COMPARED: ArtifactCompared = {
  template: [{ path: "index.html", how: "changed" }],
  data: [
    { path: "data.json › open", before: "42", after: "37" },
    { path: "data.json › milestones › 2", before: null, after: '"M66"' },
  ],
  cut: false,
};

function answers(history: History = HISTORY) {
  mockIPC((cmd) => {
    if (cmd === "artifact_history") return history;
    if (cmd === "artifact_compare") return COMPARED;
    return null;
  });
}

const meta: Meta<typeof ArtifactHistory> = {
  title: "Views/Pinned artifact/History",
  component: ArtifactHistory,
  args: {
    plane: "p",
    workspace: "alpha",
    folder: "backlog-state",
    name: "Backlog state",
    onAsOf: () => {},
  },
  decorators: [
    (Story) => (
      <header className="artifact-bar" style={{ inlineSize: "40rem" }}>
        <Story />
      </header>
    ),
  ],
};
export default meta;
type Story = StoryObj<typeof ArtifactHistory>;

/** Its versions, newest first, and how many Regenerates this machine keeps; Esc closes it. */
export const Versions: Story = {
  beforeEach: () => answers(),
  play: async ({ canvasElement }) => {
    const bar = within(canvasElement);
    const button = await bar.findByRole("button", { name: "History…" });
    await userEvent.tab();
    await expect(button).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    const page = within(canvasElement.ownerDocument.body);
    const dialog = await page.findByRole("dialog", { name: "History of Backlog state" });
    await expect(dialog).toHaveTextContent("your last 20 Regenerates");
    await userEvent.keyboard("{Escape}");
    await expect(page.queryByRole("dialog")).toBeNull();
  },
};

/** The newest version compared with now: the page's files and the data's values that changed. */
export const Compared: Story = {
  beforeEach: () => answers(),
  play: async ({ canvasElement }) => {
    const bar = within(canvasElement);
    await userEvent.click(await bar.findByRole("button", { name: "History…" }));
    const page = within(canvasElement.ownerDocument.body);
    const compare = await page.findByRole("button", { name: "Compare" });
    await userEvent.click(compare);
    // Compare is pressed while the dialog may still be fading in, so its answer can arrive
    // unseen (#6360).
    await untilSeenInDialog(await page.findByText(/data\.json › open/));
  },
};

/** Nothing kept and nothing saved yet: says so, and offers neither showing nor comparing. */
export const NoneYet: Story = {
  beforeEach: () => answers({ kept_most: 20, versions: [] }),
  play: async ({ canvasElement }) => {
    const bar = within(canvasElement);
    await userEvent.click(await bar.findByRole("button", { name: "History…" }));
    const page = within(canvasElement.ownerDocument.body);
    const said = await page.findByText(/No versions yet/);
    await untilSeenInDialog(said);
    await expect(page.queryByRole("button", { name: "Show this version" })).toBeNull();
    await expect(page.queryByRole("button", { name: "Compare" })).toBeNull();
  },
};

/** A tab shown as of a version: when, what of it is not as it was, and the way back. */
export const AsOfAVersion: Story = {
  render: () => (
    <AsOfStanding
      then={{
        opened: {
          label: "artifact-1",
          standing: "as-of",
          changed_by: null,
          generated: true,
          queries: [],
          plane_granted: false,
        },
        at: "2026-11-07 10:00 UTC",
        template: null,
        data_at: "2026-11-07 10:00 UTC",
      }}
      onBack={() => {}}
    />
  ),
  play: async ({ canvasElement }) => {
    const bar = within(canvasElement);
    await expect(bar.getByRole("status")).toHaveTextContent("As of 2026-11-07 10:00 UTC");
    const back = bar.getByRole("button", { name: "Back to now" });
    await userEvent.tab();
    await expect(back).toHaveFocus();
    expectFocusInSight();
  },
};
